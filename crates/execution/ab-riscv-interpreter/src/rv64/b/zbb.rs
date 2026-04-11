//! RV64 Zbb extension

pub mod rv64_zbb_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbbInstruction<Reg>
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
                let value = u64::from(state.regs.read(rs1).leading_zeros());
                state.regs.write(rd, value);
            }
            Self::Clzw { rd, rs1 } => {
                let value = u64::from((state.regs.read(rs1) as u32).leading_zeros());
                state.regs.write(rd, value);
            }
            Self::Ctz { rd, rs1 } => {
                let value = u64::from(state.regs.read(rs1).trailing_zeros());
                state.regs.write(rd, value);
            }
            Self::Ctzw { rd, rs1 } => {
                let value = u64::from((state.regs.read(rs1) as u32).trailing_zeros());
                state.regs.write(rd, value);
            }
            Self::Cpop { rd, rs1 } => {
                let value = u64::from(state.regs.read(rs1).count_ones());
                state.regs.write(rd, value);
            }
            Self::Cpopw { rd, rs1 } => {
                let value = u64::from((state.regs.read(rs1) as u32).count_ones());
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
                let value = i64::from(state.regs.read(rs1) as i8).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Sexth { rd, rs1 } => {
                let value = i64::from(state.regs.read(rs1) as i16).cast_unsigned();
                state.regs.write(rd, value);
            }
            Self::Zexth { rd, rs1 } => {
                let value = u64::from(state.regs.read(rs1) as u16);
                state.regs.write(rd, value);
            }
            Self::Rol { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x3f) as u32;
                let value = state.regs.read(rs1).rotate_left(shamt);
                state.regs.write(rd, value);
            }
            Self::Rolw { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x1f) as u32;
                let value = i64::from(
                    (state.regs.read(rs1) as u32)
                        .rotate_left(shamt)
                        .cast_signed(),
                );
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Ror { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x3f) as u32;
                let value = state.regs.read(rs1).rotate_right(shamt);
                state.regs.write(rd, value);
            }
            Self::Rori { rd, rs1, shamt } => {
                let value = state.regs.read(rs1).rotate_right(u32::from(shamt & 0x3f));
                state.regs.write(rd, value);
            }
            Self::Roriw { rd, rs1, shamt } => {
                let value = i64::from(
                    (state.regs.read(rs1) as u32)
                        .rotate_right((shamt & 0x1f) as u32)
                        .cast_signed(),
                );
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Rorw { rd, rs1, rs2 } => {
                let shamt = (state.regs.read(rs2) & 0x1f) as u32;
                let value = i64::from(
                    (state.regs.read(rs1) as u32)
                        .rotate_right(shamt)
                        .cast_signed(),
                );
                state.regs.write(rd, value.cast_unsigned());
            }
            Self::Orcb { rd, rs1 } => {
                let src = state.regs.read(rs1);

                state.regs.write(rd, rv64_zbb_helpers::orc_b(src));
            }
            Self::Rev8 { rd, rs1 } => {
                let value = state.regs.read(rs1).swap_bytes();
                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
