//! RV64 Zknd extension

pub mod rv64_zknd_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZkndInstruction<Reg>
where
    Reg: Register<Type = u64>,
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
            Self::Aes64Ds { rd, rs1, rs2 } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64ds(v1, v2),
                )))
            }
            Self::Aes64Dsm { rd, rs1, rs2 } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64dsm(v1, v2),
                )))
            }
            Self::Aes64Im { rd, rs1 } => {
                let v1 = regs.read(rs1);
                Ok(ControlFlow::Continue((rd, rv64_zknd_helpers::aes64im(v1))))
            }
            Self::Aes64Ks1i { rd, rs1, rnum } => {
                let v1 = regs.read(rs1);
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64ks1i(v1, rnum),
                )))
            }
            Self::Aes64Ks2 { rd, rs1, rs2 } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64ks2(v1, v2),
                )))
            }
        }
    }
}
