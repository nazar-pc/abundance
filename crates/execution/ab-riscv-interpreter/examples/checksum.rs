//! Composing an instruction set and an execution environment: RV64IMAC with a syscall.
//!
//! Three things come together here, and each of them is a place where the crate expects to be
//! extended:
//! * [`ChecksumInstruction`] is an instruction set assembled out of the base ISA and the M, A and C
//!   (as Zca) extensions with the `#[instruction]` macro. The macro needs
//!   `ab_riscv_macros::process_instruction_macros()` to be called from `build.rs` of the crate the
//!   composition lives in, and `links` to be set in its `Cargo.toml`, both of which
//!   `ab-riscv-interpreter` already does
//! * [`Env`] is the execution environment. Extensions constrain it: the A extension needs a
//!   [`ReservationSet`] for `lr`/`sc`, and every instruction set that contains `ecall` needs a
//!   [`SystemInstructionHandler`]. `IllegalEcallSystemInstructionHandler` of the [`basic`] module
//!   implements both, but only usefully for a guest that never makes a syscall
//! * with Zca in the set, instructions are 2 or 4 bytes long and may start at any even address,
//!   which the composed `Instruction::ALIGNMENT` reflects automatically: it is the smallest of the
//!   alignments of the inherited instruction sets
//!
//! [`basic`]: ab_riscv_interpreter::basic

#![expect(incomplete_features, reason = "explicit_tail_calls")]
#![feature(
    const_trait_impl,
    const_try,
    const_try_residual,
    explicit_tail_calls,
    fn_align,
    signed_bigint_helpers,
    try_blocks
)]

use ab_riscv_interpreter::basic::{
    BasicInstructionFetcher, BasicInterpreterState, BasicMemory, BasicRegisters,
};
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use object::{Object, ObjectSection};
use std::fmt;
use std::ops::ControlFlow;

/// The guest program, see `examples/guests` in the repository
const GUEST_ELF: &[u8] = include_bytes!("prebuilt/checksum.elf");
/// FNV-1a offset basis, the same the guest starts from
const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime, the same the guest multiplies by
const PRIME: u64 = 0x0000_0100_0000_01b3;
/// The syscall with which the guest asks for the `u64` in `a0` to be printed, see
/// `examples/guests/src/bin/checksum.rs`
const HOST_CALL_REPORT: u64 = 1;
/// Guest memory base address, which is where the guest ELF is linked to be loaded
const MEMORY_BASE_ADDRESS: u64 = 0x1_0000;
/// Guest memory size, generous enough for the program, its stack, and the input buffer
const MEMORY_SIZE: usize = 64 * 1024;
/// Address at which the interpreter stops execution gracefully
const TRAP_ADDRESS: u64 = 0;
/// Where the host puts the bytes to be checksummed, past the program image and below the stack
const INPUT_ADDRESS: u64 = MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64 / 2;
/// How many bytes there are to checksum
const INPUT_LENGTH: usize = 4096;
/// Stack pointer at the top of guest memory, 16-byte aligned as the psABI requires
const STACK_POINTER: u64 = (MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64) & !0xf;

/// Register type of the composed instruction set
type ChecksumRegister = Reg<u64>;

/// RV64IMAC, assembled out of the base ISA and the extensions the guest is compiled for
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        Rv64AInstruction,
        Rv64ZcaInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChecksumInstruction<Reg = ChecksumRegister> {}

#[instruction]
const impl<Reg> Instruction for ChecksumInstruction<Reg> {
    const ALIGNMENT: u8 = align_of::<u32>() as u8;

    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for ChecksumInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for ChecksumInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Env> ExecutableInstructionCsr<Env> for ChecksumInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ChecksumInstruction<Reg>
where
    Reg: Register,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}

/// Execution environment of this example.
///
/// Whatever the instructions of the composed set need beyond registers and memory ends up here,
/// which for RV64IMAC is the reservation of `lr`/`sc` and the handling of `ecall`.
#[derive(Debug, Default)]
struct Env {
    /// Address the guest currently holds a reservation on, if any
    reservation: Option<u64>,
}

impl ReservationSet<Reg<u64>> for Env {
    fn reservation(&self) -> Option<u64> {
        self.reservation
    }

    fn set_reservation(&mut self, address: u64) {
        self.reservation = Some(address);
    }

    fn clear_reservation(&mut self) {
        self.reservation = None;
    }
}

impl<Regs, Memory, PC> SystemInstructionHandler<Reg<u64>, Regs, Memory, PC> for Env
where
    Regs: RegisterFile<Reg<u64>>,
    PC: ProgramCounter<u64, Memory>,
{
    fn handle_ecall(
        &mut self,
        regs: &mut Regs,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        // Nothing about `ecall` says where its operands are. This is the convention the guest was
        // written against, see `examples/guests/src/bin/checksum.rs`.
        match regs.read(Reg::A7) {
            HOST_CALL_REPORT => {
                println!("Guest reported: {:#018x}", regs.read(Reg::A0));

                Ok(ControlFlow::Continue(()))
            }
            _ => Err(ExecutionError::EcallUnsupported {
                address: PackedAddress::new(program_counter.old_pc(size_of::<u32>() as u8)),
            }),
        }
    }
}

/// FNV-1a hash of `data`, the same thing the guest computes
fn checksum(data: &[u8]) -> u64 {
    let mut hash = OFFSET_BASIS;

    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }

    hash
}

fn main() -> anyhow::Result<()> {
    let input = (0..INPUT_LENGTH)
        .map(|index| index as u8)
        .collect::<Vec<_>>();

    let mut memory = BasicMemory::<MEMORY_BASE_ADDRESS, MEMORY_SIZE>::default();

    // The guest is statically linked and has no relocations, so loading it is nothing but copying
    // every section that has an address to that address
    let elf = object::File::parse(GUEST_ELF).context("Failed to parse guest ELF")?;
    for section in elf.sections().filter(|section| section.address() != 0) {
        let address = section.address();
        let data = section.data().context("Failed to read guest ELF section")?;
        memory
            .write_slice(address, data)
            .map_err(anyhow::Error::from)
            .with_context(|| format!("Section at {address:#x} does not fit into guest memory"))?;
    }

    memory
        .write_slice(INPUT_ADDRESS, &input)
        .map_err(anyhow::Error::from)
        .context("Input does not fit into guest memory")?;

    let mut regs = BasicRegisters::<Reg<u64>>::default();
    regs.write(Reg::Ra, TRAP_ADDRESS);
    regs.write(Reg::Sp, STACK_POINTER);
    regs.write(Reg::A0, INPUT_ADDRESS);
    regs.write(Reg::A1, input.len() as u64);

    let mut state = BasicInterpreterState {
        regs,
        env: Env::default(),
        memory,
        instruction_fetcher: BasicInstructionFetcher::<ChecksumInstruction>::new(
            TRAP_ADDRESS,
            elf.entry(),
        ),
    };

    state
        .execute::<ChecksumInstruction>()
        .context("Guest execution failed")?;

    let expected = checksum(&input);
    assert_eq!(state.regs.read(Reg::A0), expected);

    Ok(())
}
