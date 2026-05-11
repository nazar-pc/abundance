//! RV32 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32MInstruction<Reg>
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
            Self::Mul { rd, rs1, rs2 } => {
                let value = regs.read(rs1).wrapping_mul(regs.read(rs2));
                regs.write(rd, value);
            }
            Self::Mulh { rd, rs1, rs2 } => {
                // Signed × signed: widen to i64, take upper 32 bits
                let (_lo, prod) = regs
                    .read(rs1)
                    .cast_signed()
                    .widening_mul(regs.read(rs2).cast_signed());
                regs.write(rd, prod.cast_unsigned());
            }
            Self::Mulhsu { rd, rs1, rs2 } => {
                // Signed × unsigned: widen to i64, take upper 32 bits
                let prod = i64::from(regs.read(rs1).cast_signed()) * i64::from(regs.read(rs2));
                let value = prod >> 32;
                regs.write(rd, value.cast_unsigned() as u32);
            }
            Self::Mulhu { rd, rs1, rs2 } => {
                // Unsigned × unsigned: widen to u64, take upper 32 bits
                let prod = u64::from(regs.read(rs1)) * u64::from(regs.read(rs2));
                let value = prod >> 32;
                regs.write(rd, value as u32);
            }
            Self::Div { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1).cast_signed();
                let divisor = regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    -1i32
                } else if dividend == i32::MIN && divisor == -1 {
                    i32::MIN
                } else {
                    dividend / divisor
                };
                regs.write(rd, value.cast_unsigned());
            }
            Self::Divu { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1);
                let divisor = regs.read(rs2);
                let value = dividend.checked_div(divisor).unwrap_or(u32::MAX);
                regs.write(rd, value);
            }
            Self::Rem { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1).cast_signed();
                let divisor = regs.read(rs2).cast_signed();
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i32::MIN && divisor == -1 {
                    0
                } else {
                    dividend % divisor
                };
                regs.write(rd, value.cast_unsigned());
            }
            Self::Remu { rd, rs1, rs2 } => {
                let dividend = regs.read(rs1);
                let divisor = regs.read(rs2);
                let value = if divisor == 0 {
                    dividend
                } else {
                    dividend % divisor
                };
                regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
