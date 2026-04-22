//! Zicond extension

#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
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
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            // Conditional zero, equal to zero.
            //
            // rd = (rs2 == 0) ? 0 : rs1
            Self::CzeroEqz { rd, rs1, rs2 } => {
                let condition = regs.read(rs2);
                let src = regs.read(rs1);
                let result = if condition == Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                regs.write(rd, result);
            }

            // Conditional zero, nonzero.
            //
            // rd = (rs2 != 0) ? 0 : rs1
            Self::CzeroNez { rd, rs1, rs2 } => {
                let condition = regs.read(rs2);
                let src = regs.read(rs1);
                let result = if condition != Reg::Type::from(0u8) {
                    Reg::Type::from(0u8)
                } else {
                    src
                };
                regs.write(rd, result);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
