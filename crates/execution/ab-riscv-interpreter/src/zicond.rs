//! Zicond extension

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
    for ZicondInstruction<Reg>
where
    Reg: Register,
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
            // Conditional zero, equal to zero.
            //
            // rd = (rs2 == 0) ? 0 : rs1
            Self::CzeroEqz { rd, rs1: _, rs2: _ } => {
                let condition = rs2_value;
                let src = rs1_value;
                let result = if condition == Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                Ok(ControlFlow::Continue((rd, result)))
            }

            // Conditional zero, nonzero.
            //
            // rd = (rs2 != 0) ? 0 : rs1
            Self::CzeroNez { rd, rs1: _, rs2: _ } => {
                let condition = rs2_value;
                let src = rs1_value;
                let result = if condition != Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                Ok(ControlFlow::Continue((rd, result)))
            }
        }
    }
}
