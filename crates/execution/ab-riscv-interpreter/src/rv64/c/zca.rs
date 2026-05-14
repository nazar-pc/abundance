//! RV64 Zca extension

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
    for Rv64ZcaInstruction<Reg>
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
                    sp_val.wrapping_add(u64::from(nzuimm)),
                )))
            }
            Self::CLw { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = i64::from(memory.read::<i32>(addr)?);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CLd { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = memory.read::<u64>(addr)?;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSw {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value as u32)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CSd {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value)?;
                Ok(ControlFlow::Continue(Default::default()))
            }

            // Quadrant 01
            Self::CNop => {}
            Self::CAddi { rd, nzimm } => {
                let value = regs.read(rd).wrapping_add(i64::from(nzimm).cast_unsigned());
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CAddiw { rd, imm } => {
                let sum = (regs.read(rd) as i32).wrapping_add(i32::from(imm));
                Ok(ControlFlow::Continue((rd, i64::from(sum).cast_unsigned())))
            }
            Self::CLi { rd, imm } => {
                Ok(ControlFlow::Continue((rd, i64::from(imm).cast_unsigned())))
            }
            Self::CAddi16sp { nzimm } => {
                let value = regs
                    .read(Reg::SP)
                    .wrapping_add(i64::from(nzimm).cast_unsigned());
                Ok(ControlFlow::Continue((Reg::SP, value)))
            }
            Self::CLui { rd, nzimm } => Ok(ControlFlow::Continue((
                rd,
                i64::from(nzimm).cast_unsigned(),
            ))),
            Self::CSrli { rd, shamt } => {
                let value = regs.read(rd) >> shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSrai { rd, shamt } => {
                let value = regs.read(rd).cast_signed() >> shamt;
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CAndi { rd, imm } => {
                let value = regs.read(rd) & i64::from(imm).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSub { rd, rs2: _ } => {
                let value = regs.read(rd).wrapping_sub(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CXor { rd, rs2: _ } => {
                let value = regs.read(rd) ^ rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::COr { rd, rs2: _ } => {
                let value = regs.read(rd) | rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CAnd { rd, rs2: _ } => {
                let value = regs.read(rd) & rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSubw { rd, rs2: _ } => {
                let diff = (regs.read(rd) as i32).wrapping_sub(rs2_value as i32);
                Ok(ControlFlow::Continue((rd, i64::from(diff).cast_unsigned())))
            }
            Self::CAddw { rd, rs2: _ } => {
                let sum = (regs.read(rd) as i32).wrapping_add(rs2_value as i32);
                Ok(ControlFlow::Continue((rd, i64::from(sum).cast_unsigned())))
            }
            Self::CJ { imm } => {
                let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                return program_counter
                    .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from);
            }
            Self::CBeqz { rs1: _, imm } => {
                if rs1_value == 0 {
                    let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                        .map_err(ExecutionError::from);
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CBnez { rs1: _, imm } => {
                if rs1_value != 0 {
                    let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
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
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                let value = i64::from(memory.read::<i32>(addr)?);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::CLdsp { rd, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                let value = memory.read::<u64>(addr)?;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CJr { rs1: _ } => {
                let target = rs1_value & !1;
                return program_counter
                    .set_pc(memory, target)
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from);
            }
            Self::CMv { rd, rs2: _ } => Ok(ControlFlow::Continue((rd, rs2_value))),
            Self::CEbreak => {
                system_instruction_handler.handle_ebreak(regs, memory, program_counter.get_pc());
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CJalr { rs1: _ } => {
                let target = rs1_value & !1;
                let return_addr = program_counter.get_pc();
                regs.write(Reg::RA, return_addr);
                return program_counter
                    .set_pc(memory, target)
                    .map(|control_flow| control_flow.map_continue(|()| Default::default()))
                    .map_err(ExecutionError::from);
            }
            Self::CAdd { rd, rs2: _ } => {
                let value = regs.read(rd).wrapping_add(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::CSwsp { rs2: _, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value as u32)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CSdsp { rs2: _, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::CUnimp => {
                let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                return Err(ExecutionError::IllegalInstruction { address: old_pc });
            }
        }
    }
}
