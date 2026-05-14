//! RV64 Zbkx extension

pub mod rv64_zbkx_helpers;
// TODO: Portable SIMD attempts to use unsupported intrinsics under Miri:
//  https://github.com/rust-lang/portable-simd/issues/524
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZbkxInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::Xperm4 { rd, rs1: _, rs2: _ } => Ok(ControlFlow::Continue((
                rd,
                rv64_zbkx_helpers::xperm4(rs1_value, rs2_value),
            ))),
            Self::Xperm8 { rd, rs1: _, rs2: _ } => Ok(ControlFlow::Continue((
                rd,
                rv64_zbkx_helpers::xperm8(rs1_value, rs2_value),
            ))),
        }
    }
}
