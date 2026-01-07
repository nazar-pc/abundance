//! Part of the interpreter responsible for RISC-V RV64 base instruction set

#[cfg(test)]
mod tests;

use crate::{ExecuteError, GenericInstructionHandler, VirtualMemory};
use ab_riscv_primitives::instruction::Rv64MBZbcInstruction;
use ab_riscv_primitives::instruction::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::{GenericRegister, Registers};
use core::fmt;

#[inline(always)]
pub fn execute_rv64<Reg, Memory, InstructionHandler, CustomError>(
    regs: &mut Registers<Reg>,
    memory: &mut Memory,
    pc: &mut u64,
    instruction_handlers: &mut InstructionHandler,
    old_pc: u64,
    instruction: Rv64Instruction<Reg>,
) -> Result<(), ExecuteError<Rv64MBZbcInstruction<Reg>, CustomError>>
where
    Reg: GenericRegister<Type = u64>,
    [(); Reg::N]:,
    Memory: VirtualMemory,
    InstructionHandler:
        GenericInstructionHandler<Rv64MBZbcInstruction<Reg>, Reg, Memory, CustomError>,
    CustomError: fmt::Display,
{
    match instruction {
        Rv64Instruction::Add { rd, rs1, rs2 } => {
            let value = regs.read(rs1).wrapping_add(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64Instruction::Sub { rd, rs1, rs2 } => {
            let value = regs.read(rs1).wrapping_sub(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64Instruction::Sll { rd, rs1, rs2 } => {
            let shamt = regs.read(rs2) & 0x3f;
            let value = regs.read(rs1) << shamt;
            regs.write(rd, value);
        }
        Rv64Instruction::Slt { rd, rs1, rs2 } => {
            let value = regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed();
            regs.write(rd, value as u64);
        }
        Rv64Instruction::Sltu { rd, rs1, rs2 } => {
            let value = regs.read(rs1) < regs.read(rs2);
            regs.write(rd, value as u64);
        }
        Rv64Instruction::Xor { rd, rs1, rs2 } => {
            let value = regs.read(rs1) ^ regs.read(rs2);
            regs.write(rd, value);
        }
        Rv64Instruction::Srl { rd, rs1, rs2 } => {
            let shamt = regs.read(rs2) & 0x3f;
            let value = regs.read(rs1) >> shamt;
            regs.write(rd, value);
        }
        Rv64Instruction::Sra { rd, rs1, rs2 } => {
            let shamt = regs.read(rs2) & 0x3f;
            let value = regs.read(rs1).cast_signed() >> shamt;
            regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Or { rd, rs1, rs2 } => {
            let value = regs.read(rs1) | regs.read(rs2);
            regs.write(rd, value);
        }
        Rv64Instruction::And { rd, rs1, rs2 } => {
            let value = regs.read(rs1) & regs.read(rs2);
            regs.write(rd, value);
        }

        Rv64Instruction::Addw { rd, rs1, rs2 } => {
            let sum = (regs.read(rs1) as i32).wrapping_add(regs.read(rs2) as i32);
            regs.write(rd, (sum as i64).cast_unsigned());
        }
        Rv64Instruction::Subw { rd, rs1, rs2 } => {
            let diff = (regs.read(rs1) as i32).wrapping_sub(regs.read(rs2) as i32);
            regs.write(rd, (diff as i64).cast_unsigned());
        }
        Rv64Instruction::Sllw { rd, rs1, rs2 } => {
            let shamt = regs.read(rs2) & 0x1f;
            let shifted = (regs.read(rs1) as u32) << shamt;
            regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Srlw { rd, rs1, rs2 } => {
            let shamt = regs.read(rs2) & 0x1f;
            let shifted = (regs.read(rs1) as u32) >> shamt;
            regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Sraw { rd, rs1, rs2 } => {
            let shamt = regs.read(rs2) & 0x1f;
            let shifted = (regs.read(rs1) as i32) >> shamt;
            regs.write(rd, (shifted as i64).cast_unsigned());
        }

        Rv64Instruction::Addi { rd, rs1, imm } => {
            let value = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            regs.write(rd, value);
        }
        Rv64Instruction::Slti { rd, rs1, imm } => {
            let value = regs.read(rs1).cast_signed() < (imm as i64);
            regs.write(rd, value as u64);
        }
        Rv64Instruction::Sltiu { rd, rs1, imm } => {
            let value = regs.read(rs1) < ((imm as i64).cast_unsigned());
            regs.write(rd, value as u64);
        }
        Rv64Instruction::Xori { rd, rs1, imm } => {
            let value = regs.read(rs1) ^ ((imm as i64).cast_unsigned());
            regs.write(rd, value);
        }
        Rv64Instruction::Ori { rd, rs1, imm } => {
            let value = regs.read(rs1) | ((imm as i64).cast_unsigned());
            regs.write(rd, value);
        }
        Rv64Instruction::Andi { rd, rs1, imm } => {
            let value = regs.read(rs1) & ((imm as i64).cast_unsigned());
            regs.write(rd, value);
        }
        Rv64Instruction::Slli { rd, rs1, shamt } => {
            let value = regs.read(rs1) << shamt;
            regs.write(rd, value);
        }
        Rv64Instruction::Srli { rd, rs1, shamt } => {
            let value = regs.read(rs1) >> shamt;
            regs.write(rd, value);
        }
        Rv64Instruction::Srai { rd, rs1, shamt } => {
            let value = regs.read(rs1).cast_signed() >> shamt;
            regs.write(rd, value.cast_unsigned());
        }

        Rv64Instruction::Addiw { rd, rs1, imm } => {
            let sum = (regs.read(rs1) as i32).wrapping_add(imm);
            regs.write(rd, (sum as i64).cast_unsigned());
        }
        Rv64Instruction::Slliw { rd, rs1, shamt } => {
            let shifted = (regs.read(rs1) as u32) << shamt;
            regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Srliw { rd, rs1, shamt } => {
            let shifted = (regs.read(rs1) as u32) >> shamt;
            regs.write(rd, (shifted.cast_signed() as i64).cast_unsigned());
        }
        Rv64Instruction::Sraiw { rd, rs1, shamt } => {
            let shifted = (regs.read(rs1) as i32) >> shamt;
            regs.write(rd, (shifted as i64).cast_unsigned());
        }

        Rv64Instruction::Lb { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<i8>(addr)? as i64;
            regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Lh { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<i16>(addr)? as i64;
            regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Lw { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<i32>(addr)? as i64;
            regs.write(rd, value.cast_unsigned());
        }
        Rv64Instruction::Ld { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<u64>(addr)?;
            regs.write(rd, value);
        }
        Rv64Instruction::Lbu { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<u8>(addr)?;
            regs.write(rd, value as u64);
        }
        Rv64Instruction::Lhu { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<u16>(addr)?;
            regs.write(rd, value as u64);
        }
        Rv64Instruction::Lwu { rd, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            let value = memory.read::<u32>(addr)?;
            regs.write(rd, value as u64);
        }

        Rv64Instruction::Jalr { rd, rs1, imm } => {
            let target = (regs.read(rs1).wrapping_add((imm as i64).cast_unsigned())) & !1u64;
            regs.write(rd, *pc);
            *pc = target;
        }

        Rv64Instruction::Sb { rs2, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            memory.write(addr, regs.read(rs2) as u8)?;
        }
        Rv64Instruction::Sh { rs2, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            memory.write(addr, regs.read(rs2) as u16)?;
        }
        Rv64Instruction::Sw { rs2, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            memory.write(addr, regs.read(rs2) as u32)?;
        }
        Rv64Instruction::Sd { rs2, rs1, imm } => {
            let addr = regs.read(rs1).wrapping_add((imm as i64).cast_unsigned());
            memory.write(addr, regs.read(rs2))?;
        }

        Rv64Instruction::Beq { rs1, rs2, imm } => {
            if regs.read(rs1) == regs.read(rs2) {
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }
        }
        Rv64Instruction::Bne { rs1, rs2, imm } => {
            if regs.read(rs1) != regs.read(rs2) {
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }
        }
        Rv64Instruction::Blt { rs1, rs2, imm } => {
            if regs.read(rs1).cast_signed() < regs.read(rs2).cast_signed() {
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }
        }
        Rv64Instruction::Bge { rs1, rs2, imm } => {
            if regs.read(rs1).cast_signed() >= regs.read(rs2).cast_signed() {
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }
        }
        Rv64Instruction::Bltu { rs1, rs2, imm } => {
            if regs.read(rs1) < regs.read(rs2) {
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }
        }
        Rv64Instruction::Bgeu { rs1, rs2, imm } => {
            if regs.read(rs1) >= regs.read(rs2) {
                *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
            }
        }

        Rv64Instruction::Lui { rd, imm } => {
            regs.write(rd, (imm as i64).cast_unsigned());
        }

        Rv64Instruction::Auipc { rd, imm } => {
            regs.write(rd, old_pc.wrapping_add((imm as i64).cast_unsigned()));
        }

        Rv64Instruction::Jal { rd, imm } => {
            regs.write(rd, *pc);
            *pc = old_pc.wrapping_add((imm as i64).cast_unsigned());
        }

        Rv64Instruction::Fence { .. } => {
            // NOP for single-threaded
        }

        Rv64Instruction::Ecall => {
            instruction_handlers.handle_ecall(
                regs,
                memory,
                pc,
                Rv64MBZbcInstruction::Base(Rv64Instruction::Ecall),
            )?;
        }
        Rv64Instruction::Ebreak => {
            instruction_handlers.handle_ebreak(
                regs,
                memory,
                pc,
                Rv64MBZbcInstruction::Base(Rv64Instruction::Ebreak),
            )?;
        }

        Rv64Instruction::Unimp => {
            return Err(ExecuteError::UnimpInstruction { address: old_pc });
        }

        Rv64Instruction::Invalid(instruction) => {
            return Err(ExecuteError::InvalidInstruction {
                address: old_pc,
                instruction,
            });
        }
    }

    Ok(())
}
