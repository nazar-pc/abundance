//! Base RISC-V RV32 instruction set

pub mod a;
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
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutableInstructionResult, ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, SystemInstructionHandler, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::marker::Destruct;
use core::ops::ControlFlow;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32Instruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv32Instruction<Reg>
where
    Reg: Register<Type = u32>,
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32Instruction<Reg>
where
    Reg: [const] Register<Type = u32>,
    Regs: [const] RegisterFile<Reg>,
    Memory: [const] VirtualMemory,
    PC: [const] ProgramCounter<Reg::Type, Memory, CustomError>,
    InstructionHandler: [const] SystemInstructionHandler<Reg, Regs, Memory, PC, CustomError>,
    CustomError: [const] Destruct,
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
    ) -> ExecutableInstructionResult<(), Self, CustomError> {
        match self {
            Self::Add { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.wrapping_add(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sub { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.wrapping_sub(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sll { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let value = rs1_value << shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Slt { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.cast_signed() < rs2_value.cast_signed();
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::Sltu { rd, rs1: _, rs2: _ } => {
                let value = rs1_value < rs2_value;
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::Xor { rd, rs1: _, rs2: _ } => {
                let value = rs1_value ^ rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Srl { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let value = rs1_value >> shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sra { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let value = rs1_value.cast_signed() >> shamt;
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Or { rd, rs1: _, rs2: _ } => {
                let value = rs1_value | rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::And { rd, rs1: _, rs2: _ } => {
                let value = rs1_value & rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }

            Self::Addi { rd, rs1: _, imm } => {
                let value = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Slti { rd, rs1: _, imm } => {
                let value = rs1_value.cast_signed() < i32::from(imm);
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::Sltiu { rd, rs1: _, imm } => {
                let value = rs1_value < i32::from(imm).cast_unsigned();
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::Xori { rd, rs1: _, imm } => {
                let value = rs1_value ^ i32::from(imm).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Ori { rd, rs1: _, imm } => {
                let value = rs1_value | i32::from(imm).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Andi { rd, rs1: _, imm } => {
                let value = rs1_value & i32::from(imm).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Slli { rd, rs1: _, shamt } => {
                let value = rs1_value << shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Srli { rd, rs1: _, shamt } => {
                let value = rs1_value >> shamt;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Srai { rd, rs1: _, shamt } => {
                let value = rs1_value.cast_signed() >> shamt;
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }

            Self::Lb { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = i32::from(memory.read::<i8>(u64::from(addr))?);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Lh { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = i32::from(memory.read::<i16>(u64::from(addr))?);
                Ok(ControlFlow::Continue((rd, value.cast_unsigned())))
            }
            Self::Lw { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u32>(u64::from(addr))?;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Lbu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u8>(u64::from(addr))?;
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }
            Self::Lhu { rd, rs1: _, imm } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                let value = memory.read::<u16>(u64::from(addr))?;
                Ok(ControlFlow::Continue((rd, u32::from(value))))
            }

            Self::Jalr { rd, rs1: _, imm } => {
                let target = (rs1_value.wrapping_add(i32::from(imm).cast_unsigned())) & !1u32;
                regs.write(rd, program_counter.get_pc());

                match program_counter.set_pc(memory, target) {
                    Ok(control_flow) => Ok(match control_flow {
                        ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                        ControlFlow::Break(()) => ControlFlow::Break(()),
                    }),
                    Err(err) => Err(ExecutionError::from(err)),
                }
            }

            Self::Sb {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), rs2_value as u8)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Sh {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), rs2_value as u16)?;
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Sw {
                rs2: _,
                rs1: _,
                imm,
            } => {
                let addr = rs1_value.wrapping_add(i32::from(imm).cast_unsigned());
                memory.write(u64::from(addr), rs2_value)?;
                Ok(ControlFlow::Continue(Default::default()))
            }

            Self::Beq {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value == rs2_value {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return match program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                    {
                        Ok(control_flow) => Ok(match control_flow {
                            ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                            ControlFlow::Break(()) => ControlFlow::Break(()),
                        }),
                        Err(err) => Err(ExecutionError::from(err)),
                    };
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Bne {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value != rs2_value {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return match program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                    {
                        Ok(control_flow) => Ok(match control_flow {
                            ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                            ControlFlow::Break(()) => ControlFlow::Break(()),
                        }),
                        Err(err) => Err(ExecutionError::from(err)),
                    };
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Blt {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value.cast_signed() < rs2_value.cast_signed() {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return match program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                    {
                        Ok(control_flow) => Ok(match control_flow {
                            ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                            ControlFlow::Break(()) => ControlFlow::Break(()),
                        }),
                        Err(err) => Err(ExecutionError::from(err)),
                    };
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Bge {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value.cast_signed() >= rs2_value.cast_signed() {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return match program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                    {
                        Ok(control_flow) => Ok(match control_flow {
                            ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                            ControlFlow::Break(()) => ControlFlow::Break(()),
                        }),
                        Err(err) => Err(ExecutionError::from(err)),
                    };
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Bltu {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value < rs2_value {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return match program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                    {
                        Ok(control_flow) => Ok(match control_flow {
                            ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                            ControlFlow::Break(()) => ControlFlow::Break(()),
                        }),
                        Err(err) => Err(ExecutionError::from(err)),
                    };
                }

                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::Bgeu {
                rs1: _,
                rs2: _,
                imm,
            } => {
                if rs1_value >= rs2_value {
                    let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                    return match program_counter
                        .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                    {
                        Ok(control_flow) => Ok(match control_flow {
                            ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                            ControlFlow::Break(()) => ControlFlow::Break(()),
                        }),
                        Err(err) => Err(ExecutionError::from(err)),
                    };
                }

                Ok(ControlFlow::Continue(Default::default()))
            }

            Self::Lui { rd, imm } => Ok(ControlFlow::Continue((rd, imm.to_i32().cast_unsigned()))),

            Self::Auipc { rd, imm } => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                Ok(ControlFlow::Continue((
                    rd,
                    old_pc.wrapping_add(imm.to_i32().cast_unsigned()),
                )))
            }

            Self::Jal { rd, imm } => {
                let pc = program_counter.get_pc();
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                regs.write(rd, pc);

                match program_counter
                    .set_pc(memory, old_pc.wrapping_add(imm.to_i32().cast_unsigned()))
                {
                    Ok(control_flow) => Ok(match control_flow {
                        ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                        ControlFlow::Break(()) => ControlFlow::Break(()),
                    }),
                    Err(err) => Err(ExecutionError::ProgramCounter(err)),
                }
            }

            Self::Fence { pred, succ } => {
                system_instruction_handler.handle_fence(pred, succ);
                Ok(ControlFlow::Continue(Default::default()))
            }
            Self::FenceTso => {
                system_instruction_handler.handle_fence_tso();
                Ok(ControlFlow::Continue(Default::default()))
            }

            Self::Ecall => {
                match system_instruction_handler.handle_ecall(regs, memory, program_counter) {
                    Ok(control_flow) => Ok(match control_flow {
                        ControlFlow::Continue(()) => ControlFlow::Continue(Default::default()),
                        ControlFlow::Break(()) => ControlFlow::Break(()),
                    }),
                    Err(err) => Err(err),
                }
            }
            Self::Ebreak => {
                system_instruction_handler.handle_ebreak(regs, memory, program_counter.get_pc());
                Ok(ControlFlow::Continue(Default::default()))
            }

            Self::Unimp => {
                let old_pc = program_counter.old_pc(size_of::<u32>() as u8);
                Err(ExecutionError::IllegalInstruction { address: old_pc })
            }
        }
    }
}
