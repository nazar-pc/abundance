//! RV32 Zaamo extension

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
const impl<Reg> ExecutableInstructionOperands for Rv32ZaamoInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32ZaamoInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv32ZaamoInstruction<Reg>
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
            Self::Amoswap {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                memory.write(u64::from(rs1_value), rs2_value)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amoadd {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                memory.write(u64::from(rs1_value), old.wrapping_add(rs2_value))?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amoxor {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                memory.write(u64::from(rs1_value), old ^ rs2_value)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amoand {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                memory.write(u64::from(rs1_value), old & rs2_value)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amoor {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                memory.write(u64::from(rs1_value), old | rs2_value)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amomin {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                let new = if old.cast_signed() < rs2_value.cast_signed() {
                    old
                } else {
                    rs2_value
                };
                memory.write(u64::from(rs1_value), new)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amomax {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                let new = if old.cast_signed() > rs2_value.cast_signed() {
                    old
                } else {
                    rs2_value
                };
                memory.write(u64::from(rs1_value), new)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amominu {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                let new = if old < rs2_value { old } else { rs2_value };
                memory.write(u64::from(rs1_value), new)?;
                ExecutionResult::Continue { rd, value: old }
            }
            Self::Amomaxu {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let old = memory.read::<u32>(u64::from(rs1_value))?;
                let new = if old > rs2_value { old } else { rs2_value };
                memory.write(u64::from(rs1_value), new)?;
                ExecutionResult::Continue { rd, value: old }
            }
        }
    }
}
