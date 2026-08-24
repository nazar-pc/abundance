//! RV32 Zalrsc extension

#[cfg(test)]
mod tests;

use crate::rv32::a::ReservationSet;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZalrscInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for Rv32ZalrscInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for Rv32ZalrscInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
    Env: [const] ReservationSet<Reg>,
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
        env: &mut Env,
        memory: &mut Memory,
        _program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Lr {
                rd,
                rs1: _,
                aq: _,
                rl: _,
            } => {
                let value = memory.read::<u32>(u64::from(rs1_value))?;
                env.set_reservation(rs1_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sc {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let success = env.reservation() == Some(rs1_value);
                env.clear_reservation();

                if success {
                    memory.write(u64::from(rs1_value), rs2_value)?;
                    ExecutionResult::Continue { rd, value: 0 }
                } else {
                    ExecutionResult::Continue { rd, value: 1 }
                }
            }
        }
    }
}
