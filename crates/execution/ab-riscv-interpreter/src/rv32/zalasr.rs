//! RV32 Zalasr extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZalasrInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32ZalasrInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv32ZalasrInstruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::LbAq { rd, rs1: _, rl: _ } => {
                let addr = u64::from(rs1_value);
                let value = i32::from(memory.read::<i8>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::LhAq { rd, rs1: _, rl: _ } => {
                let addr = u64::from(rs1_value);
                let value = i32::from(memory.read::<i16>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::LwAq { rd, rs1: _, rl: _ } => {
                let addr = u64::from(rs1_value);
                let value = memory.read::<u32>(addr)?;
                ExecutionResult::Continue { rd, value }
            }
            Self::SbRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                let addr = u64::from(rs1_value);
                memory.write(addr, rs2_value as u8)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::ShRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                let addr = u64::from(rs1_value);
                memory.write(addr, rs2_value as u16)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::SwRl {
                rs1: _,
                rs2: _,
                aq: _,
            } => {
                let addr = u64::from(rs1_value);
                memory.write(addr, rs2_value)?;
                ExecutionResult::ContinueNoWrite
            }
        }
    }
}
