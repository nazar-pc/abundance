//! RV32 Zknd extension

pub mod rv32_zknd_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZkndInstruction<Reg>
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
            Self::Aes32Dsi { rd, rs1, rs2, bs } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                regs.write(rd, rv32_zknd_helpers::aes32dsi(v1, v2, bs));
            }
            Self::Aes32Dsmi { rd, rs1, rs2, bs } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                regs.write(rd, rv32_zknd_helpers::aes32dsmi(v1, v2, bs));
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
