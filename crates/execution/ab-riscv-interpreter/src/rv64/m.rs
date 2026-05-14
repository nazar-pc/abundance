//! RV64 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::{
    ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64MInstruction<Reg>
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
            Self::Mul { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.wrapping_mul(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Mulh { rd, rs1: _, rs2: _ } => {
                // Signed × signed: widen to i128, take upper 64 bits
                let (_lo, prod) = rs1_value
                    .cast_signed()
                    .widening_mul(rs2_value.cast_signed());
                Ok(ControlFlow::Continue((rd, prod.cast_unsigned())))
            }
            Self::Mulhsu { rd, rs1: _, rs2: _ } => {
                // Signed × unsigned: widen to i128, take upper 64 bits
                let prod = i128::from(rs1_value.cast_signed()) * i128::from(rs2_value);
                let value = prod >> 64;
                Ok(ControlFlow::Continue((rd, value.cast_unsigned() as u64)))
            }
            Self::Mulhu { rd, rs1: _, rs2: _ } => {
                // Unsigned × unsigned: widen to u128, take upper 64 bits
                let prod = u128::from(rs1_value) * u128::from(rs2_value);
                let value = prod >> 64;
                Ok(ControlFlow::Continue((rd, value as u64)))
            }
            Self::Div { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value.cast_signed();
                let divisor = rs2_value.cast_signed();
                let value = if divisor == 0 {
                    -1i64
                } else if dividend == i64::MIN && divisor == -1 {
                    i64::MIN
                } else {
                    dividend / divisor
                };
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Divu { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value;
                let divisor = rs2_value;
                let value = dividend.checked_div(divisor).unwrap_or(u64::MAX);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Rem { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value.cast_signed();
                let divisor = rs2_value.cast_signed();
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i64::MIN && divisor == -1 {
                    0
                } else {
                    dividend % divisor
                };
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Remu { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value;
                let divisor = rs2_value;
                let value = if divisor == 0 {
                    dividend
                } else {
                    dividend % divisor
                };
                Ok(ControlFlow::Continue((rd, value)))
            }

            // RV64 R-type W
            Self::Mulw { rd, rs1: _, rs2: _ } => {
                let prod = (rs1_value as i32).wrapping_mul(rs2_value as i32);
                Ok(ControlFlow::Continue((rd, (prod as i64).cast_unsigned())))
            }
            Self::Divw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as i32;
                let divisor = rs2_value as i32;
                let value = if divisor == 0 {
                    -1i64
                } else if dividend == i32::MIN && divisor == -1 {
                    i64::from(i32::MIN)
                } else {
                    i64::from(dividend / divisor)
                };
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Divuw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as u32;
                let divisor = rs2_value as u32;
                let value = dividend.checked_div(divisor).map_or(u64::MAX, |value| {
                    i64::from(value.cast_signed()).cast_unsigned()
                });
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Remw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as i32;
                let divisor = rs2_value as i32;
                let value = if divisor == 0 {
                    (dividend as i64).cast_unsigned()
                } else if dividend == i32::MIN && divisor == -1 {
                    0
                } else {
                    ((dividend % divisor) as i64).cast_unsigned()
                };
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Remuw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as u32;
                let divisor = rs2_value as u32;
                let value = if divisor == 0 {
                    dividend.cast_signed() as i64
                } else {
                    (dividend % divisor).cast_signed() as i64
                };
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
        }
    }
}
