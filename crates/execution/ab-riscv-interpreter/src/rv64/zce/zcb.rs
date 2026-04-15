//! Zcb compressed instruction execution

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZcbInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: SystemInstructionHandler<Reg, Memory, PC, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::CLbu { rd, rs1, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                let value = state.memory.read::<u8>(addr)?;
                state.regs.write(rd, u64::from(value));
            }
            Self::CLh { rd, rs1, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                let value = i64::from(state.memory.read::<i16>(addr)?);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::CLhu { rd, rs1, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                let value = state.memory.read::<u16>(addr)?;
                state.regs.write(rd, u64::from(value));
            }
            Self::CSb { rs1, rs2, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                state.memory.write(addr, state.regs.read(rs2) as u8)?;
            }
            Self::CSh { rs1, rs2, uimm } => {
                let addr = state.regs.read(rs1).wrapping_add(u64::from(uimm));
                state.memory.write(addr, state.regs.read(rs2) as u16)?;
            }
            Self::CZextB { rd } => {
                let value = state.regs.read(rd) & 0xff;
                state.regs.write(rd, value);
            }
            Self::CSextB { rd } => {
                let value = i64::from(state.regs.read(rd) as i8);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::CZextH { rd } => {
                let value = state.regs.read(rd) & 0xffff;
                state.regs.write(rd, value);
            }
            Self::CSextH { rd } => {
                let value = i64::from(state.regs.read(rd) as i16);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::CZextW { rd } => {
                let value = state.regs.read(rd) & 0xffff_ffff;
                state.regs.write(rd, value);
            }
            Self::CNot { rd } => {
                let value = !state.regs.read(rd);
                state.regs.write(rd, value);
            }
            Self::CMul { rd, rs2 } => {
                let value = state.regs.read(rd).wrapping_mul(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
