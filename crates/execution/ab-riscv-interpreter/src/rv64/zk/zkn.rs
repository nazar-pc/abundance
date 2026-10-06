//! RV64 Zkn extension

pub mod zknd;
pub mod zkne;
pub mod zknh;

use crate::rv64::b::zbc::rv64_zbc_helpers;
use crate::rv64::zk::zbkx::rv64_zbkx_helpers;
use crate::rv64::zk::zkn::zknd::rv64_zknd_helpers;
use crate::rv64::zk::zkn::zkne::rv64_zkne_helpers;
use crate::rv64::zk::zkn::zknh::rv64_zknh_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for Rv64ZknInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for Rv64ZknInstruction<Hart>
where
    Reg: Register<Type = u64>,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv64ZknInstruction<Hart>
where
    Reg: [const] Register<Type = u64>,
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
        ExecutionResult::ContinueNoWrite
    }
}
