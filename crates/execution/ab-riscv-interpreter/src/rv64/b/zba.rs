//! RV64 Zba extension

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
    for Rv64ZbaInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        _rs1rs2_values: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
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
            Self::AddUw { rd, rs1, rs2 } => {
                let rs1_val = (regs.read(rs1) as u32) as u64;
                let value = rs1_val.wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh1add { rd, rs1, rs2 } => {
                let value = (regs.read(rs1) << 1).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh1addUw { rd, rs1, rs2 } => {
                let rs1_val = (regs.read(rs1) as u32) as u64;
                let value = (rs1_val << 1).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh2add { rd, rs1, rs2 } => {
                let value = (regs.read(rs1) << 2).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh2addUw { rd, rs1, rs2 } => {
                let rs1_val = (regs.read(rs1) as u32) as u64;
                let value = (rs1_val << 2).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh3add { rd, rs1, rs2 } => {
                let value = (regs.read(rs1) << 3).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh3addUw { rd, rs1, rs2 } => {
                let rs1_val = (regs.read(rs1) as u32) as u64;
                let value = (rs1_val << 3).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::SlliUw { rd, rs1, shamt } => {
                let rs1_val = (regs.read(rs1) as u32) as u64;
                let value = rs1_val << (shamt & 0x3f);
                Ok(ControlFlow::Continue((rd, value)))
            }
        }
    }
}
