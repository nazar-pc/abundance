//! Base RISC-V RV32 instruction set

pub mod b;
pub mod c;
pub mod m;
#[cfg(test)]
pub(crate) mod test_utils;
#[cfg(test)]
mod tests;
pub mod zce;
pub mod zk;

use crate::{
    ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile, SystemInstructionHandler,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32Instruction<Reg>
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
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Add { rd, rs1, rs2 } => {
                let value = regs.read(rs1).wrapping_add(regs.read(rs2));
                regs.write(rd, value);
            }
            Self::Sub { rd, rs1, rs2 } => {
                let value = regs.read(rs1).wrapping_sub(regs.read(rs2));
                regs.write(rd, value);
            }
            Self::Sll { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let value = regs.read(rs1) << shamt;
                regs.write(rd, value);
            }
            Self::Slt { rd, rs1, rs2 } => {
                let value = regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed();
                regs.write(rd, value as u32);
            }
            Self::Sltu { rd, rs1, rs2 } => {
                let value = regs.read(rs1) < regs.read(rs2);
                regs.write(rd, value as u32);
            }
            Self::Xor { rd, rs1, rs2 } => {
                let value = regs.read(rs1) ^ regs.read(rs2);
                regs.write(rd, value);
            }
            Self::Srl { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let value = regs.read(rs1) >> shamt;
                regs.write(rd, value);
            }
            Self::Sra { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let value = regs.read(rs1).cast_signed() >> shamt;
                regs.write(rd, value.cast_unsigned());
            }
            Self::Or { rd, rs1, rs2 } => {
                let value = regs.read(rs1) | regs.read(rs2);
                regs.write(rd, value);
            }
            Self::And { rd, rs1, rs2 } => {
                let value = regs.read(rs1) & regs.read(rs2);
                regs.write(rd, value);
            }

            Self::Addi { rd, rs1, imm } => {
                let value = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                regs.write(rd, value);
            }
            Self::Slti { rd, rs1, imm } => {
                let value = regs.read(rs1).cast_signed() < i32::from(imm);
                regs.write(rd, value as u32);
            }
            Self::Sltiu { rd, rs1, imm } => {
                let value = regs.read(rs1) < i32::from(imm).cast_unsigned();
                regs.write(rd, value as u32);
            }
            Self::Xori { rd, rs1, imm } => {
                let value = regs.read(rs1) ^ i32::from(imm).cast_unsigned();
                regs.write(rd, value);
            }
            Self::Ori { rd, rs1, imm } => {
                let value = regs.read(rs1) | i32::from(imm).cast_unsigned();
                regs.write(rd, value);
            }
            Self::Andi { rd, rs1, imm } => {
                let value = regs.read(rs1) & i32::from(imm).cast_unsigned();
                regs.write(rd, value);
            }
            Self::Slli { rd, rs1, shamt } => {
                let value = regs.read(rs1) << shamt;
                regs.write(rd, value);
            }
            Self::Srli { rd, rs1, shamt } => {
                let value = regs.read(rs1) >> shamt;
                regs.write(rd, value);
            }
            Self::Srai { rd, rs1, shamt } => {
                let value = regs.read(rs1).cast_signed() >> shamt;
                regs.write(rd, value.cast_unsigned());
            }

            Self::Lb { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                let value = i32::from(memory.read::<i8>(u64::from(addr))?);
                regs.write(rd, value.cast_unsigned());
            }
            Self::Lh { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                let value = i32::from(memory.read::<i16>(u64::from(addr))?);
                regs.write(rd, value.cast_unsigned());
            }
            Self::Lw { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u32>(u64::from(addr))?;
                regs.write(rd, value);
            }
            Self::Lbu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u8>(u64::from(addr))?;
                regs.write(rd, value as u32);
            }
            Self::Lhu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u16>(u64::from(addr))?;
                regs.write(rd, value as u32);
            }

            Self::Jalr { rd, rs1, imm } => {
                let target = (regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned())) & !1u32;
                regs.write(rd, program_counter.get_pc());
                return program_counter
                    .set_pc(memory, target)
                    .map_err(ExecutionError::from);
            }

            Self::Sb { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), regs.read(rs2) as u8)?;
            }
            Self::Sh { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), regs.read(rs2) as u16)?;
            }
            Self::Sw { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), regs.read(rs2))?;
            }

            Self::Beq { rs1, rs2, imm } => {
                if regs.read(rs1) == regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bne { rs1, rs2, imm } => {
                if regs.read(rs1) != regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Blt { rs1, rs2, imm } => {
                if regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed() {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bge { rs1, rs2, imm } => {
                if regs.read(rs1).cast_signed() >= regs.read(rs2).cast_signed() {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bltu { rs1, rs2, imm } => {
                if regs.read(rs1) < regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bgeu { rs1, rs2, imm } => {
                if regs.read(rs1) >= regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }

            Self::Lui { rd, imm } => {
                regs.write(rd, imm.cast_unsigned());
            }

            Self::Auipc { rd, imm } => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                regs.write(rd, old_pc.wrapping_add(imm.cast_unsigned()));
            }

            Self::Jal { rd, imm } => {
                let pc = program_counter.get_pc();
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                regs.write(rd, pc);
                return program_counter
                    .set_pc(memory, old_pc.wrapping_add(imm.cast_unsigned()))
                    .map_err(ExecutionError::from);
            }

            Self::Fence { pred, succ } => {
                system_instruction_handler.handle_fence(pred, succ);
            }
            Self::FenceTso => {
                system_instruction_handler.handle_fence_tso();
            }

            Self::Ecall => {
                return system_instruction_handler.handle_ecall(regs, memory, program_counter);
            }
            Self::Ebreak => {
                system_instruction_handler.handle_ebreak(regs, memory, program_counter.get_pc());
            }

            Self::Unimp => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                return Err(ExecutionError::IllegalInstruction { address: old_pc });
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
