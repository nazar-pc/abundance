#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(
    const_cmp,
    const_trait_impl,
    const_try,
    const_try_residual,
    generic_const_exprs,
    iter_array_chunks,
    signed_bigint_helpers,
    try_blocks
)]

mod abundance_rv32i_max;
mod abundance_rv64i_max;
mod instruction;
mod interpreter;

use crate::abundance_rv32i_max::instruction::AbundanceRv32IMaxInstruction;
use crate::abundance_rv32i_max::interpreter::AbundanceRv32IMaxExtState;
use crate::abundance_rv64i_max::instruction::AbundanceRv64IMaxInstruction;
use crate::abundance_rv64i_max::interpreter::AbundanceRv64IMaxExtState;
use crate::interpreter::Act4SystemHandler;
use ab_riscv_interpreter::basic::{BasicInstructionFetcher, BasicInterpreterState, BasicRegisters};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use clap::{Parser, ValueEnum};
use colored::Colorize;
use interpreter::Act4Memory;
use object::{Object, ObjectSegment, ObjectSymbol};
use std::ffi::CStr;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[cfg(not(target_endian = "little"))]
compile_error!("Only little-endian platforms are supported");

type RegisterType<I> = <<I as Instruction>::Reg as Register>::Type;

const RAM_BASE: u64 = 0x8000_0000;
const RAM_SIZE: usize = 4 * 1024 * 1024;
const MRET_INSTRUCTION: u32 = 0x3020_0073;

/// RISC-V ISA
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Isa {
    /// RV32
    Rv32,
    /// RV64
    Rv64,
}

#[derive(Parser)]
#[command(about = "Run RISC-V ACT compliance tests against the interpreter")]
struct Cli {
    isa: Isa,
    /// Directory containing *.elf ACT4 test binaries
    elfs: PathBuf,
    /// Only run tests whose filename contains this substring
    #[arg(long)]
    filter: Option<String>,
    /// Stop after the first failing test
    #[arg(long)]
    fail_fast: bool,
}

struct ParsedElf<Reg>
where
    Reg: Register,
{
    entry: Reg::Type,
    tohost_addr: u64,
    begin_signature: u64,
    end_signature: u64,
    begin_failure_scratch: u64,
    // PT_LOAD segments with file data: (vaddr, bytes)
    segments: Vec<(u64, Vec<u8>)>,
}

impl<Reg> ParsedElf<Reg>
where
    Reg: Register,
{
    fn from_path(path: &Path) -> anyhow::Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("Failed to read ELF file {}", path.display()))?;
        let elf = object::File::parse(bytes.as_slice())
            .with_context(|| format!("Failed to parse ELF file {}", path.display()))?;

        let mut tohost_addr = None;
        let mut begin_signature = None;
        let mut end_signature = None;
        let mut begin_failure_scratch = None;
        for sym in elf.symbols() {
            match sym.name().unwrap_or("") {
                "tohost" => tohost_addr = Some(sym.address()),
                "begin_signature" => begin_signature = Some(sym.address()),
                "end_signature" => end_signature = Some(sym.address()),
                "begin_failure_scratch" => begin_failure_scratch = Some(sym.address()),
                _ => {}
            }
        }
        let tohost_addr = tohost_addr.context("Symbol `tohost` not found")?;
        let begin_signature = begin_signature.context("Symbol `begin_signature` not found")?;
        let end_signature = end_signature.context("Symbol `end_signature` not found")?;
        let begin_failure_scratch =
            begin_failure_scratch.context("Symbol `begin_failure_scratch` not found")?;

        let mut segments = Vec::new();
        for segment in elf.segments() {
            let data = match segment.data() {
                Ok(d) if !d.is_empty() => d,
                _ => continue,
            };
            let vaddr = segment.address();
            if vaddr < RAM_BASE {
                continue;
            }
            segments.push((vaddr, data.to_vec()));
        }

        let entry = Reg::Type::from(elf.entry() as u32);
        if entry.as_u64() != elf.entry() {
            return Err(anyhow::anyhow!(
                "Entry point {} outside 32-bit range",
                elf.entry()
            ));
        }

        Ok(Self {
            entry,
            tohost_addr,
            begin_signature,
            end_signature,
            begin_failure_scratch,
            segments,
        })
    }

    fn reference_signature(&self) -> anyhow::Result<&[u8]> {
        let begin = self.begin_signature;
        let end = self.end_signature;
        let len = end.checked_sub(begin).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid signature region: end_signature (0x{end:x}) is before \
                begin_signature (0x{begin:x})"
            )
        })? as usize;
        if len == 0 {
            return Ok(&[]);
        }
        if !len.is_multiple_of(size_of::<Reg::Type>()) {
            return Err(anyhow::anyhow!(
                "Signature region length {len} is not a multiple of {}",
                size_of::<Reg::Type>()
            ));
        }

        for (seg_addr, data) in &self.segments {
            let seg_end = seg_addr + data.len() as u64;
            if begin >= *seg_addr && end <= seg_end {
                let off = (begin - seg_addr) as usize;
                return Ok(&data[off..][..len]);
            }
        }

        Err(anyhow::anyhow!(
            "Signature region 0x{begin:x}..0x{end:x} not found in any loadable segment"
        ))
    }
}

#[derive(Debug)]
enum TestError<RegType> {
    HtifFail {
        exit_code: u64,
        detail: String,
    },
    SignatureMismatch {
        word: usize,
        actual: RegType,
        expected: RegType,
    },
    LengthMismatch {
        actual_bytes: usize,
        expected_bytes: usize,
    },
    Execution(ExecutionError<RegType>),
    Test(anyhow::Error),
}

impl<RegType> From<ExecutionError<RegType>> for TestError<RegType> {
    fn from(error: ExecutionError<RegType>) -> Self {
        Self::Execution(error)
    }
}

impl<RegType> From<anyhow::Error> for TestError<RegType> {
    fn from(error: anyhow::Error) -> Self {
        Self::Test(error)
    }
}

trait ToHost {
    fn tohost_value<RT>(&self, tohost_addr: u64) -> anyhow::Result<Option<RT>>
    where
        RT: RegType + BasicInt;
}

impl<T> ToHost for T
where
    T: VirtualMemory,
{
    fn tohost_value<RT>(&self, tohost_addr: u64) -> anyhow::Result<Option<RT>>
    where
        RT: RegType + BasicInt,
    {
        let raw_value = self
            .read::<RT>(tohost_addr)
            .context("Failed to read `tohost`")?;

        Ok(if raw_value.as_u64() == 0 {
            None
        } else {
            Some(raw_value)
        })
    }
}

fn read_cstring<const RAM_BASE: u64, const RAM_SIZE: usize>(
    memory: &Act4Memory<RAM_BASE, RAM_SIZE>,
    addr: u64,
) -> Option<&str> {
    let slice = memory.read_slice_up_to(addr, 512);
    CStr::from_bytes_until_nul(slice).ok()?.to_str().ok()
}

fn read_failure_info<const RAM_BASE: u64, const RAM_SIZE: usize, RT>(
    memory: &Act4Memory<RAM_BASE, RAM_SIZE>,
    begin_failure_scratch: u64,
) -> Option<String>
where
    RT: RegType + BasicInt,
{
    // Offsets from `begin_failure_scratch` for the failure info fields.
    //
    // Layout defined in `tests/env/failure_code.h` (`RVTEST_FAILURE_DATA`):
    // * `begin_failure_scratch` is the same address as `failure_type` (offset 0)
    // * x0..x31 are saved at byte offsets 0..248, using a fixed 256-byte region (`.fill 64, 4`)
    //   regardless of XLEN
    // * The fields below follow the register save area at offset 256 (0x100)

    /// Always `sw` (4 bytes): 0=int, 1=fp, 2=fflags, 3=trap handler, 4=vector
    const FAILURE_TYPE: u64 = 0x000;
    /// Always `sw` (4 bytes): raw instruction bits
    const FAILING_INSTRUCTION: u64 = 0x100;
    /// Always `sw` (4 bytes): register number
    const FAILING_REG: u64 = 0x104;
    /// XLEN-wide (`SREG`): PC of the failing instruction
    const FAILING_ADDR: u64 = 0x108;
    /// XLEN-wide (`SREG`): actual (bad) register value
    const FAILING_VALUE: u64 = 0x110;
    /// XLEN-wide (`SREG`): expected register value
    const EXPECTED_VALUE: u64 = 0x118;
    /// XLEN-wide (`SREG`): pointer to the test name string
    const FAILURE_STRING_PTR: u64 = 0x120;

    let failure_type = memory
        .read::<u32>(begin_failure_scratch + FAILURE_TYPE)
        .ok()?;
    let raw_inst = memory
        .read::<u32>(begin_failure_scratch + FAILING_INSTRUCTION)
        .ok()?;
    let failing_reg = memory
        .read::<u32>(begin_failure_scratch + FAILING_REG)
        .ok()?;
    let failing_addr = memory
        .read::<RT>(begin_failure_scratch + FAILING_ADDR)
        .ok()?
        .as_u64();
    let actual_value = memory
        .read::<RT>(begin_failure_scratch + FAILING_VALUE)
        .ok()?
        .as_u64();
    let expected_value = memory
        .read::<RT>(begin_failure_scratch + EXPECTED_VALUE)
        .ok()?
        .as_u64();
    let str_ptr = memory
        .read::<RT>(begin_failure_scratch + FAILURE_STRING_PTR)
        .ok()?
        .as_u64();

    let test_name = read_cstring(memory, str_ptr).unwrap_or("<unknown>");

    let xlen_hex_width = size_of::<RT>() * 2;

    let reg_prefix = match failure_type {
        0 | 3 => "x",
        1 | 2 => "f",
        4 => "v",
        _ => "?",
    };

    Some(format!(
        "\n  test:     {test_name}\
         \n  pc:       0x{failing_addr:0xlen_hex_width$x}\
         \n  inst:     0x{raw_inst:08x}\
         \n  reg:      {reg_prefix}{failing_reg}\
         \n  actual:   0x{actual_value:0xlen_hex_width$x}\
         \n  expected: 0x{expected_value:0xlen_hex_width$x}"
    ))
}

// TODO: It doesn't seem to be possible to make this generic over the instruction type at the moment
fn run_rv32i_max_test(
    elf_path: &Path,
) -> Result<(), TestError<RegisterType<AbundanceRv32IMaxInstruction>>> {
    let elf = ParsedElf::<<AbundanceRv32IMaxInstruction as Instruction>::Reg>::from_path(elf_path)?;

    let mut ram = Act4Memory::<RAM_BASE, RAM_SIZE>::new();
    for (vaddr, data) in &elf.segments {
        ram.write_slice(*vaddr, data)
            .map_err(ExecutionError::from)?;
    }

    let mut state = BasicInterpreterState {
        regs: BasicRegisters::default(),
        ext_state: AbundanceRv32IMaxExtState::new(),
        memory: ram,
        instruction_fetcher: BasicInstructionFetcher::<AbundanceRv32IMaxInstruction>::new(
            // Not used, setting to something that is unlikely to be used
            0, elf.entry,
        ),
        system_instruction_handler: Act4SystemHandler,
    };

    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory) {
            Ok(FetchInstructionResult::Instruction(instruction)) => instruction,
            Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(()))) => break,
            Ok(FetchInstructionResult::ControlFlow(ControlFlow::Continue(()))) => continue,
            // TODO: This custom handling is temporary until interpreter has abstractions and
            //  support for privileged instructions
            Err(ExecutionError::IllegalInstruction { address }) => {
                // Check for mret before treating as a trap - mret is a privileged instruction the
                // interpreter doesn't implement, so it arrives here as an illegal instruction
                let raw_instruction = state
                    .memory
                    .read::<u32>(u64::from(address))
                    .map_err(ExecutionError::from)?;
                if raw_instruction == MRET_INSTRUCTION {
                    let mepc = state
                        .ext_state
                        .read_csr(MCsr::Mepc as u16)
                        .map_err(ExecutionError::from)?;
                    match state
                        .instruction_fetcher
                        .set_pc(&state.memory, mepc)
                        .map_err(ExecutionError::from)?
                    {
                        ControlFlow::Continue(()) => {
                            continue;
                        }
                        ControlFlow::Break(()) => {
                            break;
                        }
                    }
                }

                // All other illegal instructions dispatch through the trap handler
                let trap_pc = state
                    .ext_state
                    .take_trap(
                        MCauseException::IllegalInstruction,
                        address,
                        raw_instruction,
                    )
                    .ok_or(ExecutionError::IllegalInstruction { address })?;
                match state
                    .instruction_fetcher
                    .set_pc(&state.memory, trap_pc)
                    .map_err(ExecutionError::from)?
                {
                    ControlFlow::Continue(()) => {
                        continue;
                    }
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            Err(error) => {
                if state
                    .memory
                    .tohost_value::<RegisterType<AbundanceRv32IMaxInstruction>>(elf.tohost_addr)?
                    .is_some()
                {
                    break;
                }
                return Err(error.into());
            }
        };

        let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
        let rs1rs2_values = Rs1Rs2OperandValues {
            rs1_value: state.regs.read(rs1),
            rs2_value: state.regs.read(rs2),
        };

        match instruction.execute(
            rs1rs2_values,
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        ) {
            Ok(ControlFlow::Continue((rd, rd_value))) => {
                state.regs.write(rd, rd_value);
                if state
                    .memory
                    .tohost_value::<RegisterType<AbundanceRv32IMaxInstruction>>(elf.tohost_addr)?
                    .is_some()
                {
                    break;
                }
            }
            Ok(ControlFlow::Break(())) => {
                break;
            }
            Err(error) => {
                if state
                    .memory
                    .tohost_value::<RegisterType<AbundanceRv32IMaxInstruction>>(elf.tohost_addr)?
                    .is_some()
                {
                    break;
                }
                return Err(error.into());
            }
        }
    }

    check_signature(&elf, &state.memory)
}

// TODO: It doesn't seem to be possible to make this generic over the instruction type at the moment
fn run_rv64i_max_test(
    elf_path: &Path,
) -> Result<(), TestError<RegisterType<AbundanceRv64IMaxInstruction>>> {
    let elf = ParsedElf::<<AbundanceRv64IMaxInstruction as Instruction>::Reg>::from_path(elf_path)?;

    let mut ram = Act4Memory::<RAM_BASE, RAM_SIZE>::new();
    for (vaddr, data) in &elf.segments {
        ram.write_slice(*vaddr, data)
            .map_err(ExecutionError::from)?;
    }

    let mut state = BasicInterpreterState {
        regs: BasicRegisters::default(),
        ext_state: AbundanceRv64IMaxExtState::new(),
        memory: ram,
        instruction_fetcher: BasicInstructionFetcher::<AbundanceRv64IMaxInstruction>::new(
            // Not used, setting to something that is unlikely to be used
            0, elf.entry,
        ),
        system_instruction_handler: Act4SystemHandler,
    };

    loop {
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory) {
            Ok(FetchInstructionResult::Instruction(instruction)) => instruction,
            Ok(FetchInstructionResult::ControlFlow(ControlFlow::Break(()))) => break,
            Ok(FetchInstructionResult::ControlFlow(ControlFlow::Continue(()))) => continue,
            // TODO: This custom handling is temporary until interpreter has abstractions and
            //  support for privileged instructions
            Err(ExecutionError::IllegalInstruction { address }) => {
                // Check for mret before treating as a trap - mret is a privileged instruction the
                // interpreter doesn't implement, so it arrives here as an illegal instruction
                let raw_instruction = state
                    .memory
                    .read::<u32>(address)
                    .map_err(ExecutionError::from)?;
                if raw_instruction == MRET_INSTRUCTION {
                    let mepc = state
                        .ext_state
                        .read_csr(MCsr::Mepc as u16)
                        .map_err(ExecutionError::from)?;
                    match state
                        .instruction_fetcher
                        .set_pc(&state.memory, mepc)
                        .map_err(ExecutionError::from)?
                    {
                        ControlFlow::Continue(()) => {
                            continue;
                        }
                        ControlFlow::Break(()) => {
                            break;
                        }
                    }
                }

                // All other illegal instructions dispatch through the trap handler
                let trap_pc = state
                    .ext_state
                    .take_trap(
                        MCauseException::IllegalInstruction,
                        address,
                        u64::from(raw_instruction),
                    )
                    .ok_or(ExecutionError::IllegalInstruction { address })?;
                match state
                    .instruction_fetcher
                    .set_pc(&state.memory, trap_pc)
                    .map_err(ExecutionError::from)?
                {
                    ControlFlow::Continue(()) => {
                        continue;
                    }
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            Err(error) => {
                if state
                    .memory
                    .tohost_value::<RegisterType<AbundanceRv64IMaxInstruction>>(elf.tohost_addr)?
                    .is_some()
                {
                    break;
                }
                return Err(error.into());
            }
        };

        let Rs1Rs2Operands { rs1, rs2 } = instruction.get_rs1_rs2_operands();
        let rs1rs2_values = Rs1Rs2OperandValues {
            rs1_value: state.regs.read(rs1),
            rs2_value: state.regs.read(rs2),
        };

        match instruction.execute(
            rs1rs2_values,
            &mut state.regs,
            &mut state.ext_state,
            &mut state.memory,
            &mut state.instruction_fetcher,
            &mut state.system_instruction_handler,
        ) {
            Ok(ControlFlow::Continue((rd, rd_value))) => {
                state.regs.write(rd, rd_value);
                if state
                    .memory
                    .tohost_value::<RegisterType<AbundanceRv64IMaxInstruction>>(elf.tohost_addr)?
                    .is_some()
                {
                    break;
                }
            }
            Ok(ControlFlow::Break(())) => {
                break;
            }
            Err(error) => {
                if state
                    .memory
                    .tohost_value::<RegisterType<AbundanceRv64IMaxInstruction>>(elf.tohost_addr)?
                    .is_some()
                {
                    break;
                }
                return Err(error.into());
            }
        }
    }

    check_signature(&elf, &state.memory)
}

fn check_signature<const RAM_BASE: u64, const RAM_SIZE: usize, Reg>(
    elf: &ParsedElf<Reg>,
    memory: &Act4Memory<RAM_BASE, RAM_SIZE>,
) -> Result<(), TestError<Reg::Type>>
where
    Reg: Register<Type: BasicInt>,
    [(); size_of::<Reg::Type>()]:,
{
    let Some(tohost) = memory.tohost_value::<Reg::Type>(elf.tohost_addr)? else {
        return Err(TestError::Test(anyhow::anyhow!(
            "Program never wrote `tohost`"
        )));
    };
    let tohost = tohost.as_u64();

    // Halt protocol is HTIF (Host-Target Interface) tohost write:
    //   `tohost == 1`: pass
    //   `tohost == (n << 1) | 1`: fail with exit code `n`
    if tohost != 1 {
        let detail = read_failure_info::<_, _, Reg::Type>(memory, elf.begin_failure_scratch)
            .unwrap_or_default();
        return Err(TestError::HtifFail {
            exit_code: tohost >> 1,
            detail,
        });
    }

    let expected_signature = elf.reference_signature()?;
    let sig_len = (elf.end_signature - elf.begin_signature) as u32;
    let actual_signature = match memory.read_slice(elf.begin_signature, sig_len) {
        Ok(actual_signature) => actual_signature,
        Err(error) => {
            return Err(TestError::Test(
                anyhow::Error::new(error).context("Failed to read signature"),
            ));
        }
    };

    if actual_signature.len() != expected_signature.len() {
        return Err(TestError::LengthMismatch {
            actual_bytes: actual_signature.len(),
            expected_bytes: expected_signature.len(),
        });
    }

    for (word, (actual, expected)) in actual_signature
        .iter()
        .copied()
        .array_chunks::<{ size_of::<Reg::Type>() }>()
        .map(|bytes| {
            // SAFETY: Correct size with all bit patterns being valid
            unsafe { bytes.as_ptr().cast::<Reg::Type>().read_unaligned() }
        })
        .zip(
            expected_signature
                .iter()
                .copied()
                .array_chunks::<{ size_of::<Reg::Type>() }>()
                .map(|bytes| {
                    // SAFETY: Correct size with all bit patterns being valid
                    unsafe { bytes.as_ptr().cast::<Reg::Type>().read_unaligned() }
                }),
        )
        .enumerate()
    {
        if actual != expected {
            return Err(TestError::SignatureMismatch {
                word,
                actual,
                expected,
            });
        }
    }

    Ok(())
}

fn collect_elf_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut elf_paths = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recurse and extend with all .elf files from the subdirectory
            let sub_paths = collect_elf_files(&path)?;
            elf_paths.extend(sub_paths);
        } else if path.extension().is_some_and(|e| e == "elf") {
            elf_paths.push(path);
        }
    }

    Ok(elf_paths)
}

fn process_error<RT>(
    error: TestError<RT>,
    hex_width: usize,
    stem: &str,
    failed: &mut usize,
    errors: &mut usize,
) where
    RT: RegType,
{
    match error {
        TestError::HtifFail { exit_code, detail } => {
            println!(
                "{} {stem} (HTIF exit code {exit_code}){detail}",
                "FAIL".red()
            );
            *failed += 1;
        }
        TestError::SignatureMismatch {
            word,
            actual,
            expected,
        } => {
            println!(
                "{} {stem} (sig word {word}: \
                    actual 0x{actual:0hex_width$x}, \
                    expected 0x{expected:0hex_width$x})",
                "FAIL".red()
            );
            *failed += 1;
        }
        TestError::LengthMismatch {
            actual_bytes,
            expected_bytes,
        } => {
            println!(
                "{} {stem} (sig length: \
                    actual {actual_bytes} bytes, \
                    expected {expected_bytes} bytes)",
                "FAIL".red()
            );
            *failed += 1;
        }
        TestError::Execution(error) => {
            println!("{} {stem} ({error})", "ERR".red());
            *errors += 1;
        }
        TestError::Test(error) => {
            println!("{} {stem} ({error})", "ERR".red());
            *errors += 1;
        }
    }
}

fn main() {
    let cli = Cli::parse();

    let mut elf_paths = collect_elf_files(&cli.elfs).expect("Failed to read --elfs directory");
    elf_paths.sort();

    if let Some(filter) = &cli.filter {
        elf_paths.retain(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(filter.as_str()))
        });
    }

    let total = elf_paths.len();
    let mut passed = 0_usize;
    let mut failed = 0_usize;
    let mut errors = 0_usize;

    for elf_path in &elf_paths {
        let stem = elf_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        match cli.isa {
            Isa::Rv32 => {
                let Err(error) = run_rv32i_max_test(elf_path) else {
                    println!("{} {stem}", "PASS".green());
                    passed += 1;
                    continue;
                };

                // 2 hex digits per byte
                let hex_width = size_of::<RegisterType<AbundanceRv32IMaxInstruction>>() * 2;

                process_error(error, hex_width, stem, &mut failed, &mut errors);
            }
            Isa::Rv64 => {
                let Err(error) = run_rv64i_max_test(elf_path) else {
                    println!("{} {stem}", "PASS".green());
                    passed += 1;
                    continue;
                };

                // 2 hex digits per byte
                let hex_width = size_of::<RegisterType<AbundanceRv64IMaxInstruction>>() * 2;

                process_error(error, hex_width, stem, &mut failed, &mut errors);
            }
        }

        if cli.fail_fast {
            break;
        }
    }

    println!(
        "\n{total} tests: {} passed, {} failed, {} errors",
        passed.to_string().green(),
        if failed > 0 {
            failed.to_string().red().to_string()
        } else {
            failed.to_string()
        },
        if errors > 0 {
            errors.to_string().red().to_string()
        } else {
            errors.to_string()
        }
    );

    if failed > 0 || errors > 0 {
        std::process::exit(1);
    }
}
