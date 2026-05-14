//! RV64 Zmmul extension (multiplication subset of M extension)

use crate::{
    ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZmmulInstruction<Reg>
where
    Reg: Register<Type = u64>,
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
        Ok(ControlFlow::Continue(Default::default()))
    }
}
