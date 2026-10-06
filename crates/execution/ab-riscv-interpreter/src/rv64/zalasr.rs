//! RV64 Zalasr extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for Rv64ZalasrInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for Rv64ZalasrInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64ZalasrInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<Reg::Type>,
        _regs: &mut Regs,
        _env: &mut Env,
        memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        match self {
            Self::LbAq { rd, rs1: _, rl: _ } => {
                let value = i64::from(memory.read::<i8>(rs1_value)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::LhAq { rd, rs1: _, rl: _ } => {
                let value = i64::from(memory.read::<i16>(rs1_value)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::LwAq { rd, rs1: _, rl: _ } => {
                let value = i64::from(memory.read::<i32>(rs1_value)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::LdAq { rd, rs1: _, rl: _ } => {
                let value = memory.read::<u64>(rs1_value)?;
                ExecutionResult::Continue { rd, value }
            }
            Self::SbRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                memory.write(rs1_value, rs2_value as u8)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::ShRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                memory.write(rs1_value, rs2_value as u16)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::SwRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                memory.write(rs1_value, rs2_value as u32)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::SdRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                memory.write(rs1_value, rs2_value)?;
                ExecutionResult::ContinueNoWrite
            }
        }
    }
}
