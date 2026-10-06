//! RV32 B extension

pub mod zba;
pub mod zbb;
pub mod zbc;
pub mod zbs;

use crate::rv32::b::zbb::rv32_zbb_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for Rv32BInstruction<Hart>
where
    Reg: Register<Type = u32>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for Rv32BInstruction<Hart>
where
    Reg: Register<Type = u32>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv32BInstruction<Hart>
where
    Reg: Register<Type = u32>,
    Hart: HartConfig<Reg = Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
        ExecutionResult::ContinueNoWrite
    }
}
