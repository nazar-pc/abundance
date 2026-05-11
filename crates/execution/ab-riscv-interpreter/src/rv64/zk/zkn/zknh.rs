//! RV64 Zknh extension

pub mod rv64_zknh_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZknhInstruction<Reg>
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
            Self::Sha256Sig0 { rd, rs1 } => {
                let x = regs.read(rs1) as u32;

                let res32 = rv64_zknh_helpers::sha256sig0(x);

                Ok(ControlFlow::Continue((
                    rd,
                    i64::from(res32.cast_signed()).cast_unsigned(),
                )))
            }
            Self::Sha256Sig1 { rd, rs1 } => {
                let x = regs.read(rs1) as u32;

                let res32 = rv64_zknh_helpers::sha256sig1(x);

                Ok(ControlFlow::Continue((
                    rd,
                    i64::from(res32.cast_signed()).cast_unsigned(),
                )))
            }
            Self::Sha256Sum0 { rd, rs1 } => {
                let x = regs.read(rs1) as u32;

                let res32 = rv64_zknh_helpers::sha256sum0(x);

                Ok(ControlFlow::Continue((
                    rd,
                    i64::from(res32.cast_signed()).cast_unsigned(),
                )))
            }
            Self::Sha256Sum1 { rd, rs1 } => {
                let x = regs.read(rs1) as u32;

                let res32 = rv64_zknh_helpers::sha256sum1(x);

                Ok(ControlFlow::Continue((
                    rd,
                    i64::from(res32.cast_signed()).cast_unsigned(),
                )))
            }
            Self::Sha512Sig0 { rd, rs1 } => {
                let x = regs.read(rs1);

                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknh_helpers::sha512sig0(x),
                )))
            }
            Self::Sha512Sig1 { rd, rs1 } => {
                let x = regs.read(rs1);

                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknh_helpers::sha512sig1(x),
                )))
            }
            Self::Sha512Sum0 { rd, rs1 } => {
                let x = regs.read(rs1);

                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknh_helpers::sha512sum0(x),
                )))
            }
            Self::Sha512Sum1 { rd, rs1 } => {
                let x = regs.read(rs1);

                Ok(ControlFlow::Continue((
                    rd,
                    rv64_zknh_helpers::sha512sum1(x),
                )))
            }
        }
    }
}
