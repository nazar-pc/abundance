//! RV32 Zca extension

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
    for Rv32ZcaInstruction<Reg>
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
        program_counter: &mut PC,
        system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            // Quadrant 00
            Self::CAddi4spn { rd, nzuimm } => {
                let sp_val = regs.read(Reg::SP);
                Ok(ControlFlow::Continue((
                    rd,
                    sp_val.wrapping_add(u32::from(nzuimm)),
                )))
            }
            Self::CLw { rd, rs1, uimm } => {
                let addr = regs.read(rs1).wrapping_add(u32::from(uimm));
                let value = memory.read::<i32>(u64::from(addr))?.cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSw { rs1, rs2, uimm } => {
                let addr = regs.read(rs1).wrapping_add(u32::from(uimm));
                memory.write(u64::from(addr), regs.read(rs2))?;
                Ok(ControlFlow::Continue(Default::default()))
            }

            // Quadrant 01
            Self::CNop => Ok(ControlFlow::Continue(Default::default())),
            Self::CAddi { rd, nzimm } => {
                let value = regs.read(rd).wrapping_add(i32::from(nzimm).cast_unsigned());
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CJal { imm } => {
                let return_addr = program_counter.get_pc();
                regs.write(Reg::RA, return_addr);
                let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                program_counter
                    .set_pc(memory, old_pc.wrapping_add(i32::from(imm).cast_unsigned()))
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from)
            }
            Self::CLi { rd, imm } => {
                Ok(ControlFlow::Continue((rd, i32::from(imm).cast_unsigned())))
            }
            Self::CAddi16sp { nzimm } => {
                let value = regs
                    .read(Reg::SP)
                    .wrapping_add(i32::from(nzimm).cast_unsigned());
                Ok(ControlFlow::Continue((Reg::SP, value)))
            }
            Self::CLui { rd, nzimm } => {
                Ok(ControlFlow::Continue((rd, nzimm.to_i32().cast_unsigned())))
            }
            Self::CSrli { rd, shamt } => {
                let value = regs.read(rd) >> shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSrai { rd, shamt } => {
                let value = regs.read(rd).cast_signed() >> shamt;
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CAndi { rd, imm } => {
                let value = regs.read(rd) & i32::from(imm).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSub { rd, rs2 } => {
                let value = regs.read(rd).wrapping_sub(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CXor { rd, rs2 } => {
                let value = regs.read(rd) ^ regs.read(rs2);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::COr { rd, rs2 } => {
                let value = regs.read(rd) | regs.read(rs2);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CAnd { rd, rs2 } => {
                let value = regs.read(rd) & regs.read(rs2);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CJ { imm } => {
                let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                program_counter
                    .set_pc(memory, old_pc.wrapping_add(i32::from(imm).cast_unsigned()))
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from)
            }
            Self::CBeqz { rs1, imm } => {
                if regs.read(rs1) == 0 {
                    let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i32::from(imm).cast_unsigned()))
                        .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                        .map_err(ExecutionError::from);
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CBnez { rs1, imm } => {
                if regs.read(rs1) != 0 {
                    let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i32::from(imm).cast_unsigned()))
                        .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                        .map_err(ExecutionError::from);
                }

                Ok(ControlFlow::Continue(Default::default()))
            }

            // Quadrant 10
            Self::CSlli { rd, shamt } => {
                let value = regs.read(rd) << shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CLwsp { rd, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u32::from(uimm));
                let value = memory.read::<i32>(u64::from(addr))?.cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CJr { rs1 } => {
                let target = regs.read(rs1) & !1;
                program_counter
                    .set_pc(memory, target)
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from)
            }
            Self::CMv { rd, rs2 } => Ok(ControlFlow::Continue((rd, regs.read(rs2)))),
            Self::CEbreak => {
                system_instruction_handler.handle_ebreak(regs, memory, program_counter.get_pc());
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CJalr { rs1 } => {
                let target = regs.read(rs1) & !1;
                let return_addr = program_counter.get_pc();
                regs.write(Reg::RA, return_addr);
                return program_counter
                    .set_pc(memory, target)
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from);
            }
            Self::CAdd { rd, rs2 } => {
                let value = regs.read(rd).wrapping_add(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSwsp { rs2, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u32::from(uimm));
                memory.write(u64::from(addr), regs.read(rs2))?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CUnimp => {
                let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                Err(ExecutionError::IllegalInstruction { address: old_pc })
            }
        }
    }
}
