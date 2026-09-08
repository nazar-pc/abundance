//! Vector extensions, and the execution environment they ask for.
//!
//! Everything the previous examples needed from an environment was small enough to fit in a couple
//! of trait methods. Vectors are where that stops being true: the instruction set needs a vector
//! register file, the CSRs that describe how it is currently interpreted (`vtype`, `vl`,
//! `vstart`, ...), and it needs them to start out in the state the specification calls for. That
//! is what [`Env`] below is, and it is the interesting part of this example.
//!
//! Two things are configuration rather than instructions, which is why they are constants of
//! [`VectorRegisters`] instead of inherited instruction sets: `ELEN`, the widest element the
//! implementation supports, and `VLEN`, how wide a vector register is. `Zve64x` and `Zvl128b`,
//! which the guest was compiled for, are exactly those two numbers.
//!
//! The guest itself is a plain `for` loop over a slice, see
//! `examples/guests/src/bin/vector-sum.rs`: compiled for a target with a vector extension, LLVM
//! turns it into a strip-mined vector loop that reconfigures `vl` on every iteration, so this runs
//! the real thing rather than a hand-written sequence of vector instructions.

#![expect(incomplete_features, reason = "explicit_tail_calls")]
#![feature(
    const_cmp,
    const_trait_impl,
    const_try,
    const_try_residual,
    explicit_tail_calls,
    fn_align,
    generic_const_args,
    generic_const_items,
    inherent_associated_types,
    macroless_generic_const_args,
    min_generic_const_args,
    signed_bigint_helpers,
    try_blocks,
    variant_count
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
use std::mem::variant_count;
use std::ops::ControlFlow;

/// The guest program, see `examples/guests` in the repository
const GUEST_ELF: &[u8] = include_bytes!("prebuilt/vector-sum.elf");
/// Guest memory base address, which is where the guest ELF is linked to be loaded
const MEMORY_BASE_ADDRESS: u64 = 0x1_0000;
/// Guest memory size, generous enough for the program, its stack, and the input array
const MEMORY_SIZE: usize = 64 * 1024;
/// Address at which the interpreter stops execution gracefully
const TRAP_ADDRESS: u64 = 0;
/// Where the host puts the array to be summed, past the program image and below the stack
const INPUT_ADDRESS: u64 = MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64 / 2;
/// Stack pointer at the top of guest memory, 16-byte aligned as the psABI requires
const STACK_POINTER: u64 = (MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64) & !0xf;
/// How many `u64` values the guest is asked to sum
const INPUT_LENGTH: usize = 1024;
/// How many vector CSRs there are: `vstart`, `vxsat`, `vxrm`, `vcsr`, `vl`, `vtype` and `vlenb`
const VECTOR_CSRS: usize = variant_count::<VectorCsr>();

/// Register type of the composed instruction set
type VectorSumRegister = Reg<u64>;

/// The base ISA, multiplication and the vector instruction set.
///
/// `ZveXxInstruction` brings `ZicsrInstruction` with it, since the vector CSRs are read and written
/// like any other.
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        ZveXxInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VectorSumInstruction<Reg = VectorSumRegister> {}

#[instruction]
const impl<Reg> Instruction for VectorSumInstruction<Reg> {
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
impl<Reg> fmt::Display for VectorSumInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for VectorSumInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Env> ExecutableInstructionCsr<Env> for VectorSumInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for VectorSumInstruction<Reg>
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
/// A vector instruction set needs three things from it: the vector CSRs, which [`Csrs`] provides,
/// the vector register file and the few pieces of state that have no CSR of their own, which
/// [`VectorRegisters`] provides, and `ecall` handling, which the base ISA needs regardless.
///
/// [`VectorRegistersExt`] on top of those is where the vector CSRs get their meaning: it decodes
/// and encodes `vtype`, `vl` and friends out of the raw values [`Csrs`] stores, so an environment
/// only has to provide storage. Its default implementations are what [`Env::new()`] uses to bring
/// the vector state up in the reset configuration the specification describes.
#[derive(Debug)]
struct Env {
    /// Raw values of the vector CSRs, indexed by [`VectorCsr`]
    csrs: [u64; VECTOR_CSRS],
    /// The vector register file itself
    vregs: VectorRegisterFile<{ Vlen::L128 }>,
}

impl Csrs<Reg<u64>> for Env {
    fn read_csr(&self, csr_index: u16) -> Result<u64, CsrError> {
        let csr =
            VectorCsr::from_csr_index(csr_index).ok_or(CsrError::IllegalRead { csr_index })?;

        Ok(self.csrs[csr as usize])
    }

    fn write_csr(&mut self, csr_index: u16, value: u64) -> Result<(), CsrError> {
        let csr =
            VectorCsr::from_csr_index(csr_index).ok_or(CsrError::IllegalWrite { csr_index })?;
        self.csrs[csr as usize] = value;

        Ok(())
    }
}

impl VectorRegisters for Env {
    /// The widest element the guest may ask for, the `64` of `Zve64x`
    const ELEN: Elen = Elen::L64;
    /// How wide a vector register is, the `128` of `Zvl128b`
    const VLEN: Vlen = Vlen::L128;

    fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }> {
        &self.vregs
    }

    fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }> {
        &mut self.vregs
    }

    fn vector_instructions_allowed(&self) -> bool {
        // There is no `mstatus` here to turn vector instructions off through
        true
    }

    fn mark_vs_dirty(&mut self) {
        // Nothing to mark dirty without `mstatus` either
    }
}

impl VectorRegistersExt<Reg<u64>> for Env {}

impl<Regs, Memory, PC> SystemInstructionHandler<Reg<u64>, Regs, Memory, PC> for Env
where
    PC: ProgramCounter<u64, Memory>,
{
    fn handle_ecall(
        &mut self,
        _regs: &mut Regs,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> Result<ControlFlow<()>, ExecutionError<u64>> {
        Err(ExecutionError::EcallUnsupported {
            address: PackedAddress::new(program_counter.old_pc(size_of::<u32>() as u8)),
        })
    }
}

impl Env {
    /// Create an environment with the vector state in its reset configuration
    fn new() -> Self {
        let mut env = Self {
            csrs: [0; _],
            vregs: VectorRegisterFile::default(),
        };

        // `vlenb` is not state, it reports `VLEN` in bytes and never changes
        env.csrs[VectorCsr::Vlenb as usize] = u64::from(Self::VLEN.bytes());
        // `vtype` starts out invalid and `vl` zero, so that a guest has to configure them with
        // `vsetvl{i}` before it may execute anything else
        env.initialize_vector_state();

        env
    }
}

fn main() -> anyhow::Result<()> {
    let input = (0..INPUT_LENGTH as u64)
        .map(|index| index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
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

    let input_bytes = input
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    memory
        .write_slice(INPUT_ADDRESS, &input_bytes)
        .map_err(anyhow::Error::from)
        .context("Input does not fit into guest memory")?;

    let mut regs = BasicRegisters::<Reg<u64>>::default();
    regs.write(Reg::Ra, TRAP_ADDRESS);
    regs.write(Reg::Sp, STACK_POINTER);
    regs.write(Reg::A0, INPUT_ADDRESS);
    regs.write(Reg::A1, input.len() as u64);

    let mut state = BasicInterpreterState {
        regs,
        env: Env::new(),
        memory,
        instruction_fetcher: BasicInstructionFetcher::<VectorSumInstruction>::new(
            TRAP_ADDRESS,
            elf.entry(),
        ),
    };

    state
        .execute::<VectorSumInstruction>()
        .context("Guest execution failed")?;

    let sum = state.regs.read(Reg::A0);
    println!("Guest summed {INPUT_LENGTH} values into {sum:#018x}");

    let expected = input.iter().copied().fold(0, u64::wrapping_add);
    assert_eq!(sum, expected);

    Ok(())
}
