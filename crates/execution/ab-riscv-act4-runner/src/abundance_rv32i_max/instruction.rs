use ab_riscv_primitives::registers::general_purpose::{Reg, RegType};
// TODO: Some way to allow re-exporting imports by the macro, such that explicit imports for helpers
//  and such are not needed
use ab_riscv_interpreter::rv32::b::zbb::rv32_zbb_helpers;
use ab_riscv_interpreter::rv32::b::zbc::rv32_zbc_helpers;
use ab_riscv_interpreter::rv32::zk::zbkb::rv32_zbkb_helpers;
use ab_riscv_interpreter::rv32::zk::zbkx::rv32_zbkx_helpers;
use ab_riscv_interpreter::v::vector_registers::VectorRegistersExt;
use ab_riscv_interpreter::v::zve64x::arith::zve64x_arith_helpers;
use ab_riscv_interpreter::v::zve64x::config::zve64x_config_helpers;
use ab_riscv_interpreter::v::zve64x::fixed_point::zve64x_fixed_point_helpers;
use ab_riscv_interpreter::v::zve64x::load::zve64x_load_helpers;
use ab_riscv_interpreter::v::zve64x::mask::zve64x_mask_helpers;
use ab_riscv_interpreter::v::zve64x::muldiv::zve64x_muldiv_helpers;
use ab_riscv_interpreter::v::zve64x::perm::zve64x_perm_helpers;
use ab_riscv_interpreter::v::zve64x::reduction::zve64x_reduction_helpers;
use ab_riscv_interpreter::v::zve64x::store::zve64x_store_helpers;
use ab_riscv_interpreter::v::zve64x::widen_narrow::zve64x_widen_narrow_helpers;
use ab_riscv_interpreter::v::zve64x::zve64x_helpers;
use ab_riscv_interpreter::zicsr::zicsr_helpers;
use ab_riscv_interpreter::{
    CsrError, Csrs, ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::{instruction, instruction_execution};
use ab_riscv_primitives::instructions::Instruction;
use ab_riscv_primitives::instructions::rv32::Rv32Instruction;
use ab_riscv_primitives::instructions::rv32::b::zba::Rv32ZbaInstruction;
use ab_riscv_primitives::instructions::rv32::b::zbb::Rv32ZbbInstruction;
use ab_riscv_primitives::instructions::rv32::b::zbc::Rv32ZbcInstruction;
use ab_riscv_primitives::instructions::rv32::b::zbs::Rv32ZbsInstruction;
use ab_riscv_primitives::instructions::rv32::m::Rv32MInstruction;
use ab_riscv_primitives::instructions::rv32::zk::zbkb::Rv32ZbkbInstruction;
use ab_riscv_primitives::instructions::rv32::zk::zbkx::Rv32ZbkxInstruction;
use ab_riscv_primitives::instructions::rv32::zk::zkn::zknh::Rv32ZknhInstruction;
use ab_riscv_primitives::instructions::zicond::ZicondInstruction;
// TODO: Improve macro generation to use the declared dependency enum for `fmt::Display`
//  implementation instead of the original one, so these imports are no longer needed
use ab_riscv_interpreter::rv32::zk::zkn::zknh::zknh_helpers;
use ab_riscv_primitives::instructions::v::zve64x::arith::Zve64xArithInstruction;
use ab_riscv_primitives::instructions::v::zve64x::config::Zve64xConfigInstruction;
use ab_riscv_primitives::instructions::v::zve64x::fixed_point::Zve64xFixedPointInstruction;
use ab_riscv_primitives::instructions::v::zve64x::load::Zve64xLoadInstruction;
use ab_riscv_primitives::instructions::v::zve64x::mask::Zve64xMaskInstruction;
use ab_riscv_primitives::instructions::v::zve64x::muldiv::Zve64xMulDivInstruction;
use ab_riscv_primitives::instructions::v::zve64x::perm::Zve64xPermInstruction;
use ab_riscv_primitives::instructions::v::zve64x::reduction::Zve64xReductionInstruction;
use ab_riscv_primitives::instructions::v::zve64x::store::Zve64xStoreInstruction;
use ab_riscv_primitives::instructions::v::zve64x::widen_narrow::Zve64xWidenNarrowInstruction;
use ab_riscv_primitives::instructions::v::{Eew, Vsew};
use ab_riscv_primitives::instructions::zicsr::ZicsrInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use ab_riscv_primitives::registers::machine::MCsr;
use ab_riscv_primitives::registers::vector::{VCsr, VReg};
use core::fmt;
use core::ops::ControlFlow;

/// All instructions supported by the interpreter for RV32I base ISA
pub(crate) type AbundanceRv32IMaxInstruction = AbundanceRv32IMaxInstructionPrototype<Reg<u32>>;

/// All instructions supported by the interpreter for RV32I base ISA
#[instruction(
    inherit = [
        Rv32Instruction,
        Rv32BInstruction,
        Rv32MInstruction,
        Rv32ZbcInstruction,
        Rv32ZbkbInstruction,
        Rv32ZbkcInstruction,
        Rv32ZbkxInstruction,
        Rv32ZknhInstruction,
        ZicondInstruction,
        ZicsrInstruction,
        Zve64xInstruction,
        MachineModePlaceholder,
    ],
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbundanceRv32IMaxInstructionPrototype<Reg> {}

#[instruction]
impl<Reg> const Instruction for AbundanceRv32IMaxInstructionPrototype<Reg>
where
    Reg: [const] Register<Type = u32>,
{
    type Reg = Reg;

    #[inline(always)]
    fn try_decode(instruction: u32) -> Option<Self> {
        None
    }

    #[inline(always)]
    fn alignment() -> u8 {
        align_of::<u32>() as u8
    }

    #[inline(always)]
    fn size(&self) -> u8 {
        size_of::<u32>() as u8
    }
}

#[instruction]
impl<Reg> fmt::Display for AbundanceRv32IMaxInstructionPrototype<Reg>
where
    Reg: fmt::Display + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {}
    }
}

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for AbundanceRv32IMaxInstructionPrototype<Reg>
where
    Reg: Register<Type = u32>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        Ok(ControlFlow::Continue(()))
    }
}
