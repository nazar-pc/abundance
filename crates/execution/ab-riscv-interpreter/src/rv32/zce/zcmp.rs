//! RV32 Zcmp extension

pub mod rv32_zcmp_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZcmpInstruction<Reg>
where
    Reg: ZcmpRegister<Type = u32>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
{
    #[inline(always)]
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
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::CmPush { urlist, stack_adj } => {
                rv32_zcmp_helpers::do_push(regs, memory, urlist, stack_adj)
            }
            Self::CmPop { urlist, stack_adj } => {
                rv32_zcmp_helpers::do_pop(regs, memory, urlist, stack_adj)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CmPopretz { urlist, stack_adj } => {
                let ra_val = rv32_zcmp_helpers::do_pop(regs, memory, urlist, stack_adj)?;
                // Zero a0 before returning
                regs.write(Reg::A0, 0);
                // Jump to ra with LSB cleared (RISC-V mode bit)
                let target = ra_val & !1;
                program_counter
                    .set_pc(memory, target)
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from)
            }
            Self::CmPopret { urlist, stack_adj } => {
                let ra_val = rv32_zcmp_helpers::do_pop(regs, memory, urlist, stack_adj)?;
                // Jump to ra with LSB cleared (RISC-V mode bit)
                let target = ra_val & !1;
                program_counter
                    .set_pc(memory, target)
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from)
            }
            Self::CmMva01s { rs1: _, rs2: _ } => {
                // Read both sources before any write to avoid aliasing
                let v1 = rs1_value;
                let v2 = rs2_value;
                regs.write(Reg::A0, v1);
                Ok(ControlFlow::Continue((Reg::A1, v2)))
            }
            Self::CmMvsa01 { rs1, rs2 } => {
                // Read both sources before any write to avoid aliasing
                let a0_val = regs.read(Reg::A0);
                let a1_val = regs.read(Reg::A1);
                regs.write(rs1, a0_val);
                Ok(ControlFlow::Continue((rs2, a1_val)))
            }
        }
    }
}
