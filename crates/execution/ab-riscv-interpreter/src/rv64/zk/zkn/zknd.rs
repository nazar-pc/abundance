//! RV64 Zknd extension

pub mod rv64_zknd_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Rv64ZkndInstruction<Reg> where Reg: Register<Type = u64> {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv64ZkndInstruction<Reg>
where
    Reg: Register<Type = u64>,
{
}

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
            Self::Aes64Ds { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64ds(v1, v2),
                )))
            }
            Self::Aes64Dsm { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64dsm(v1, v2),
                )))
            }
            Self::Aes64Im { rd, rs1: _ } => {
                let v1 = rs1_value;
                Ok(ControlFlow::Continue((rd, rv64_zknd_helpers::aes64im(v1))))
            }
            Self::Aes64Ks1i { rd, rs1: _, rnum } => {
                let v1 = rs1_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64ks1i(v1, rnum),
                )))
            }
            Self::Aes64Ks2 { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknd_helpers::aes64ks2(v1, v2),
                )))
            }
        }
    }
}
