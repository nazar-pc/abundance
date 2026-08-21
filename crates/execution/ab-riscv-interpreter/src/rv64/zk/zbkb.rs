//! RV64 Zbkb extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZbkbInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZbkbInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZbkbInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Pack { rd, rs1: _, rs2: _ } => {
                // Pack lower 32 bits of rs1 into lower 32 bits of rd,
                // lower 32 bits of rs2 into upper 32 bits of rd.
                let lo = rs1_value & 0x0000_0000_FFFF_FFFFu64;
                let hi = (rs2_value & 0x0000_0000_FFFF_FFFFu64) << 32;
                ExecutionResult::Continue { rd, value: lo | hi }
            }
            Self::Packh { rd, rs1: _, rs2: _ } => {
                // Pack low byte of rs1 into bits [7:0], low byte of rs2 into bits [15:8].
                // Upper bits of rd are zeroed.
                let lo = rs1_value & 0xFF;
                let hi = (rs2_value & 0xFF) << 8;
                ExecutionResult::Continue { rd, value: lo | hi }
            }
            Self::Packw { rd, rs1: _, rs2: _ } => {
                // Pack low 16 bits of rs1 into bits [15:0] of the 32-bit result,
                // low 16 bits of rs2 into bits [31:16], then sign-extend to 64 bits.
                let lo = rs1_value & 0xFFFF;
                let hi = (rs2_value & 0xFFFF) << 16;
                let word = (lo | hi) as u32;
                let value = i64::from(word.cast_signed()).cast_unsigned();
                ExecutionResult::Continue { rd, value }
            }
            Self::Brev8 { rd, rs1: _ } => {
                // Reverse bits within each byte of rs1
                let src = rs1_value;
                let mut bytes = src.to_le_bytes();
                for byte in &mut bytes {
                    *byte = byte.reverse_bits();
                }
                ExecutionResult::Continue {
                    rd,
                    value: u64::from_le_bytes(bytes),
                }
            }
        }
    }
}
