//! RV64 Zca extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, PackedAddress, ProgramCounter,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, SystemInstructionHandler,
    ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZcaInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZcaInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZcaInstruction<Reg>
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
            // Quadrant 00
            Self::CAddi4spn { rd, nzuimm } => {
                let sp_val = regs.read(Reg::SP);
                ExecutionResult::Continue {
                    rd,
                    value: sp_val.wrapping_add(u64::from(nzuimm)),
                }
            }
            Self::CLw { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = i64::from(memory.read::<i32>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::CLd { rd, rs1: _, uimm } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                let value = memory.read::<u64>(addr)?;
                ExecutionResult::Continue { rd, value }
            }
            Self::CSw {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value as u32)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::CSd {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = rs1_value.wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value)?;
                ExecutionResult::ContinueNoWrite
            }

            // Quadrant 01
            Self::CNop => {}
            Self::CAddi { rd, nzimm } => {
                let value = regs.read(rd).wrapping_add(i64::from(nzimm).cast_unsigned());
                ExecutionResult::Continue { rd, value }
            }
            Self::CAddiw { rd, imm } => {
                let sum = (regs.read(rd) as i32).wrapping_add(i32::from(imm));
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(sum).cast_unsigned(),
                }
            }
            Self::CLi { rd, imm } => ExecutionResult::Continue {
                rd,
                value: i64::from(imm).cast_unsigned(),
            },
            Self::CAddi16sp { nzimm } => {
                let value = regs
                    .read(Reg::SP)
                    .wrapping_add(i64::from(nzimm).cast_unsigned());
                ExecutionResult::Continue { rd: Reg::SP, value }
            }
            Self::CLui { rd, nzimm } => ExecutionResult::Continue {
                rd,
                value: i64::from(nzimm).cast_unsigned(),
            },
            Self::CSrli { rd, shamt } => {
                let value = regs.read(rd) >> shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::CSrai { rd, shamt } => {
                let value = regs.read(rd).cast_signed() >> shamt;
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::CAndi { rd, imm } => {
                let value = regs.read(rd) & i64::from(imm).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::CSub { rd, rs2: _ } => {
                let value = regs.read(rd).wrapping_sub(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::CXor { rd, rs2: _ } => {
                let value = regs.read(rd) ^ rs2_value;
                ExecutionResult::Continue { rd, value }
            }
            Self::COr { rd, rs2: _ } => {
                let value = regs.read(rd) | rs2_value;
                ExecutionResult::Continue { rd, value }
            }
            Self::CAnd { rd, rs2: _ } => {
                let value = regs.read(rd) & rs2_value;
                ExecutionResult::Continue { rd, value }
            }
            Self::CSubw { rd, rs2: _ } => {
                let diff = (regs.read(rd) as i32).wrapping_sub(rs2_value as i32);
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(diff).cast_unsigned(),
                }
            }
            Self::CAddw { rd, rs2: _ } => {
                let sum = (regs.read(rd) as i32).wrapping_add(rs2_value as i32);
                ExecutionResult::Continue {
                    rd,
                    value: i64::from(sum).cast_unsigned(),
                }
            }
            Self::CJ { imm } => ExecutionResult::Branch {
                offset: i32::from(imm),
            },
            Self::CBeqz { rs1: _, imm } => {
                if rs1_value == 0 {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }
            Self::CBnez { rs1: _, imm } => {
                if rs1_value != 0 {
                    return ExecutionResult::Branch {
                        offset: i32::from(imm),
                    };
                }

                ExecutionResult::ContinueNoWrite
            }

            // Quadrant 10
            Self::CSlli { rd, shamt } => {
                let value = regs.read(rd) << shamt;
                ExecutionResult::Continue { rd, value }
            }
            Self::CLwsp { rd, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                let value = i64::from(memory.read::<i32>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::CLdsp { rd, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                let value = memory.read::<u64>(addr)?;
                ExecutionResult::Continue { rd, value }
            }
            Self::CJr { rs1: _ } => {
                let target = rs1_value & !1;
                ExecutionResult::Jump { target }
            }
            Self::CMv { rd, rs2: _ } => ExecutionResult::Continue {
                rd,
                value: rs2_value,
            },
            Self::CEbreak => {
                system_instruction_handler.handle_ebreak(regs, memory, program_counter.get_pc());
                ExecutionResult::ContinueNoWrite
            }
            Self::CJalr { rs1: _ } => {
                let target = rs1_value & !1;
                let return_addr = program_counter.get_pc();
                regs.write(Reg::RA, return_addr);
                ExecutionResult::Jump { target }
            }
            Self::CAdd { rd, rs2: _ } => {
                let value = regs.read(rd).wrapping_add(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
            Self::CSwsp { rs2: _, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value as u32)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::CSdsp { rs2: _, uimm } => {
                let addr = regs.read(Reg::SP).wrapping_add(u64::from(uimm));
                memory.write(addr, rs2_value)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::CUnimp => {
                let old_pc = program_counter.old_pc(size_of::<u16>() as u8);
                return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(old_pc),
                });
            }
        }
    }
}
