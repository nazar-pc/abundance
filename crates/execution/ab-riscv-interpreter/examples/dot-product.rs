//! Composing for speed: eager decoding, threaded dispatch, an environment built for the hot path.
//!
//! `vector-sum` shows what a vector instruction set asks of an execution environment in the most
//! straightforward way: the vector CSRs are kept in the raw form a `csrr` returns. [`Env`] here
//! keeps every one of them decoded instead, in the type that describes it, and overrides the
//! [`VectorRegistersExt`] accessors to hand that decoded form straight to the instructions. They
//! ask for `vtype` and `vl` on every single vector instruction and go through a CSR instruction
//! only rarely, so this is the side of the trade worth being on.
//!
//! The rest of the composition is built out of the [`basic`] module:
//! * [`BasicEagerInstructions`] decodes the whole code section once and hands out a fetcher that
//!   walks the decoded stream, so fetching skips the bounds and alignment checks
//!   `BasicInstructionFetcher` does on every instruction. That is what the `unsafe` on its
//!   constructors is about: the program has to be known to end with an unconditional jump and to
//!   never write into its own code
//! * [`ThreadedExecutableInstruction::execute_threaded()`] walks that stream with each instruction
//!   tail-calling the handler of the next one, instead of returning to a central `match`
//! * the register file is a part of that composition rather than an afterthought: `ZEROSTORE`
//!   throws away writes to `x0` instead of checking for it on every read, which is what threaded
//!   dispatch wants, since there is no central loop left to hoist that check out of
//!
//! [`basic`]: ab_riscv_interpreter::basic

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
    integer_widen_truncate,
    macroless_generic_const_args,
    min_generic_const_args,
    signed_bigint_helpers,
    try_blocks
)]

use ab_riscv_interpreter::basic::{BasicEagerInstructions, BasicMemory, BasicRegisters};
use ab_riscv_interpreter::impl_vector_registers_for_mut_ref;
use ab_riscv_interpreter::prelude::*;
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::prelude::*;
use anyhow::Context;
use object::{Object, ObjectSection};
use std::fmt;
use std::hint::cold_path;
use std::ops::ControlFlow;

/// The guest program, see `examples/guests` in the repository
const GUEST_ELF: &[u8] = include_bytes!("prebuilt/dot-product.elf");
/// Guest memory base address, which is where the guest ELF is linked to be loaded
const MEMORY_BASE_ADDRESS: u64 = 0x1_0000;
/// Guest memory size, generous enough for the program, its stack, and the input array
const MEMORY_SIZE: usize = 64 * 1024;
/// Address at which the interpreter stops execution gracefully
const TRAP_ADDRESS: u64 = 0;
/// How many `u64` values each of the two arrays holds
const INPUT_LENGTH: usize = 1024;
/// Where the host puts the first array, past the program image and below the stack
const A_ADDRESS: u64 = MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64 / 2;
/// Where the host puts the second one, right after the first
const B_ADDRESS: u64 = A_ADDRESS + (INPUT_LENGTH * size_of::<u64>()) as u64;
/// Stack pointer at the top of guest memory, 16-byte aligned as the psABI requires
const STACK_POINTER: u64 = (MEMORY_BASE_ADDRESS + MEMORY_SIZE as u64) & !0xf;

/// Register type of the composed instruction set
type DotProductRegister = Reg<u64>;

/// The base ISA, multiplication and the vector instruction set. `ZveXxInstruction` brings
/// `ZicsrInstruction` with it, since the vector CSRs are read and written like any other.
#[instruction(
    inherit = [
        Rv64Instruction,
        Rv64MInstruction,
        ZveXxInstruction,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DotProductInstruction<Reg = DotProductRegister> {}

#[instruction]
const impl<Reg> Instruction for DotProductInstruction<Reg> {
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
impl<Reg> fmt::Display for DotProductInstruction<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for DotProductInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Env> ExecutableInstructionCsr<Env> for DotProductInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for DotProductInstruction<Reg>
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
/// Every vector CSR is stored decoded, in the type that describes it. [`Csrs`] is then the place
/// where the raw form a CSR instruction works with is encoded and decoded. The overrides of
/// [`VectorRegistersExt`] below hand the decoded form straight to the instructions that need it,
/// which is what they ask for on every single vector instruction.
#[derive(Debug)]
struct Env {
    /// The vector register file itself
    vregs: VectorRegisterFile<{ Vlen::L512 }>,
    /// Element index the next vector instruction resumes at
    vstart: Vstart,
    /// Fixed-point saturation flag
    vxsat: bool,
    /// Fixed-point rounding mode
    vxrm: Vxrm,
    /// `vxsat` and `vxrm` in the one CSR that shadows both
    vcsr: u64,
    /// How many elements vector instructions currently operate on
    vl: Vl,
    /// How those elements are currently interpreted, `None` while the configuration is invalid
    vtype: Option<Vtype<{ Elen::L64 }, { Vlen::L512 }>>,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            vregs: VectorRegisterFile::default(),
            // The reset state the specification asks for: no valid configuration, so a guest has to
            // set one up with `vsetvl{i}` before it may execute any other vector instruction
            vstart: Vstart::ZERO,
            vxsat: false,
            vxrm: Vxrm::default(),
            vcsr: 0,
            vl: Vl::ZERO,
            vtype: None,
        }
    }
}

impl Csrs<Reg<u64>> for Env {
    fn read_csr(&self, csr_index: u16) -> Result<u64, CsrError> {
        let csr =
            VectorCsr::from_csr_index(csr_index).ok_or(CsrError::IllegalRead { csr_index })?;

        Ok(match csr {
            VectorCsr::Vstart => u64::from(u16::from(self.vstart)),
            VectorCsr::Vxsat => u64::from(self.vxsat),
            VectorCsr::Vxrm => u64::from(self.vxrm.to_bits()),
            VectorCsr::Vcsr => self.vcsr,
            VectorCsr::Vl => u64::from(self.vl),
            VectorCsr::Vtype => match self.vtype {
                Some(vtype) => vtype.to_raw::<Reg<u64>>(),
                None => Vtype::<{ Elen::L64 }, { Vlen::L512 }>::illegal_raw::<Reg<u64>>(),
            },
            VectorCsr::Vlenb => u64::from(Self::VLEN.bytes()),
        })
    }

    fn write_csr(&mut self, csr_index: u16, value: u64) -> Result<(), CsrError> {
        let csr =
            VectorCsr::from_csr_index(csr_index).ok_or(CsrError::IllegalWrite { csr_index })?;

        match csr {
            VectorCsr::Vstart => {
                self.vstart = Vstart::from(value.truncate::<u16>());
            }
            VectorCsr::Vxsat => {
                self.vxsat = (value & 1) == 1;
            }
            VectorCsr::Vxrm => {
                self.vxrm = Vxrm::from_bits(value.truncate::<u8>());
            }
            VectorCsr::Vcsr => {
                self.vcsr = value;
            }
            VectorCsr::Vl => {
                self.vl = Vl::new(value.truncate::<u32>()).unwrap_or_default();
            }
            VectorCsr::Vtype => {
                self.vtype = Vtype::from_raw::<Reg<u64>>(value);
            }
            VectorCsr::Vlenb => {
                cold_path();
                // `VLEN` is fixed, so this one only ever reports it
                return Err(CsrError::ReadOnly { csr_index });
            }
        }

        Ok(())
    }
}

impl VectorRegisters for Env {
    /// The widest element the guest may ask for, the `64` of `Zve64x`
    const ELEN: Elen = Elen::L64;
    /// How wide a vector register is. The guest was compiled for `Zvl128b`, which is a lower
    /// bound: it configures `vl` with `vsetvli` and processes whatever the implementation gives it,
    /// so a wider register file just means fewer trips around its loop.
    const VLEN: Vlen = Vlen::L512;

    #[inline(always)]
    fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }> {
        &self.vregs
    }

    #[inline(always)]
    fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }> {
        &mut self.vregs
    }

    #[inline(always)]
    fn vector_instructions_allowed(&self) -> bool {
        // There is no `mstatus` here to turn vector instructions off through
        true
    }

    #[inline(always)]
    fn mark_vs_dirty(&mut self) {
        // Nothing to mark dirty without `mstatus` either
    }
}

impl VectorRegistersExt<Reg<u64>> for Env {
    #[inline(always)]
    fn vstart(&self) -> Vstart {
        self.vstart
    }

    #[inline(always)]
    fn set_vstart(&mut self, vstart: Vstart) {
        self.vstart = vstart;
    }

    #[inline(always)]
    fn vxsat(&self) -> bool {
        self.vxsat
    }

    #[inline(always)]
    fn set_vxsat(&mut self, vxsat: bool) {
        self.vxsat = vxsat;
        // Mirror `vxsat` into `vcsr[0]`, preserving `vcsr[2:1]` (`vxrm`)
        self.vcsr = (self.vcsr & !1) | u64::from(vxsat);
    }

    #[inline(always)]
    fn vxrm(&self) -> Vxrm {
        self.vxrm
    }

    #[inline(always)]
    fn set_vxrm(&mut self, vxrm: Vxrm) {
        self.vxrm = vxrm;
        // Mirror `vxrm` into `vcsr[2:1]`, preserving `vcsr[0]` (`vxsat`)
        self.vcsr = (self.vcsr & !0b110) | (u64::from(vxrm.to_bits()) << 1);
    }

    #[inline(always)]
    fn vl(&self) -> Vl {
        self.vl
    }

    #[inline(always)]
    fn set_vl(&mut self, vl: Vl) {
        self.vl = vl;
    }

    #[inline(always)]
    fn vtype(&self) -> Option<Vtype<{ Self::ELEN }, { Self::VLEN }>> {
        self.vtype
    }

    #[inline(always)]
    fn set_vtype(&mut self, vtype: Option<Vtype<{ Self::ELEN }, { Self::VLEN }>>) {
        self.vtype = vtype;
    }
}

// Threaded execution passes the environment as `&mut Env`, and the vector traits are not
// blanket-implemented for references the way the simpler ones are
impl_vector_registers_for_mut_ref!(Env, Reg<u64>);

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

fn main() -> anyhow::Result<()> {
    if !OpaqueThreadedExecutionResult::<DotProductInstruction>::platform_supported() {
        println!("Threaded dispatch is not supported on this platform");

        return Ok(());
    }

    let a = (0..INPUT_LENGTH as u64)
        .map(|index| index.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .collect::<Vec<_>>();
    let b = (0..INPUT_LENGTH as u64)
        .map(|index| index.wrapping_mul(0xbf58_476d_1ce4_e5b9))
        .collect::<Vec<_>>();

    let mut memory = BasicMemory::<MEMORY_BASE_ADDRESS, MEMORY_SIZE>::new_boxed();

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

    for (address, values) in [(A_ADDRESS, &a), (B_ADDRESS, &b)] {
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        memory
            .write_slice(address, &bytes)
            .map_err(anyhow::Error::from)
            .context("Input does not fit into guest memory")?;
    }

    let text = elf
        .section_by_name(".text")
        .context("Guest ELF has no `.text` section")?;
    // SAFETY: The guest is compiled by a trusted compiler and ends with a `ret`, it does not write
    // into its own code, the trap address is outside of it, `.text` is loaded at its own address,
    // which is a multiple of the instruction alignment, and guest memory is far from the end of the
    // address space
    let instructions = unsafe {
        BasicEagerInstructions::decode(
            text.data()
                .context("Failed to read `.text` section of the guest ELF")?,
            DotProductInstruction::Unimp {
                rs1: Reg::ZERO,
                rs2: Reg::ZERO,
            },
            TRAP_ADDRESS,
            text.address(),
        )
    };

    // Arguments and the return address, exactly as a RISC-V caller would set them up
    let mut regs = BasicRegisters::<Reg<u64>, true>::default();
    regs.write(Reg::Ra, TRAP_ADDRESS);
    regs.write(Reg::Sp, STACK_POINTER);
    regs.write(Reg::A0, A_ADDRESS);
    regs.write(Reg::A1, B_ADDRESS);
    regs.write(Reg::A2, INPUT_LENGTH as u64);

    let ThreadedExecutionResult {
        outcome,
        program_counter: _,
    } = DotProductInstruction::execute_threaded(
        // SAFETY: The entry point is one of the instructions decoded above
        unsafe { instructions.fetcher(elf.entry()) },
        &mut regs,
        &mut Env::default(),
        memory.as_mut(),
    );
    outcome.context("Guest execution failed")?;

    let dot_product = regs.read(Reg::A0);
    println!("Guest computed {dot_product:#018x}");

    // The same thing the guest did, this time natively
    let expected = a
        .iter()
        .zip(&b)
        .map(|(a, b)| a.wrapping_mul(*b))
        .fold(0, u64::wrapping_add);
    assert_eq!(dot_product, expected);

    Ok(())
}
