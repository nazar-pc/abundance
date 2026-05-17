//! Zcb compressed instruction execution

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Rv64ZcbInstruction<Reg> where Reg: Register<Type = u64> {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv64ZcbInstruction<Reg>
where
    Reg: Register<Type = u64>,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZcbInstruction<Reg>
where
    Reg: Register<Type = u64>,
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
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::CLbu { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = memory.read::<u8>(addr)?;
                Ok(ControlFlow::Continue((rd, u64::from(value))))
            }
            Self::CLh { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = i64::from(memory.read::<i16>(addr)?);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CLhu { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = memory.read::<u16>(addr)?;
                Ok(ControlFlow::Continue((rd, u64::from(value))))
            }
            Self::CSb {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value as u8)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CSh {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value as u16)?;

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CZextB { rd } => {
                let value = regs.read(rd) & 0xff;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSextB { rd } => {
                let value = i64::from(regs.read(rd) as i8);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CZextH { rd } => {
                let value = regs.read(rd) & 0xffff;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSextH { rd } => {
                let value = i64::from(regs.read(rd) as i16);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CZextW { rd } => {
                let value = regs.read(rd) & 0xffff_ffff;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CNot { rd } => {
                let value = !regs.read(rd);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CMul { rd, rs2: _ } => {
                let value = regs.read(rd).wrapping_mul(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
        }
    }
}
