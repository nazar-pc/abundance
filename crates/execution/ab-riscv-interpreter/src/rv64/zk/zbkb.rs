//! RV64 Zbkb extension

#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::zk::zbkb::Rv64ZbkbInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbkbInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Pack { rd, rs1, rs2 } => {
                // Pack lower 32 bits of rs1 into lower 32 bits of rd,
                // lower 32 bits of rs2 into upper 32 bits of rd.
                let lo = state.regs.read(rs1) & 0x0000_0000_FFFF_FFFFu64;
                let hi = (state.regs.read(rs2) & 0x0000_0000_FFFF_FFFFu64) << 32;
                state.regs.write(rd, lo | hi);
            }
            Self::Packh { rd, rs1, rs2 } => {
                // Pack low byte of rs1 into bits [7:0], low byte of rs2 into bits [15:8].
                // Upper bits of rd are zeroed.
                let lo = state.regs.read(rs1) & 0xFF;
                let hi = (state.regs.read(rs2) & 0xFF) << 8;
                state.regs.write(rd, lo | hi);
            }
            Self::Packw { rd, rs1, rs2 } => {
                // Pack low 16 bits of rs1 into bits [15:0] of the 32-bit result,
                // low 16 bits of rs2 into bits [31:16], then sign-extend to 64 bits.
                let lo = state.regs.read(rs1) & 0xFFFF;
                let hi = (state.regs.read(rs2) & 0xFFFF) << 16;
                let word = (lo | hi) as u32;
                let value = i64::from(word.cast_signed()).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Brev8 { rd, rs1 } => {
                // Reverse bits within each byte of rs1
                let src = state.regs.read(rs1);
                let bytes = src.to_le_bytes().map(u8::reverse_bits);
                state.regs.write(rd, u64::from_le_bytes(bytes));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
