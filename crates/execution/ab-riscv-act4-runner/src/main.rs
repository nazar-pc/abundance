#![expect(incomplete_features, reason = "generic_const_exprs")]
#![feature(
    const_cmp,
    const_trait_impl,
    const_try,
    const_try_residual,
    generic_const_exprs,
    iter_array_chunks,
    try_blocks,
    widening_mul
)]

mod abundance_rv32i_max;
mod abundance_rv64i_max;
mod instruction;
mod interpreter;

use crate::abundance_rv32i_max::instruction::AbundanceRv32IMaxInstruction;
use crate::abundance_rv32i_max::interpreter::AbundanceRv32IMaxExtState;
use crate::abundance_rv64i_max::instruction::AbundanceRv64IMaxInstruction;
use crate::abundance_rv64i_max::interpreter::AbundanceRv64IMaxExtState;
use crate::interpreter::{Act4InstructionFetcher, Act4SystemHandler};
use ab_riscv_interpreter::{
    BasicInt, Csrs, ExecutableInstruction, ExecutionError, FetchInstructionResult,
    InstructionFetcher, InterpreterState, ProgramCounter, VirtualMemory,
};
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use clap::{Parser, ValueEnum};
use colored::Colorize;
use interpreter::Act4Memory;
use object::{Object, ObjectSegment, ObjectSymbol};
use std::fs;
use std::marker::PhantomData;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[cfg(not(target_endian = "little"))]
compile_error!("Only little-endian platforms are supported");

type RegisterType<I> = <<I as Instruction>::Reg as Register>::Type;

const RAM_BASE: u64 = 0x8000_0000;
const RAM_SIZE: usize = 4 * 1024 * 1024;
// TODO: This should be moved to primitives once privileged instructions are implemented
const MRET_INSTRUCTION: u32 = 0x30200073;

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
        for sym in elf.symbols() {
            match sym.name().unwrap_or("") {
                "tohost" => tohost_addr = Some(sym.address()),
                "begin_signature" => begin_signature = Some(sym.address()),
                "end_signature" => end_signature = Some(sym.address()),
                _ => {}
            }
        }
        let tohost_addr = tohost_addr.context("Symbol `tohost` not found")?;
        let begin_signature = begin_signature.context("Symbol `begin_signature` not found")?;
        let end_signature = end_signature.context("Symbol `end_signature` not found")?;

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

    let mut state = InterpreterState {
        regs: Registers::default(),
        ext_state: AbundanceRv32IMaxExtState::new(),
        memory: ram,
        instruction_fetcher: Act4InstructionFetcher::<AbundanceRv32IMaxInstruction>::new(elf.entry),
        system_instruction_handler: Act4SystemHandler,
        custom_error: PhantomData,
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

        match instruction.execute(&mut state) {
            Ok(ControlFlow::Continue(())) => {
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

    let mut state = InterpreterState {
        regs: Registers::default(),
        ext_state: AbundanceRv64IMaxExtState::new(),
        memory: ram,
        instruction_fetcher: Act4InstructionFetcher::<AbundanceRv64IMaxInstruction>::new(elf.entry),
        system_instruction_handler: Act4SystemHandler,
        custom_error: PhantomData,
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

        match instruction.execute(&mut state) {
            Ok(ControlFlow::Continue(())) => {
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
        return Err(TestError::HtifFail {
            exit_code: tohost >> 1,
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
            // SAFETY: Correct size with all bit pattens being valid
            unsafe { bytes.as_ptr().cast::<Reg::Type>().read_unaligned() }
        })
        .zip(
            expected_signature
                .iter()
                .copied()
                .array_chunks::<{ size_of::<Reg::Type>() }>()
                .map(|bytes| {
                    // SAFETY: Correct size with all bit pattens being valid
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
        } else if path.extension().map(|e| e == "elf").unwrap_or_default() {
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
        TestError::HtifFail { exit_code } => {
            println!("{} {stem} (HTIF exit code {exit_code})", "FAIL".red());
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
                .map(|n| n.contains(filter.as_str()))
                .unwrap_or_default()
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
