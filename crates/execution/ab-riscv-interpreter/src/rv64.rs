//! Base RISC-V RV64 instruction set

pub mod a;
pub mod b;
pub mod c;
pub mod m;
#[cfg(test)]
pub(crate) mod test_utils;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod threaded_tests;
pub mod zabha;
pub mod zacas;
pub mod zalasr;
pub mod zce;
pub mod zk;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    PackedAddress, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    SystemInstructionHandler, ThreadedExecutableInstruction, ThreadedExecutionResult,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64Instruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64Instruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for Rv64Instruction<Reg>
where
    Reg: [const] Register<Type = u64>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
    PC: [const] ProgramCounter<Reg::Type, Memory>,
    InstructionHandler: [const] SystemInstructionHandler<Reg, Regs, Memory, PC>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
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
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Add { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sub { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.wrapping_sub(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::Sll { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x3f;
                let value = rs1_value << shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::Slt { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.cast_signed() < rs2_value.cast_signed();
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }
            Self::Sltu { rd, rs1: _, rs2: _ } => {
                let value = rs1_value < rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }
            Self::Xor { rd, rs1: _, rs2: _ } => {
                let value = rs1_value ^ rs2_value;
                ExecutionResult::Continue { rd, value }
            }
            Self::Srl { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x3f;
                let value = rs1_value >> shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::Sra { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x3f;
                let value = rs1_value.cast_signed() >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Or { rd, rs1: _, rs2: _ } => {
                let value = rs1_value | rs2_value;
                ExecutionResult::Continue { rd, value }
            }
            Self::And { rd, rs1: _, rs2: _ } => {
                let value = rs1_value & rs2_value;
                ExecutionResult::Continue { rd, value }
            }

            Self::Addw { rd, rs1: _, rs2: _ } => {
                let sum = (rs1_value as i32).wrapping_add(rs2_value as i32);
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(sum).cast_unsigned(),
                }
            }
            Self::Subw { rd, rs1: _, rs2: _ } => {
                let diff = (rs1_value as i32).wrapping_sub(rs2_value as i32);
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(diff).cast_unsigned(),
                }
            }
            Self::Sllw { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let shifted = (rs1_value as u32) << shamt;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(shifted.cast_signed()).cast_unsigned(),
                }
            }
            Self::Srlw { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let shifted = (rs1_value as u32) >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(shifted.cast_signed()).cast_unsigned(),
                }
            }
            Self::Sraw { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let shifted = (rs1_value as i32) >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(shifted).cast_unsigned(),
                }
            }

            Self::Addi { rd, rs1: _, imm } => {
                let value = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                ExecutionResult::Continue { rd, value }
            }
            Self::Slti { rd, rs1: _, imm } => {
                let value = rs1_value.cast_signed() < i64::from(imm);
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }
            Self::Sltiu { rd, rs1: _, imm } => {
                let value = rs1_value < i64::from(imm).cast_unsigned();
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }
            Self::Xori { rd, rs1: _, imm } => {
                let value = rs1_value ^ i64::from(imm).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::Ori { rd, rs1: _, imm } => {
                let value = rs1_value | i64::from(imm).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::Andi { rd, rs1: _, imm } => {
                let value = rs1_value & i64::from(imm).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::Slli { rd, rs1: _, shamt } => {
                let value = rs1_value << shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::Srli { rd, rs1: _, shamt } => {
                let value = rs1_value >> shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::Srai { rd, rs1: _, shamt } => {
                let value = rs1_value.cast_signed() >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }

            Self::Addiw { rd, rs1: _, imm } => {
                let sum = (rs1_value as i32).wrapping_add(i32::from(imm));
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(sum).cast_unsigned(),
                }
            }
            Self::Slliw { rd, rs1: _, shamt } => {
                let shifted = (rs1_value as u32) << shamt;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(shifted.cast_signed()).cast_unsigned(),
                }
            }
            Self::Srliw { rd, rs1: _, shamt } => {
                let shifted = (rs1_value as u32) >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(shifted.cast_signed()).cast_unsigned(),
                }
            }
            Self::Sraiw { rd, rs1: _, shamt } => {
                let shifted = (rs1_value as i32) >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(shifted).cast_unsigned(),
                }
            }

            Self::Lb { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(memory.read::<i8>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Lh { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(memory.read::<i16>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Lw { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = i64::from(memory.read::<i32>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Ld { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u64>(addr)?;
                ExecutionResult::Continue { rd, value }
            }
            Self::Lbu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u8>(addr)?;
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }
            Self::Lhu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u16>(addr)?;
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }
            Self::Lwu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                let value = memory.read::<u32>(addr)?;
                ExecutionResult::Continue {
                    rd,
                    value: u64::from(value),
                }
            }

            Self::Jalr { rd, rs1: _, imm } => {
                let target = (rs1_value.wrapping_add(i64::from(imm).cast_unsigned())) & !1u64;
                regs.write(rd, program_counter.get_pc());

                ExecutionResult::Jump { target }
            }

            Self::Sb {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, rs2_value as u8)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::Sh {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, rs2_value as u16)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::Sw {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, rs2_value as u32)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::Sd {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i64::from(imm).cast_unsigned());
                memory.write(addr, rs2_value)?;
                ExecutionResult::ContinueNoWrite
            }

            Self::Beq {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value == rs2_value {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }
            Self::Bne {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value != rs2_value {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }
            Self::Blt {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value.cast_signed() < rs2_value.cast_signed() {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }
            Self::Bge {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value.cast_signed() >= rs2_value.cast_signed() {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }
            Self::Bltu {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value < rs2_value {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }
            Self::Bgeu {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value >= rs2_value {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }

            Self::Lui { rd, imm } => ExecutionResult::Continue {
                rd,
                value: i64::from(imm).cast_unsigned(),
            },

            Self::Auipc { rd, imm } => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                ExecutionResult::Continue {
                    rd,
                    value: old_pc.wrapping_add(i64::from(imm).cast_unsigned()),
                }
            }

            Self::Jal { rd, imm } => {
                let pc = program_counter.get_pc();
                regs.write(rd, pc);

                ExecutionResult::Branch {
                    offset: i32::from(imm),
                }
            }

            Self::Fence { pred, succ } => {
                system_instruction_handler.handle_fence(pred, succ);
                ExecutionResult::ContinueNoWrite
            }
            Self::FenceTso => {
                system_instruction_handler.handle_fence_tso();
                ExecutionResult::ContinueNoWrite
            }

            Self::Ecall => {
                match system_instruction_handler.handle_ecall(regs, memory, program_counter)? {
                    ControlFlow::Continue(()) => ExecutionResult::ContinueNoWrite,
                    ControlFlow::Break(()) => ExecutionResult::Break,
                }
            }
            Self::Ebreak => {
                system_instruction_handler.handle_ebreak(regs, memory, program_counter.get_pc());
                ExecutionResult::ContinueNoWrite
            }

            Self::Unimp => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                ExecutionResult::Err(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(old_pc),
                })
            }
        }
    }
}
