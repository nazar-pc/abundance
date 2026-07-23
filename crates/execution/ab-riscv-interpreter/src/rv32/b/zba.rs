//! RV32 Zba extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutableInstructionResult, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZbaInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv32ZbaInstruction<Reg>
where
    Reg: Register<Type = u32>,
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZbaInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
    Regs: [const] RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
    ) -> ExecutableInstructionResult<(), Self, CustomError> {
        match self {
            Self::Sh1add { rd, rs1: _, rs2: _ } => {
                let value = (rs1_value << 1).wrapping_add(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh2add { rd, rs1: _, rs2: _ } => {
                let value = (rs1_value << 2).wrapping_add(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sh3add { rd, rs1: _, rs2: _ } => {
                let value = (rs1_value << 3).wrapping_add(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
        }
    }
}
