//! RV64 Zknd extension

pub mod rv64_zknd_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for Rv64ZkndZkneSharedInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for Rv64ZkndZkneSharedInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64ZkndZkneSharedInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
    Regs: [const] RegisterFile<Reg>,
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
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        match self {
            Self::Aes64Ks1i { rd, rs1: _, rnum } => {
                let v1 = rs1_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknd_helpers::aes64ks1i(v1, rnum),
                }
            }
            Self::Aes64Ks2 { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknd_helpers::aes64ks2(v1, v2),
                }
            }
        }
    }
}

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for Rv64ZkndInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for Rv64ZkndInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64ZkndInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
    Regs: [const] RegisterFile<Reg>,
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
        _memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        match self {
            Self::Aes64Ds { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknd_helpers::aes64ds(v1, v2),
                }
            }
            Self::Aes64Dsm { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknd_helpers::aes64dsm(v1, v2),
                }
            }
            Self::Aes64Im { rd, rs1: _ } => {
                let v1 = rs1_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknd_helpers::aes64im(v1),
                }
            }
        }
    }
}
