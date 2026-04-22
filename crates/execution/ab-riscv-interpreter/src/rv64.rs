//! Base RISC-V RV64 instruction set

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
    for Rv64Instruction<Reg>
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
                let shamt = regs.read(rs2) & 0x3f;
                let value = regs.read(rs1) << shamt;
                regs.write(rd, value);
            }
            Self::Slt { rd, rs1, rs2 } => {
                let value = regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed();
                regs.write(rd, value as u64);
            }
            Self::Sltu { rd, rs1, rs2 } => {
                let value = regs.read(rs1) < regs.read(rs2);
                regs.write(rd, value as u64);
            }
            Self::Xor { rd, rs1, rs2 } => {
                let value = regs.read(rs1) ^ regs.read(rs2);
                regs.write(rd, value);
            }
            Self::Srl { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x3f;
                let value = regs.read(rs1) >> shamt;
                regs.write(rd, value);
            }
            Self::Sra { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x3f;
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

            Self::Addw { rd, rs1, rs2 } => {
                let sum = (regs.read(rs1) as i32).wrapping_add(regs.read(rs2) as i32);
                regs.write(rd, i64::from(sum).cast_unsigned());
            }
            Self::Subw { rd, rs1, rs2 } => {
                let diff = (regs.read(rs1) as i32).wrapping_sub(regs.read(rs2) as i32);
                regs.write(rd, i64::from(diff).cast_unsigned());
            }
            Self::Sllw { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let shifted = (regs.read(rs1) as u32) << shamt;
                regs.write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Srlw { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let shifted = (regs.read(rs1) as u32) >> shamt;
                regs.write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Sraw { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let shifted = (regs.read(rs1) as i32) >> shamt;
                regs.write(rd, i64::from(shifted).cast_unsigned());
            }

            Self::Addi { rd, rs1, imm } => {
                let value = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                regs.write(rd, value);
            }
            Self::Slti { rd, rs1, imm } => {
                let value = regs.read(rs1).cast_signed() < i64::from(imm);
                regs.write(rd, value as u64);
            }
            Self::Sltiu { rd, rs1, imm } => {
                let value = regs.read(rs1) < i64::from(imm).cast_unsigned();
                regs.write(rd, value as u64);
            }
            Self::Xori { rd, rs1, imm } => {
                let value = regs.read(rs1) ^ i64::from(imm).cast_unsigned();
                regs.write(rd, value);
            }
            Self::Ori { rd, rs1, imm } => {
                let value = regs.read(rs1) | i64::from(imm).cast_unsigned();
                regs.write(rd, value);
            }
            Self::Andi { rd, rs1, imm } => {
                let value = regs.read(rs1) & i64::from(imm).cast_unsigned();
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

            Self::Addiw { rd, rs1, imm } => {
                let sum = (regs.read(rs1) as i32).wrapping_add(i32::from(imm));
                regs.write(rd, i64::from(sum).cast_unsigned());
            }
            Self::Slliw { rd, rs1, shamt } => {
                let shifted = (regs.read(rs1) as u32) << shamt;
                regs.write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Srliw { rd, rs1, shamt } => {
                let shifted = (regs.read(rs1) as u32) >> shamt;
                regs.write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Sraiw { rd, rs1, shamt } => {
                let shifted = (regs.read(rs1) as i32) >> shamt;
                regs.write(rd, i64::from(shifted).cast_unsigned());
            }

            Self::Lb { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(memory.read::<i8>(addr)?);
                regs.write(rd, value.cast_unsigned());
            }
            Self::Lh { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(memory.read::<i16>(addr)?);
                regs.write(rd, value.cast_unsigned());
            }
            Self::Lw { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(memory.read::<i32>(addr)?);
                regs.write(rd, value.cast_unsigned());
            }
            Self::Ld { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u64>(addr)?;
                regs.write(rd, value);
            }
            Self::Lbu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u8>(addr)?;
                regs.write(rd, value as u64);
            }
            Self::Lhu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u16>(addr)?;
                regs.write(rd, value as u64);
            }
            Self::Lwu { rd, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u32>(addr)?;
                regs.write(rd, value as u64);
            }

            Self::Jalr { rd, rs1, imm } => {
                let target = (regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned())) & !1u64;
                regs.write(rd, program_counter.get_pc());
                return program_counter
                    .set_pc(memory, target)
                    .map_err(ExecutionError::from);
            }

            Self::Sb { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, regs.read(rs2) as u8)?;
            }
            Self::Sh { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, regs.read(rs2) as u16)?;
            }
            Self::Sw { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, regs.read(rs2) as u32)?;
            }
            Self::Sd { rs2, rs1, imm } => {
                let addr = regs.read(rs1).wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, regs.read(rs2))?;
            }

            Self::Beq { rs1, rs2, imm } => {
                if regs.read(rs1) == regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bne { rs1, rs2, imm } => {
                if regs.read(rs1) != regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Blt { rs1, rs2, imm } => {
                if regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed() {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bge { rs1, rs2, imm } => {
                if regs.read(rs1).cast_signed() >= regs.read(rs2).cast_signed() {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bltu { rs1, rs2, imm } => {
                if regs.read(rs1) < regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bgeu { rs1, rs2, imm } => {
                if regs.read(rs1) >= regs.read(rs2) {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return program_counter
                        .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
                        .map_err(ExecutionError::from);
                }
            }

            Self::Lui { rd, imm } => {
                regs.write(rd, i64::from(imm).cast_unsigned());
            }

            Self::Auipc { rd, imm } => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                regs.write(rd, old_pc.wrapping_add(i64::from(imm).cast_unsigned()));
            }

            Self::Jal { rd, imm } => {
                let pc = program_counter.get_pc();
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                regs.write(rd, pc);
                return program_counter
                    .set_pc(memory, old_pc.wrapping_add(i64::from(imm).cast_unsigned()))
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
