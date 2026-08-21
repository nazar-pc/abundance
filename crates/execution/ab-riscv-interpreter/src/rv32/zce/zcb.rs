//! Zcb compressed instruction execution (RV32)
//!
//! C.ZEXT.W is absent in RV32 - the enum has no such variant.

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    PackedAddress, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    SystemInstructionHandler, ThreadedExecutableInstruction, ThreadedExecutionResult,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZcbInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32ZcbInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv32ZcbInstruction<Reg>
where
    Reg: Register<Type = u32>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
        ExecutionResult::ContinueNoWrite
    }
}

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZcbOnlyInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32ZcbOnlyInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv32ZcbOnlyInstruction<Reg>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory>,
    InstructionHandler: SystemInstructionHandler<Reg, Regs, Memory, PC>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::CLbu { rd, rs1: _, uimm } => {
                let addr = u64::from(rs1_value.wrapping_add(u32::from(uimm)));
                let value = memory.read::<u8>(addr)?;
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::CLh { rd, rs1: _, uimm } => {
                let addr = u64::from(rs1_value.wrapping_add(u32::from(uimm)));
                let value = i32::from(memory.read::<i16>(addr)?);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::CLhu { rd, rs1: _, uimm } => {
                let addr = u64::from(rs1_value.wrapping_add(u32::from(uimm)));
                let value = memory.read::<u16>(addr)?;
                ExecutionResult::Continue {
                    rd,
                    value: u32::from(value),
                }
            }
            Self::CSb {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = u64::from(rs1_value.wrapping_add(u32::from(uimm)));
                memory.write(addr, rs2_value as u8)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::CSh {
                rs1: _,
                rs2: _,
                uimm,
            } => {
                let addr = u64::from(rs1_value.wrapping_add(u32::from(uimm)));
                memory.write(addr, rs2_value as u16)?;
                ExecutionResult::ContinueNoWrite
            }
            Self::CZextB { rd } => {
                let value = regs.read(rd) & 0xff;
                ExecutionResult::Continue { rd, value }
            }
            Self::CSextB { rd } => {
                let value = i32::from(regs.read(rd) as i8);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::CZextH { rd } => {
                let value = regs.read(rd) & 0xffff;
                ExecutionResult::Continue { rd, value }
            }
            Self::CSextH { rd } => {
                let value = i32::from(regs.read(rd) as i16);
                ExecutionResult::Continue {
                    rd,
                    value: value.cast_unsigned(),
                }
            }
            Self::CNot { rd } => {
                let value = !regs.read(rd);
                ExecutionResult::Continue { rd, value }
            }
            Self::CMul { rd, rs2: _ } => {
                let value = regs.read(rd).wrapping_mul(rs2_value);
                ExecutionResult::Continue { rd, value }
            }
        }
    }
}
