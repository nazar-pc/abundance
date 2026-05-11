//! RV32 Zba extension

#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZbaInstruction<Reg>
where
    Reg: Register<Type = u32>,
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
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::Sh1add { rd, rs1, rs2 } => {
                let value = (regs.read(rs1) << 1).wrapping_add(regs.read(rs2));
                regs.write(rd, value);
            }
            Self::Sh2add { rd, rs1, rs2 } => {
                let value = (regs.read(rs1) << 2).wrapping_add(regs.read(rs2));
                regs.write(rd, value);
            }
            Self::Sh3add { rd, rs1, rs2 } => {
                let value = (regs.read(rs1) << 3).wrapping_add(regs.read(rs2));
                regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
