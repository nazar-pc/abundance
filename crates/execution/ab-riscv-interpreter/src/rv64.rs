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
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter,
    SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::Rv64Instruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64Instruction<Reg>
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
            Self::Add { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_add(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sub { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).wrapping_sub(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sll { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x3f;
                let value = state.regs.read(rs1) << shamt;
                state.regs.write(rd, value);
            }
            Self::Slt { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).cast_signed() < state.regs.read(rs2).cast_signed();
                state.regs.write(rd, value as u64);
            }
            Self::Sltu { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) < state.regs.read(rs2);
                state.regs.write(rd, value as u64);
            }
            Self::Xor { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) ^ state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::Srl { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x3f;
                let value = state.regs.read(rs1) >> shamt;
                state.regs.write(rd, value);
            }
            Self::Sra { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x3f;
                let value = state.regs.read(rs1).cast_signed() >> shamt;
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Or { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) | state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::And { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) & state.regs.read(rs2);
                state.regs.write(rd, value);
            }

            Self::Addw { rd, rs1, rs2 } => {
                let sum = (state.regs.read(rs1) as i32).wrapping_add(state.regs.read(rs2) as i32);
                state.regs.write(rd, i64::from(sum).cast_unsigned());
            }
            Self::Subw { rd, rs1, rs2 } => {
                let diff = (state.regs.read(rs1) as i32).wrapping_sub(state.regs.read(rs2) as i32);
                state.regs.write(rd, i64::from(diff).cast_unsigned());
            }
            Self::Sllw { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x1f;
                let shifted = (state.regs.read(rs1) as u32) << shamt;
                state
                    .regs
                    .write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Srlw { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x1f;
                let shifted = (state.regs.read(rs1) as u32) >> shamt;
                state
                    .regs
                    .write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Sraw { rd, rs1, rs2 } => {
                let shamt = state.regs.read(rs2) & 0x1f;
                let shifted = (state.regs.read(rs1) as i32) >> shamt;
                state.regs.write(rd, i64::from(shifted).cast_unsigned());
            }

            Self::Addi { rd, rs1, imm } => {
                let value = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                state.regs.write(rd, value);
            }
            Self::Slti { rd, rs1, imm } => {
                let value = state.regs.read(rs1).cast_signed() < i64::from(imm);
                state.regs.write(rd, value as u64);
            }
            Self::Sltiu { rd, rs1, imm } => {
                let value = state.regs.read(rs1) < i64::from(imm).cast_unsigned();
                state.regs.write(rd, value as u64);
            }
            Self::Xori { rd, rs1, imm } => {
                let value = state.regs.read(rs1) ^ i64::from(imm).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Ori { rd, rs1, imm } => {
                let value = state.regs.read(rs1) | i64::from(imm).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Andi { rd, rs1, imm } => {
                let value = state.regs.read(rs1) & i64::from(imm).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Slli { rd, rs1, shamt } => {
                let value = state.regs.read(rs1) << shamt;
                state.regs.write(rd, value);
            }
            Self::Srli { rd, rs1, shamt } => {
                let value = state.regs.read(rs1) >> shamt;
                state.regs.write(rd, value);
            }
            Self::Srai { rd, rs1, shamt } => {
                let value = state.regs.read(rs1).cast_signed() >> shamt;
                state.regs.write(rd, value.cast_unsigned());
            }

            Self::Addiw { rd, rs1, imm } => {
                let sum = (state.regs.read(rs1) as i32).wrapping_add(i32::from(imm));
                state.regs.write(rd, i64::from(sum).cast_unsigned());
            }
            Self::Slliw { rd, rs1, shamt } => {
                let shifted = (state.regs.read(rs1) as u32) << shamt;
                state
                    .regs
                    .write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Srliw { rd, rs1, shamt } => {
                let shifted = (state.regs.read(rs1) as u32) >> shamt;
                state
                    .regs
                    .write(rd, i64::from(shifted.cast_signed()).cast_unsigned());
            }
            Self::Sraiw { rd, rs1, shamt } => {
                let shifted = (state.regs.read(rs1) as i32) >> shamt;
                state.regs.write(rd, i64::from(shifted).cast_unsigned());
            }

            Self::Lb { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(state.memory.read::<i8>(addr)?);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Lh { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(state.memory.read::<i16>(addr)?);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Lw { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(state.memory.read::<i32>(addr)?);
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Ld { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = state.memory.read::<u64>(addr)?;
                state.regs.write(rd, value);
            }
            Self::Lbu { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = state.memory.read::<u8>(addr)?;
                state.regs.write(rd, value as u64);
            }
            Self::Lhu { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = state.memory.read::<u16>(addr)?;
                state.regs.write(rd, value as u64);
            }
            Self::Lwu { rd, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                let value = state.memory.read::<u32>(addr)?;
                state.regs.write(rd, value as u64);
            }

            Self::Jalr { rd, rs1, imm } => {
                let target = (state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned()))
                    & !1u64;
                state.regs.write(rd, state.instruction_fetcher.get_pc());
                return state
                    .instruction_fetcher
                    .set_pc(&state.memory, target)
                    .map_err(ExecutionError::from);
            }

            Self::Sb { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2) as u8)?;
            }
            Self::Sh { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2) as u16)?;
            }
            Self::Sw { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2) as u32)?;
            }
            Self::Sd { rs2, rs1, imm } => {
                let addr = state
                    .regs
                    .read(rs1)
                    .wrapping_add(i64::from(imm).cast_unsigned());
                state.memory.write(addr, state.regs.read(rs2))?;
            }

            Self::Beq { rs1, rs2, imm } => {
                if state.regs.read(rs1) == state.regs.read(rs2) {
                    let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bne { rs1, rs2, imm } => {
                if state.regs.read(rs1) != state.regs.read(rs2) {
                    let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Blt { rs1, rs2, imm } => {
                if state.regs.read(rs1).cast_signed() < state.regs.read(rs2).cast_signed() {
                    let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bge { rs1, rs2, imm } => {
                if state.regs.read(rs1).cast_signed() >= state.regs.read(rs2).cast_signed() {
                    let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bltu { rs1, rs2, imm } => {
                if state.regs.read(rs1) < state.regs.read(rs2) {
                    let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }
            Self::Bgeu { rs1, rs2, imm } => {
                if state.regs.read(rs1) >= state.regs.read(rs2) {
                    let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                    return state
                        .instruction_fetcher
                        .set_pc(
                            &state.memory,
                            old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                        )
                        .map_err(ExecutionError::from);
                }
            }

            Self::Lui { rd, imm } => {
                state.regs.write(rd, i64::from(imm).cast_unsigned());
            }

            Self::Auipc { rd, imm } => {
                let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                state
                    .regs
                    .write(rd, old_pc.wrapping_add(i64::from(imm).cast_unsigned()));
            }

            Self::Jal { rd, imm } => {
                let pc = state.instruction_fetcher.get_pc();
                let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                state.regs.write(rd, pc);
                return state
                    .instruction_fetcher
                    .set_pc(
                        &state.memory,
                        old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                    )
                    .map_err(ExecutionError::from);
            }

            Self::Fence { pred, succ } => {
                state.system_instruction_handler.handle_fence(pred, succ);
            }
            Self::FenceTso => {
                state.system_instruction_handler.handle_fence_tso();
            }

            Self::Ecall => {
                return state.system_instruction_handler.handle_ecall(
                    &mut state.regs,
                    &mut state.memory,
                    &mut state.instruction_fetcher,
                );
            }
            Self::Ebreak => {
                state.system_instruction_handler.handle_ebreak(
                    &mut state.regs,
                    &mut state.memory,
                    state.instruction_fetcher.get_pc(),
                );
            }

            Self::Unimp => {
                let old_pc = state.instruction_fetcher.old_pc(size_of::<u32>() as u8);
                return Err(ExecutionError::IllegalInstruction { address: old_pc });
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
