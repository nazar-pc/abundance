//! RV32 Zmmul extension (multiplication subset of M extension)

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZmmulInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv32ZmmulInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv32ZmmulInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
            Self::Mul { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.wrapping_mul(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Mulh { rd, rs1: _, rs2: _ } => {
                // Signed × signed: multiply and take upper 32 bits
                let (_lo, prod) = rs1_value
                    .cast_signed()
                    .carrying_mul(rs2_value.cast_signed(), 0);
                ExecutionResult::Continue {
                    rd,
                    value: prod.cast_unsigned(),
                }
            }
            Self::Mulhsu { rd, rs1: _, rs2: _ } => {
                // Signed × unsigned: widen to i64, take upper 32 bits
                let prod = i64::from(rs1_value.cast_signed()) * i64::from(rs2_value);
                let value = prod >> 32;
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned() as u32,
                }
            }
            Self::Mulhu { rd, rs1: _, rs2: _ } => {
                // Unsigned × unsigned: widen to u64, take upper 32 bits
                let prod = u64::from(rs1_value) * u64::from(rs2_value);
                let value = prod >> 32;
                ExecutionResult::Continue {
                    rd,
                    value: value as u32,
                }
            }
        }
    }
}
