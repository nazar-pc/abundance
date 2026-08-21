//! Base RISC-V RV32 instruction set

pub mod a;
pub mod b;
pub mod c;
pub mod m;
#[cfg(test)]
pub(crate) mod test_utils;
#[cfg(test)]
mod tests;
pub mod zabha;
pub mod zacas;
pub mod zalasr;
pub mod zce;
pub mod zk;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, PackedAddress, ProgramCounter,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler,
    ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32Instruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32Instruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for Rv32Instruction<Reg>
where
    Reg: [const] Register<Type = u32>,
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
                let shamt = rs2_value & 0x1f;
                let value = rs1_value << shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::Slt { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.cast_signed() < rs2_value.cast_signed();
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::Sltu { rd, rs1: _, rs2: _ } => {
                let value = rs1_value < rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::Xor { rd, rs1: _, rs2: _ } => {
                let value = rs1_value ^ rs2_value;
                ExecutionResult::Continue { rd, value }
            }
            Self::Srl { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let value = rs1_value >> shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::Sra { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
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

            Self::Addi { rd, rs1: _, imm } => {
                let value = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                ExecutionResult::Continue { rd, value }
            }
            Self::Slti { rd, rs1: _, imm } => {
                let value = rs1_value.cast_signed() < i32::from(imm);
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::Sltiu { rd, rs1: _, imm } => {
                let value = rs1_value < i32::from(imm).cast_unsigned();
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::Xori { rd, rs1: _, imm } => {
                let value = rs1_value ^ i32::from(imm).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::Ori { rd, rs1: _, imm } => {
                let value = rs1_value | i32::from(imm).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::Andi { rd, rs1: _, imm } => {
                let value = rs1_value & i32::from(imm).cast_unsigned();
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

            Self::Lb { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = i32::from(memory.read::<i8>(u64::from(addr))?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Lh { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = i32::from(memory.read::<i16>(u64::from(addr))?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::Lw { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u32>(u64::from(addr))?;
                ExecutionResult::Continue { rd, value }
            }
            Self::Lbu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u8>(u64::from(addr))?;
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::Lhu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u16>(u64::from(addr))?;
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }

            Self::Jalr { rd, rs1: _, imm } => {
                let target = (rs1_value.wrapping_add(i32::from(imm).cast_unsigned())) & !1u32;
                regs.write(rd, program_counter.get_pc());

                ExecutionResult::Jump { target }
            }

            Self::Sb {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), rs2_value as u8)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::Sh {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), rs2_value as u16)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::Sw {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), rs2_value)?;
                ExecutionResult::ContinueNoWrite
            }

            Self::Beq {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value == rs2_value {
                    return ExecutionResult::Branch {
                        offset: imm.to_i32(),
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
                        offset: imm.to_i32(),
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
                        offset: imm.to_i32(),
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
                        offset: imm.to_i32(),
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
                        offset: imm.to_i32(),
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
                        offset: imm.to_i32(),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }

            Self::Lui { rd, imm } => ExecutionResult::Continue {
                rd,
                value: imm.to_i32().cast_unsigned(),
            },

            Self::Auipc { rd, imm } => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                ExecutionResult::Continue {
                    rd,
                    value: old_pc.wrapping_add(imm.to_i32().cast_unsigned()),
                }
            }

            Self::Jal { rd, imm } => {
                let pc = program_counter.get_pc();
                regs.write(rd, pc);

                ExecutionResult::Branch {
                    offset: imm.to_i32(),
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
