//! Zcb compressed instruction execution (RV32)
//!
//! C.ZEXT.W is absent in RV32 - the enum has no such variant.

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2Operands,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZcbInstruction<Reg>
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
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::CLbu { rd, rs1, uimm } => {
                let addr = u64::from(regs.read(rs1).wrapping_add(u32::from(uimm)));
                let value = memory.read::<u8>(addr)?;
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::CLh { rd, rs1, uimm } => {
                let addr = u64::from(regs.read(rs1).wrapping_add(u32::from(uimm)));
                let value = i32::from(memory.read::<i16>(addr)?);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CLhu { rd, rs1, uimm } => {
                let addr = u64::from(regs.read(rs1).wrapping_add(u32::from(uimm)));
                let value = memory.read::<u16>(addr)?;
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::CSb { rs1, rs2, uimm } => {
                let addr = u64::from(regs.read(rs1).wrapping_add(u32::from(uimm)));
                memory.write(addr, regs.read(rs2) as u8)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CSh { rs1, rs2, uimm } => {
                let addr = u64::from(regs.read(rs1).wrapping_add(u32::from(uimm)));
                memory.write(addr, regs.read(rs2) as u16)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CZextB { rd } => {
                let value = regs.read(rd) & 0xff;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSextB { rd } => {
                let value = i32::from(regs.read(rd) as i8);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CZextH { rd } => {
                let value = regs.read(rd) & 0xffff;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSextH { rd } => {
                let value = i32::from(regs.read(rd) as i16);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CNot { rd } => {
                let value = !regs.read(rd);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CMul { rd, rs2 } => {
                let value = regs.read(rd).wrapping_mul(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
        }
    }
}
