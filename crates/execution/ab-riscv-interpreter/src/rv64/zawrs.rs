//! RV64 Zawrs extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutableInstructionResult, Rs1Rs2OperandValues, Rs1Rs2Operands, WrsHandler,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZawrsInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv64ZawrsInstruction<Reg>
where
    Reg: Register<Type = u64>,
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZawrsInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
    InstructionHandler: [const] WrsHandler,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value: _,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutableInstructionResult<(), Self, CustomError> {
        match self {
            Self::WrsNto => {
                system_instruction_handler.handle_wrs_nto();
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::WrsSto => {
                system_instruction_handler.handle_wrs_sto();
                Ok(ControlFlow::Continue(Default::default()))
            }
        }
    }
}
