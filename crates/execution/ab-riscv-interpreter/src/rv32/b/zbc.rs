//! RV32 Zbc extension

pub mod rv32_zbc_helpers;
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
    for Rv32ZbcInstruction<Reg>
where
    Reg: Register<Type = u32>,
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
            Self::Clmul { rd, rs1, rs2 } => {
                let a = regs.read(rs1);
                let b = regs.read(rs2);

                Ok(ControlFlow::Continue((rd, rv32_zbc_helpers::clmul(a, b))))
            }
            Self::Clmulh { rd, rs1, rs2 } => {
                let a = regs.read(rs1);
                let b = regs.read(rs2);

                Ok(ControlFlow::Continue((rd, rv32_zbc_helpers::clmulh(a, b))))
            }
            Self::Clmulr { rd, rs1, rs2 } => {
                let a = regs.read(rs1);
                let b = regs.read(rs2);

                Ok(ControlFlow::Continue((rd, rv32_zbc_helpers::clmulr(a, b))))
            }
        }
    }
}
