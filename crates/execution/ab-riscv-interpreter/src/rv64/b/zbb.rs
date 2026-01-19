//! RV64 Zbb extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instruction::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::registers::Register;
use core::ops::ControlFlow;

impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbbInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Andn { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) & !state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::Orn { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1) | !state.regs.read(rs2);
                state.regs.write(rd, value);
            }
            Self::Xnor { rd, rs1, rs2 } => {
                let value = !(state.regs.read(rs1) ^ state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Clz { rd, rs1 } => {
                let value = state.regs.read(rs1).leading_zeros() as u64;
                state.regs.write(rd, value);
            }
            Self::Clzw { rd, rs1 } => {
                let value = (state.regs.read(rs1) as u32).leading_zeros() as u64;
                state.regs.write(rd, value);
            }
            Self::Ctz { rd, rs1 } => {
                let value = state.regs.read(rs1).trailing_zeros() as u64;
                state.regs.write(rd, value);
            }
            Self::Ctzw { rd, rs1 } => {
                let value = (state.regs.read(rs1) as u32).trailing_zeros() as u64;
                state.regs.write(rd, value);
            }
            Self::Cpop { rd, rs1 } => {
                let value = state.regs.read(rs1).count_ones() as u64;
                state.regs.write(rd, value);
            }
            Self::Cpopw { rd, rs1 } => {
                let value = (state.regs.read(rs1) as u32).count_ones() as u64;
                state.regs.write(rd, value);
            }
            Self::Max { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1).cast_signed();
                let b = state.regs.read(rs2).cast_signed();
                let value = a.max(b).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Maxu { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).max(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Min { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1).cast_signed();
                let b = state.regs.read(rs2).cast_signed();
                let value = a.min(b).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Minu { rd, rs1, rs2 } => {
                let value = state.regs.read(rs1).min(state.regs.read(rs2));
                state.regs.write(rd, value);
            }
            Self::Sextb { rd, rs1 } => {
                let value = ((state.regs.read(rs1) as i8) as i64).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Sexth { rd, rs1 } => {
                let value = ((state.regs.read(rs1) as i16) as i64).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Zexth { rd, rs1 } => {
                let value = (state.regs.read(rs1) as u16) as u64;
                state.regs.write(rd, value);
            }
            Self::Rol { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x3f) as u32;
                let value = state.regs.read(rs1).rotate_left(shamt);
                state.regs.write(rd, value);
            }
            Self::Rolw { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x1f) as u32;
                let value = ((state.regs.read(rs1) as u32)
                    .rotate_left(shamt)
                    .cast_signed() as i64)
                    .cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Ror { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x3f) as u32;
                let value = state.regs.read(rs1).rotate_right(shamt);
                state.regs.write(rd, value);
            }
            Self::Rori { rd, rs1, shamt } => {
                let value = state.regs.read(rs1).rotate_right((shamt & 0x3f) as u32);
                state.regs.write(rd, value);
            }
            Self::Roriw { rd, rs1, shamt } => {
                let value = ((state.regs.read(rs1) as u32)
                    .rotate_right((shamt & 0x1f) as u32)
                    .cast_signed() as i64)
                    .cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Rorw { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x1f) as u32;
                let value = ((state.regs.read(rs1) as u32)
                    .rotate_right(shamt)
                    .cast_signed() as i64)
                    .cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Orcb { rd, rs1 } => {
                let src = state.regs.read(rs1);
                let mut result = 0u64;
                for i in 0..8 {
                    let byte = (src >> (i * 8)) & 0xFF;
                    if byte != 0 {
                        result |= 0xFFu64 << (i * 8);
                    }
                }
                state.regs.write(rd, result);
            }
            Self::Rev8 { rd, rs1 } => {
                let value = state.regs.read(rs1).swap_bytes();
                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
