//! RV64 Zcmp extension

pub mod rv64_zcmp_helpers;
use crate::PackedAddress;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, ProgramCounter, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler, ThreadedExecutableInstruction,
    ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZcmpInstruction<Reg> where
    Reg: ZcmpRegister<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZcmpInstruction<Reg> where
    Reg: ZcmpRegister<Type = u64>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZcmpInstruction<Reg>
where
    Reg: ZcmpRegister<Type = u64>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        ExecutionResult::ContinueNoWrite
    }
}

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZcmpOnlyInstruction<Reg> where
    Reg: ZcmpRegister<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZcmpOnlyInstruction<Reg> where
    Reg: ZcmpRegister<Type = u64>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZcmpOnlyInstruction<Reg>
where
    Reg: ZcmpRegister<Type = u64>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
            Self::CmPush { urlist, stack_adj } => {
                rv64_zcmp_helpers::do_push(regs, memory, urlist, stack_adj)
            }
            Self::CmPop { urlist, stack_adj } => {
                rv64_zcmp_helpers::do_pop(regs, memory, urlist, stack_adj)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::CmPopretz { urlist, stack_adj } => {
                let ra_val = rv64_zcmp_helpers::do_pop(regs, memory, urlist, stack_adj)?;
                // Zero a0 before returning
                regs.write(Reg::A0, 0);
                // Jump to ra with LSB cleared (RISC-V mode bit)
                let target = ra_val & !1;
                ExecutionResult::Jump { target }
            }
            Self::CmPopret { urlist, stack_adj } => {
                let ra_val = rv64_zcmp_helpers::do_pop(regs, memory, urlist, stack_adj)?;
                // Jump to ra with LSB cleared (RISC-V mode bit)
                let target = ra_val & !1;
                ExecutionResult::Jump { target }
            }
            Self::CmMva01s { rs1: _, rs2: _ } => {
                // Read both sources before any write to avoid aliasing
                let v1 = rs1_value;
                let v2 = rs2_value;
                regs.write(Reg::A0, v1);
                ExecutionResult::Continue {
                    rd: Reg::A1,
                    value: v2,
                }
            }
            Self::CmMvsa01 { rs1, rs2 } => {
                // Read both sources before any write to avoid aliasing
                let a0_val = regs.read(Reg::A0);
                let a1_val = regs.read(Reg::A1);
                regs.write(rs1, a0_val);
                ExecutionResult::Continue {
                    rd: rs2,
                    value: a1_val,
                }
            }
        }
    }
}
