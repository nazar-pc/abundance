//! RV32 Zcmp extension

pub mod rv32_zcmp_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter, RegisterFile,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32ZcmpInstruction<Reg>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::CmPush { urlist, stack_adj } => {
                rv32_zcmp_helpers::do_push(state, urlist, stack_adj)?;
            }
            Self::CmPop { urlist, stack_adj } => {
                rv32_zcmp_helpers::do_pop(state, urlist, stack_adj)?;
            }
            Self::CmPopretz { urlist, stack_adj } => {
                let ra_val = rv32_zcmp_helpers::do_pop(state, urlist, stack_adj)?;
                // Zero a0 before returning
                state.regs.write(Reg::A0, 0);
                // Jump to ra with LSB cleared (RISC-V mode bit)
                let target = ra_val & !1;
                return state
                    .instruction_fetcher
                    .set_pc(&state.memory, target)
                    .map_err(ExecutionError::from);
            }
            Self::CmPopret { urlist, stack_adj } => {
                let ra_val = rv32_zcmp_helpers::do_pop(state, urlist, stack_adj)?;
                // Jump to ra with LSB cleared (RISC-V mode bit)
                let target = ra_val & !1;
                return state
                    .instruction_fetcher
                    .set_pc(&state.memory, target)
                    .map_err(ExecutionError::from);
            }
            Self::CmMva01s { r1s, r2s } => {
                // Read both sources before any write to avoid aliasing
                let v1 = state.regs.read(r1s);
                let v2 = state.regs.read(r2s);
                state.regs.write(Reg::A0, v1);
                state.regs.write(Reg::A1, v2);
            }
            Self::CmMvsa01 { r1s, r2s } => {
                // Read both sources before any write to avoid aliasing
                let a0_val = state.regs.read(Reg::A0);
                let a1_val = state.regs.read(Reg::A1);
                state.regs.write(r1s, a0_val);
                state.regs.write(r2s, a1_val);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
