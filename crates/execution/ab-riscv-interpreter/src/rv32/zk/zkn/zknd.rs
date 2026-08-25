//! RV32 Zknd extension

pub mod rv32_zknd_helpers;
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
const impl<Reg> ExecutableInstructionOperands for Rv32ZkndInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv32ZkndInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv32ZkndInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
            Self::Aes32Dsi {
                rd,
                rs1: _,
                rs2: _,
                bs,
            } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv32_zknd_helpers::aes32dsi(v1, v2, bs),
                }
            }
            Self::Aes32Dsmi {
                rd,
                rs1: _,
                rs2: _,
                bs,
            } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv32_zknd_helpers::aes32dsmi(v1, v2, bs),
                }
            }
        }
    }
}
