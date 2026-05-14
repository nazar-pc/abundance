//! RV64 Zbkb extension

#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZbkbInstruction<Reg>
where
    Reg: Register<Type = u64>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
        _rs1rs2_values: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::Pack { rd, rs1, rs2 } => {
                // Pack lower 32 bits of rs1 into lower 32 bits of rd,
                // lower 32 bits of rs2 into upper 32 bits of rd.
                let lo = regs.read(rs1) & 0x0000_0000_FFFF_FFFFu64;
                let hi = (regs.read(rs2) & 0x0000_0000_FFFF_FFFFu64) << 32;
                Ok(ControlFlow::Continue((rd, lo | hi)))
            }
            Self::Packh { rd, rs1, rs2 } => {
                // Pack low byte of rs1 into bits [7:0], low byte of rs2 into bits [15:8].
                // Upper bits of rd are zeroed.
                let lo = regs.read(rs1) & 0xFF;
                let hi = (regs.read(rs2) & 0xFF) << 8;
                Ok(ControlFlow::Continue((rd, lo | hi)))
            }
            Self::Packw { rd, rs1, rs2 } => {
                // Pack low 16 bits of rs1 into bits [15:0] of the 32-bit result,
                // low 16 bits of rs2 into bits [31:16], then sign-extend to 64 bits.
                let lo = regs.read(rs1) & 0xFFFF;
                let hi = (regs.read(rs2) & 0xFFFF) << 16;
                let word = (lo | hi) as u32;
                let value = i64::from(word.cast_signed()).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Brev8 { rd, rs1 } => {
                // Reverse bits within each byte of rs1
                let src = regs.read(rs1);
                let mut bytes = src.to_le_bytes();
                for byte in &mut bytes {
                    *byte = byte.reverse_bits();
                }
                Ok(ControlFlow::Continue((rd, u64::from_le_bytes(bytes))))
            }
        }
    }
}
