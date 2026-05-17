//! RV32 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Rv32MInstruction<Reg> where Reg: Register<Type = u32> {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv32MInstruction<Reg>
where
    Reg: Register<Type = u32>,
{
}

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
                // Signed × signed: widen to i64, take upper 32 bits
                let (_lo, prod) = rs1_value
                    .cast_signed()
                    .widening_mul(rs2_value.cast_signed());
                Ok(ControlFlow::Continue((rd, prod.cast_unsigned())))
            }
            Self::Mulhsu { rd, rs1: _, rs2: _ } => {
                // Signed × unsigned: widen to i64, take upper 32 bits
                let prod = i64::from(rs1_value.cast_signed()) * i64::from(rs2_value);
                let value = prod >> 32;
                Ok(ControlFlow::Continue((rd, value.cast_unsigned() as u32)))
            }
            Self::Mulhu { rd, rs1: _, rs2: _ } => {
                // Unsigned × unsigned: widen to u64, take upper 32 bits
                let prod = u64::from(rs1_value) * u64::from(rs2_value);
                let value = prod >> 32;
                Ok(ControlFlow::Continue((rd, value as u32)))
            }
            Self::Div { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value.cast_signed();
                let divisor = rs2_value.cast_signed();
                let value = if divisor == 0 {
                    -1i32
                } else if dividend == i32::MIN && divisor == -1 {
                    i32::MIN
                } else {
                    dividend / divisor
                };
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Divu { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value;
                let divisor = rs2_value;
                let value = dividend.checked_div(divisor).unwrap_or(u32::MAX);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Rem { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value.cast_signed();
                let divisor = rs2_value.cast_signed();
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i32::MIN && divisor == -1 {
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
        }
    }
}
