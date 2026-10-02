//! RV64 M extension

#[cfg(test)]
mod tests;
pub mod zmmul;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64MInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv64MInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64MInstruction<Reg>
where
    Reg: [const] Register<Type = u64>,
    Regs: [const] RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _env: &mut Env,
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
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
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Divu { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value;
                let divisor = rs2_value;
                let value = dividend.checked_div(divisor).unwrap_or(u64::MAX);
                ExecutionResult::Continue { rd, value }
            }
            Self::Rem { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value.cast_signed();
                let divisor = rs2_value.cast_signed();
                #[expect(
                    clippy::modulo_arithmetic,
                    reason = "This is what the code is supposed to do"
                )]
                let value = if divisor == 0 {
                    dividend
                } else if dividend == i64::MIN && divisor == -1 {
                    0
                } else {
                    dividend % divisor
                };
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Remu { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value;
                let divisor = rs2_value;
                let value = if divisor == 0 {
                    dividend
                } else {
                    dividend % divisor
                };
                ExecutionResult::Continue { rd, value }
            }

            // RV64 R-type W
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
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Divuw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as u32;
                let divisor = rs2_value as u32;
                let value = match dividend.checked_div(divisor) {
                    Some(value) => i64::from(value.cast_signed()).cast_unsigned(),
                    None => u64::MAX,
                };
                ExecutionResult::Continue { rd, value }
            }
            Self::Remw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as i32;
                let divisor = rs2_value as i32;
                #[expect(
                    clippy::modulo_arithmetic,
                    reason = "This is what the code is supposed to do"
                )]
                let value = if divisor == 0 {
                    i64::from(dividend).cast_unsigned()
                } else if dividend == i32::MIN && divisor == -1 {
                    0
                } else {
                    i64::from(dividend % divisor).cast_unsigned()
                };
                ExecutionResult::Continue { rd, value }
            }
            Self::Remuw { rd, rs1: _, rs2: _ } => {
                let dividend = rs1_value as u32;
                let divisor = rs2_value as u32;
                let value = if divisor == 0 {
                    dividend.cast_signed()
                } else {
                    (dividend % divisor).cast_signed()
                };
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(value).cast_unsigned(),
                }
            }
        }
    }
}
