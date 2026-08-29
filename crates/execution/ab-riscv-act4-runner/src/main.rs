#![expect(incomplete_features, reason = "generic_const_*, explicit_tail_calls")]
#![feature(
    adt_const_params,
    const_cmp,
    const_trait_impl,
    const_try,
    const_try_residual,
    explicit_tail_calls,
    fn_align,
    generic_const_args,
    generic_const_items,
    inherent_associated_types,
    iter_array_chunks,
    macroless_generic_const_args,
    min_generic_const_args,
    signed_bigint_helpers,
    try_blocks
)]

mod abundance_rv32i_max;
mod abundance_rv64i_max;
mod instruction;
mod interpreter;

use crate::abundance_rv32i_max::AbundanceRv32IMaxInstruction;
use crate::abundance_rv64i_max::AbundanceRv64IMaxInstruction;
use crate::interpreter::TestEnv;
use ab_riscv_interpreter::basic::{
    BasicInstructionFetcher, BasicInterpreterState, BasicMemory, BasicRegister, BasicRegisters,
};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use clap::{Parser, ValueEnum};
use colored::Colorize;
use object::{Object, ObjectSegment, ObjectSymbol};
use std::ffi::CStr;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

#[cfg(not(target_endian = "little"))]
compile_error!("Only little-endian platforms are supported");

type RegisterType<I> = <<I as Instruction>::Reg as Register>::Type;

const RAM_BASE: u64 = 0x8000_0000;
const RAM_SIZE: usize = 0x0020_0000;
const MRET_INSTRUCTION: u32 = 0x3020_0073;

const SIZE_OF<T>: usize = size_of::<T>();

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
                _ => {
                    continue;
                }
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
enum TestError<Address>
where
    Address: Copy,
{
    HtifFail {
        exit_code: u64,
        detail: String,
    },
    SignatureMismatch {
        word: usize,
        actual: Address,
        expected: Address,
    },
    LengthMismatch {
        actual_bytes: usize,
        expected_bytes: usize,
    },
    Execution(ExecutionError<Address>),
    Test(anyhow::Error),
}

impl<Address> From<ExecutionError<Address>> for TestError<Address>
where
    Address: Copy,
{
    fn from(error: ExecutionError<Address>) -> Self {
        Self::Execution(error)
    }
}

impl<Address> From<anyhow::Error> for TestError<Address>
where
    Address: Copy,
{
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
        // `tohost` is always an 8-byte-wide HTIF field regardless of XLEN (RV32 writes it as two
        // 32-bit stores; RV64 as one 64-bit store). Its upper 32 bits carry the HTIF `device`/`cmd`
        // tag: zero for a plain pass/fail exit code (`1` for pass, `(n << 1) | 1` for fail),
        // nonzero for a device command such as console I/O (e.g. `RVMODEL_IO_WRITE_STR`'s
        // `device=1,cmd=1` character-output writes). Only a zero-tagged write is test
        // completion - a device write must not be mistaken for one, or the interpreter
        // would stop mid-test on the first character a failure message prints, rather than
        // actually reaching `RVMODEL_HALT_PASS`/ `_FAIL`.
        let raw_value = self
            .read::<u64>(tohost_addr)
            .context("Failed to read `tohost`")?;

        if raw_value == 0 || (raw_value >> 32) != 0 {
            return Ok(None);
        }

        let raw_value = RT::truncate_from_u64(raw_value);

        Ok(Some(raw_value))
    }
}

/// Checks whether `tohost` holds a confirmed test-completion value.
///
/// `tohost` completion is checked after every single instruction, but a completion-shaped write
/// (untagged, i.e. its upper 32 bits are 0 - see [`ToHost::tohost_value`]) can be *transient*: both
/// `RVMODEL_HALT_PASS`/`_FAIL` and `RVMODEL_IO_WRITE_STR` (console output) write `tohost` as two
/// separate stores - the payload (low word) first, the HTIF `device`/`cmd` tag (high word) second,
/// with an unrelated instruction (loading the tag constant) in between. Checking right after just
/// the first store of an `RVMODEL_IO_WRITE_STR` character (low word = the character, high word not
/// yet updated) can transiently look exactly like a plain, untagged completion write, if the high
/// word previously held 0 (i.e. this is the very first `tohost` write of the whole test). A real
/// HTIF host only samples `tohost` asynchronously and would never observe that torn intermediate
/// state.
///
/// So a completion-shaped reading is only trusted once it has also been seen, unchanged, on the
/// immediately preceding call (tracked via `pending_tohost`, which the caller carries across calls
/// for the lifetime of one test run): the real halt sequence writes the same value on every one of
/// its (`sw`, `sw`, `jal` back to the start) loop iterations forever, so it is always eventually
/// confirmed on some later pair of consecutive calls, while a transient torn read is overwritten by
/// the tag word within a single intervening instruction and so never repeats.
fn check_tohost(
    memory: &BasicMemory<RAM_BASE, RAM_SIZE>,
    tohost_addr: u64,
    pending_tohost: &mut Option<u64>,
) -> anyhow::Result<bool> {
    let raw_tohost = memory
        .read::<u64>(tohost_addr)
        .context("Failed to read `tohost`")?;

    let completion_shaped = raw_tohost != 0 && (raw_tohost >> 32) == 0;
    if !completion_shaped {
        *pending_tohost = None;
        return Ok(false);
    }

    let confirmed = *pending_tohost == Some(raw_tohost);
    *pending_tohost = Some(raw_tohost);

    Ok(confirmed)
}

/// Read the raw encoding of an (illegal) instruction at `address`, for use as `mtval`.
///
/// Must not blindly read 4 bytes: with Zca, `address` only needs to be 2-byte aligned, and a
/// 32-bit-wide read starting there would pull in the low halfword of the *next*, unrelated
/// instruction whenever `address` isn't also 4-byte aligned. Reads the low halfword first and
/// only widens to the full word when its low bits (`0b11`) say this is a 32-bit encoding -
/// matching the same check the interpreter's own decoder uses to size an instruction, and what
/// `REPORT_ENCODING_IN_MTVAL_ON_ILLEGAL_INSTRUCTION` expects `mtval` to reflect for a compressed
/// illegal instruction: the zero-extended halfword, not 32 bits of partly foreign encoding.
fn read_raw_instruction<const RAM_BASE: u64, const RAM_SIZE: usize>(
    memory: &BasicMemory<RAM_BASE, RAM_SIZE>,
    address: u64,
) -> Result<u32, VirtualMemoryError> {
    let low = memory.read::<u16>(address)?;
    if (low & 0b11) != 0b11 {
        return Ok(u32::from(low));
    }
    let high = memory.read::<u16>(address + 2)?;
    Ok(u32::from(low) | (u32::from(high) << 16))
}

fn read_cstring<const RAM_BASE: u64, const RAM_SIZE: usize>(
    memory: &BasicMemory<RAM_BASE, RAM_SIZE>,
    addr: u64,
) -> Option<&str> {
    let slice = memory.read_slice_up_to(addr, 512);
    CStr::from_bytes_until_nul(slice).ok()?.to_str().ok()
}

fn read_failure_info<const RAM_BASE: u64, const RAM_SIZE: usize, RT>(
    memory: &BasicMemory<RAM_BASE, RAM_SIZE>,
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

/// Resolves the result of `BasicInstructionFetcher::set_pc[_relative]()` into the `ControlFlow` it
/// signals.
///
/// `set_pc[_relative]()` rejects a misaligned target with `ExecutionError::UnalignedInstruction`
/// instead of moving the program counter there. That is an instruction-address-misaligned
/// exception on real hardware, not a hard interpreter error - dispatch through the trap handler
/// exactly like the illegal-instruction case in [`run_test()`]. Per
/// `REPORT_VA_IN_MTVAL_ON_INSTRUCTION_MISALIGNED` (`false`) in the DUT config, `mtval` is left at
/// zero rather than the address. The recursive `set_pc()` call for the trap handler's own entry
/// point can't itself be misaligned (`mtvec` is masked to `MTVEC_BASE_ALIGNMENT_DIRECT` on every
/// write), so it's fine to propagate its result with a bare `?`.
fn resolve_pc_result<I, const ELEN: Elen, const VLEN: Vlen>(
    result: Result<ControlFlow<()>, ExecutionError<RegisterType<I>>>,
    env: &mut TestEnv<I::Reg, ELEN, VLEN>,
    memory: &BasicMemory<RAM_BASE, RAM_SIZE>,
    instruction_fetcher: &mut BasicInstructionFetcher<I>,
) -> Result<ControlFlow<()>, TestError<RegisterType<I>>>
where
    I: Instruction<Reg: BasicRegister<Type: BasicInt>>,
    TestEnv<I::Reg, ELEN, VLEN>: VectorRegistersExt<I::Reg>,
{
    match result {
        Ok(control_flow) => Ok(control_flow),
        Err(ExecutionError::UnalignedInstruction { address }) => {
            let address = address.get();
            let trap_pc = env
                .take_trap(
                    MCauseException::InstructionAddressMisaligned,
                    address,
                    RegisterType::<I>::default(),
                )
                .ok_or(ExecutionError::UnalignedInstruction {
                    address: PackedAddress::new(address),
                })?;
            Ok(instruction_fetcher.set_pc(memory, trap_pc)?)
        }
        Err(error) => Err(error.into()),
    }
}

fn run_test<I, const ELEN: Elen, const VLEN: Vlen>(
    elf_path: &Path,
) -> Result<(), TestError<RegisterType<I>>>
where
    I: ExecutableInstruction<
            BasicRegisters<<I as Instruction>::Reg>,
            TestEnv<<I as Instruction>::Reg, ELEN, VLEN>,
            Box<BasicMemory<RAM_BASE, RAM_SIZE>>,
            BasicInstructionFetcher<I>,
            Reg: BasicRegister<Type: BasicInt>,
        >,
    TestEnv<<I as Instruction>::Reg, ELEN, VLEN>: VectorRegistersExt<<I as Instruction>::Reg>,
{
    let elf = ParsedElf::<I::Reg>::from_path(elf_path)?;

    let mut ram = BasicMemory::<RAM_BASE, RAM_SIZE>::new_boxed();
    for (vaddr, data) in &elf.segments {
        ram.write_slice(*vaddr, data)
            .map_err(ExecutionError::from)?;
    }

    let mut state = BasicInterpreterState {
        regs: BasicRegisters::<I::Reg>::default(),
        env: TestEnv::new(),
        memory: ram,
        instruction_fetcher: BasicInstructionFetcher::<I>::new(
            // Not used by this harness (termination is always via `tohost`), so this only needs
            // to be a value no test will ever actually jump to. `0` doesn't qualify: it is
            // `RVMODEL_ACCESS_FAULT_ADDRESS` (see rvmodel_macros.h), the conventional "guaranteed
            // to fault" address ACT4 tests deliberately jump to (e.g. ExceptionsSm's instruction-
            // access-fault coverpoint) - hitting it must produce a real access fault, not silently
            // stop the interpreter. Use all-ones instead, since ACT4 conventionally targets low
            // addresses like `0` as "guaranteed bad", never the top of the address space.
            !RegisterType::<I>::default(),
            elf.entry,
        ),
    };

    // Carried across `check_tohost()` calls for the lifetime of this test run - see its doc comment
    let mut pending_tohost = None;

    let ab_trace = std::env::var_os("AB_TRACE").is_some();
    loop {
        // Checked unconditionally every iteration (not just after a `Continue`/`ContinueNoWrite`
        // execution result): the final halt loop is sometimes a pure `jal x0, self` spin with no
        // further memory writes, which never lands in either of those two arms below.
        if check_tohost(&state.memory, elf.tohost_addr, &mut pending_tohost)? {
            break;
        }
        if ab_trace {
            let pc =
                ProgramCounter::<RegisterType<I>, Box<BasicMemory<RAM_BASE, RAM_SIZE>>>::get_pc(
                    &state.instruction_fetcher,
                );
            eprintln!("pc={:#x}", pc.as_u64());
        }
        let instruction = match state.instruction_fetcher.fetch_instruction(&state.memory) {
            FetchInstructionResult::Instruction(instruction) => instruction,
            FetchInstructionResult::Break => {
                break;
            }
            FetchInstructionResult::Continue => {
                continue;
            }
            // TODO: This custom handling is temporary until interpreter has abstractions and
            //  support for privileged instructions
            FetchInstructionResult::Err(ExecutionError::IllegalInstruction { address }) => {
                let address = address.get();
                // Check for mret before treating as a trap - mret is a privileged instruction the
                // interpreter doesn't implement, so it arrives here as an illegal instruction
                let raw_instruction = read_raw_instruction(&state.memory, address.as_u64())
                    .map_err(ExecutionError::from)?;
                if raw_instruction == MRET_INSTRUCTION {
                    let mepc = state.env.return_from_trap();
                    match resolve_pc_result::<I, ELEN, VLEN>(
                        state.instruction_fetcher.set_pc(&state.memory, mepc),
                        &mut state.env,
                        &state.memory,
                        &mut state.instruction_fetcher,
                    )? {
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
                    .env
                    .take_trap(
                        MCauseException::IllegalInstruction,
                        address,
                        RegisterType::<I>::from(raw_instruction),
                    )
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(address),
                    })?;
                match resolve_pc_result::<I, ELEN, VLEN>(
                    state.instruction_fetcher.set_pc(&state.memory, trap_pc),
                    &mut state.env,
                    &state.memory,
                    &mut state.instruction_fetcher,
                )? {
                    ControlFlow::Continue(()) => {
                        continue;
                    }
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            // Out-of-bounds instruction fetch (e.g. a jump to an address outside RAM, such as
            // ACT4's conventional `RVMODEL_ACCESS_FAULT_ADDRESS`) is an instruction access fault
            // on real hardware, not a hard interpreter error - dispatch through the trap handler.
            // `mtval` carries the faulting address per
            // `REPORT_VA_IN_MTVAL_ON_INSTRUCTION_ACCESS_FAULT=true` in the DUT config.
            FetchInstructionResult::Err(ExecutionError::OutOfBoundsRead { address }) => {
                let address = address.get();
                let epc = RegisterType::<I>::truncate_from_u64(address);
                let trap_pc = state
                    .env
                    .take_trap(MCauseException::InstructionAccessFault, epc, epc)
                    .ok_or(ExecutionError::OutOfBoundsRead {
                        address: PackedAddress::new(address),
                    })?;
                match resolve_pc_result::<I, ELEN, VLEN>(
                    state.instruction_fetcher.set_pc(&state.memory, trap_pc),
                    &mut state.env,
                    &state.memory,
                    &mut state.instruction_fetcher,
                )? {
                    ControlFlow::Continue(()) => {
                        continue;
                    }
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            FetchInstructionResult::Err(error) => {
                if check_tohost(&state.memory, elf.tohost_addr, &mut pending_tohost)? {
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

        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not need other fields"
        )]
        match instruction.execute(
            rs1rs2_values,
            &mut state.regs,
            &mut state.env,
            &mut state.memory,
            &mut state.instruction_fetcher,
        ) {
            ExecutionResult::Continue { rd, value } => {
                state.regs.write(rd, value);
                if check_tohost(&state.memory, elf.tohost_addr, &mut pending_tohost)? {
                    break;
                }
            }
            ExecutionResult::ContinueNoWrite => {
                if check_tohost(&state.memory, elf.tohost_addr, &mut pending_tohost)? {
                    break;
                }
            }
            ExecutionResult::Branch { offset } => {
                match resolve_pc_result::<I, ELEN, VLEN>(
                    state.instruction_fetcher.set_pc_relative(
                        &state.memory,
                        instruction.size(),
                        offset,
                    ),
                    &mut state.env,
                    &state.memory,
                    &mut state.instruction_fetcher,
                )? {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            ExecutionResult::Jump { target } => {
                match resolve_pc_result::<I, ELEN, VLEN>(
                    state.instruction_fetcher.set_pc(&state.memory, target),
                    &mut state.env,
                    &state.memory,
                    &mut state.instruction_fetcher,
                )? {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            ExecutionResult::Break => {
                break;
            }
            // TODO: This custom handling is temporary until interpreter has abstractions and
            //  support for privileged instructions
            //
            // Two distinct error shapes land here and both dispatch through the trap handler
            // exactly like the decode-time illegal-instruction case above:
            // - CsrError: illegal/unauthorized CSR access (e.g. Zkr's `seed` pure-read restriction)
            //   is an illegal-instruction exception per the privileged spec - there is no dedicated
            //   CSR-error trap cause. It carries no address, so it is recovered from the fetcher's
            //   old PC.
            // - IllegalInstruction: a successfully *decoded* instruction whose execution is
            //   illegal, currently only `ecall` (`TestEnv` always rejects it - the interpreter has
            //   no other way to support it yet). Unlike the decode-time case, this one already
            //   carries the correct address.
            ExecutionResult::Err(
                error @ (ExecutionError::CsrReadOnly { .. }
                | ExecutionError::CsrIllegalRead { .. }
                | ExecutionError::CsrIllegalWrite { .. }
                | ExecutionError::CsrUnknown { .. }
                | ExecutionError::CsrInsufficientPrivilege { .. }
                | ExecutionError::IllegalInstruction { .. }),
            ) => {
                let address = match error {
                    ExecutionError::IllegalInstruction { address } => address.get(),
                    _ => ProgramCounter::<
                        RegisterType<I>,
                        Box<BasicMemory<RAM_BASE, RAM_SIZE>>,
                    >::old_pc(
                        &state.instruction_fetcher, instruction.size()
                    ),
                };
                let raw_instruction = read_raw_instruction(&state.memory, address.as_u64())
                    .map_err(ExecutionError::from)?;
                let trap_pc = state
                    .env
                    .take_trap(
                        MCauseException::IllegalInstruction,
                        address,
                        RegisterType::<I>::from(raw_instruction),
                    )
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(address),
                    })?;
                match resolve_pc_result::<I, ELEN, VLEN>(
                    state.instruction_fetcher.set_pc(&state.memory, trap_pc),
                    &mut state.env,
                    &state.memory,
                    &mut state.instruction_fetcher,
                )? {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            // Out-of-bounds memory accesses (e.g. a vector indexed load/store whose computed
            // effective address wraps or otherwise lands outside of RAM) are load/store access
            // faults on real hardware, not a hard interpreter error - dispatch through the trap
            // handler exactly like the illegal-instruction case above. `mtval` carries the
            // faulting virtual address, per `REPORT_VA_IN_MTVAL_ON_{LOAD,STORE_AMO}_ACCESS_FAULT`
            // in the DUT config.
            //
            // `lr`/`sc` additionally require natural alignment (unlike ordinary loads/stores,
            // which this core allows misaligned per `MISALIGNED_LDST`): the Zalrsc extension
            // mandates that a misaligned `lr`/`sc` always raises an exception, and this DUT
            // config's `LRSC_MISALIGNED_BEHAVIOR: "always raise misaligned exception"` picks an
            // address-misaligned exception over an access fault for it - a different `mcause`
            // than an out-of-bounds access, and per `REPORT_VA_IN_MTVAL_ON_{LOAD,STORE_AMO}
            // _MISALIGNED` (both `false`) `mtval` is left at zero rather than the address.
            ExecutionResult::Err(
                error @ (ExecutionError::OutOfBoundsRead { .. }
                | ExecutionError::OutOfBoundsWrite { .. }
                | ExecutionError::MisalignedRead { .. }
                | ExecutionError::MisalignedWrite { .. }),
            ) => {
                let (cause, tval) = match error {
                    ExecutionError::OutOfBoundsRead { address } => (
                        MCauseException::LoadAccessFault,
                        RegisterType::<I>::truncate_from_u64(address.get()),
                    ),
                    ExecutionError::OutOfBoundsWrite { address } => (
                        MCauseException::StoreAccessFault,
                        RegisterType::<I>::truncate_from_u64(address.get()),
                    ),
                    ExecutionError::MisalignedRead { .. } => (
                        MCauseException::LoadAddressMisaligned,
                        RegisterType::<I>::default(),
                    ),
                    ExecutionError::MisalignedWrite { .. } => (
                        MCauseException::StoreAddressMisaligned,
                        RegisterType::<I>::default(),
                    ),
                    _ => unreachable!(),
                };
                let epc =
                    ProgramCounter::<RegisterType<I>, Box<BasicMemory<RAM_BASE, RAM_SIZE>>>::old_pc(
                        &state.instruction_fetcher,
                        instruction.size(),
                    );
                let trap_pc = state.env.take_trap(cause, epc, tval).ok_or(error)?;
                match resolve_pc_result::<I, ELEN, VLEN>(
                    state.instruction_fetcher.set_pc(&state.memory, trap_pc),
                    &mut state.env,
                    &state.memory,
                    &mut state.instruction_fetcher,
                )? {
                    ControlFlow::Continue(()) => {}
                    ControlFlow::Break(()) => {
                        break;
                    }
                }
            }
            ExecutionResult::Err(error) => {
                if check_tohost(&state.memory, elf.tohost_addr, &mut pending_tohost)? {
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
    memory: &BasicMemory<RAM_BASE, RAM_SIZE>,
) -> Result<(), TestError<Reg::Type>>
where
    Reg: Register<Type: BasicInt>,
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
        .array_chunks::<{ SIZE_OF::<Reg::Type> }>()
        .map(|bytes| {
            // SAFETY: Correct size with all bit patterns being valid
            unsafe { bytes.as_ptr().cast::<Reg::Type>().read_unaligned() }
        })
        .zip(
            expected_signature
                .iter()
                .copied()
                .array_chunks::<{ SIZE_OF::<Reg::Type> }>()
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

/// Run a single test and print its outcome, returning whether the test passed
fn run_and_report<I, const ELEN: Elen, const VLEN: Vlen>(
    elf_path: &Path,
    stem: &str,
    passed: &mut usize,
    failed: &mut usize,
    errors: &mut usize,
) -> bool
where
    I: ExecutableInstruction<
            BasicRegisters<<I as Instruction>::Reg>,
            TestEnv<<I as Instruction>::Reg, ELEN, VLEN>,
            Box<BasicMemory<RAM_BASE, RAM_SIZE>>,
            BasicInstructionFetcher<I>,
            Reg: BasicRegister<Type: BasicInt>,
        >,
    TestEnv<<I as Instruction>::Reg, ELEN, VLEN>: VectorRegistersExt<<I as Instruction>::Reg>,
{
    let Err(error) = run_test::<I, ELEN, VLEN>(elf_path) else {
        println!("{} {stem}", "PASS".green());
        *passed += 1;
        return true;
    };

    // 2 hex digits per byte
    let hex_width = size_of::<RegisterType<I>>() * 2;

    process_error(error, hex_width, stem, failed, errors);

    false
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

        let test_passed = match cli.isa {
            Isa::Rv32 => run_and_report::<
                AbundanceRv32IMaxInstruction,
                { Elen::L64 },
                { Vlen::L1024 },
            >(elf_path, stem, &mut passed, &mut failed, &mut errors),
            Isa::Rv64 => run_and_report::<
                AbundanceRv64IMaxInstruction,
                { Elen::L64 },
                { Vlen::L1024 },
            >(elf_path, stem, &mut passed, &mut failed, &mut errors),
        };

        if !test_passed && cli.fail_fast {
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
