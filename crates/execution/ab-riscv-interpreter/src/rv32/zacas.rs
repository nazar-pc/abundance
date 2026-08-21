//! RV32 Zacas extension

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
const impl<Reg> ExecutableInstructionOperands for Rv32ZacasInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32ZacasInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv32ZacasInstruction<Reg>
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
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::AmocasW {
                rd,
                rs1: _,
                rs2: _,
                aq: _,
                rl: _,
            } => {
                let addr = u64::from(rs1_value);
                let compare = regs.read(rd);
                let old = memory.read::<u32>(addr)?;
                if old == compare {
                    memory.write(addr, rs2_value)?;
                }
                ExecutionResult::Continue { rd, value: old }
            }
            Self::AmocasD {
                rd,
                rs1: _,
                rs2,
                rd_hi,
                rs2_hi,
                aq: _,
                rl: _,
            } => {
                let addr = u64::from(rs1_value);
                // Per spec, when the first register of a pair is `x0`, BOTH halves of that pair
                // read as zero - not just the literal `x0` half. `compare_lo`/`rs2_value` are
                // already 0 in that case since `x0` is hardwired, but `compare_hi`/`swap_hi`
                // need an explicit override since `rd_hi`/`rs2_hi` are real registers.
                let compare_lo = regs.read(rd);
                let compare_hi = if rd == Reg::ZERO { 0 } else { regs.read(rd_hi) };
                let swap_hi = if rs2 == Reg::ZERO {
                    0
                } else {
                    regs.read(rs2_hi)
                };
                let old_lo = memory.read::<u32>(addr)?;
                let old_hi = memory.read::<u32>(addr + 4)?;
                if old_lo == compare_lo && old_hi == compare_hi {
                    memory.write(addr, rs2_value)?;
                    memory.write(addr + 4, swap_hi)?;
                }
                // Per spec, when `rd == x0` the whole register-pair write (both halves) is
                // skipped, not just the low half (which is a no-op anyway since x0 is
                // hardwired). Only `rd_hi` needs an explicit guard since it's a real register.
                if rd != Reg::ZERO {
                    regs.write(rd_hi, old_hi);
                }
                ExecutionResult::Continue { rd, value: old_lo }
            }
        }
    }
}
