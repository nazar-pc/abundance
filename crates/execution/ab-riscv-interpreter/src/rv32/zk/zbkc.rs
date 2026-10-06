//! RV32 Zbkc extension (subset of Zbc extension)

use crate::rv32::b::zbc::rv32_zbc_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for Rv32ZbkcInstruction<Hart>
where
    Reg: Register<Type = u32>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for Rv32ZbkcInstruction<Hart>
where
    Reg: Register<Type = u32>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv32ZbkcInstruction<Hart>
where
    Reg: [const] Register<Type = u32>,
    Hart: HartConfig<Reg = Reg>,
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
            Self::Clmul { rd, rs1: _, rs2: _ } => {
                let a = rs1_value;
                let b = rs2_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv32_zbc_helpers::clmul(a, b),
                }
            }
            Self::Clmulh { rd, rs1: _, rs2: _ } => {
                let a = rs1_value;
                let b = rs2_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv32_zbc_helpers::clmulh(a, b),
                }
            }
        }
    }
}
