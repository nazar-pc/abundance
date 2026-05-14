//! RV32 Zbb extension

pub mod rv32_zbb_helpers;
#[cfg(test)]
mod tests;

use crate::{ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2Operands};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZbbInstruction<Reg>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    fn execute(
        self,
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
            Self::Andn { rd, rs1, rs2 } => {
                let value = regs.read(rs1) & !regs.read(rs2);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Orn { rd, rs1, rs2 } => {
                let value = regs.read(rs1) | !regs.read(rs2);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Xnor { rd, rs1, rs2 } => {
                let value = !(regs.read(rs1) ^ regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Clz { rd, rs1 } => {
                let value = regs.read(rs1).leading_zeros();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Ctz { rd, rs1 } => {
                let value = regs.read(rs1).trailing_zeros();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Cpop { rd, rs1 } => {
                let value = regs.read(rs1).count_ones();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Max { rd, rs1, rs2 } => {
                let a = regs.read(rs1).cast_signed();
                let b = regs.read(rs2).cast_signed();
                let value = a.max(b).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Maxu { rd, rs1, rs2 } => {
                let value = regs.read(rs1).max(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Min { rd, rs1, rs2 } => {
                let a = regs.read(rs1).cast_signed();
                let b = regs.read(rs2).cast_signed();
                let value = a.min(b).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Minu { rd, rs1, rs2 } => {
                let value = regs.read(rs1).min(regs.read(rs2));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sextb { rd, rs1 } => {
                let value = i32::from(regs.read(rs1) as i8).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sexth { rd, rs1 } => {
                let value = i32::from(regs.read(rs1) as i16).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Zexth { rd, rs1 } => {
                let value = u32::from(regs.read(rs1) as u16);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Rol { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let value = regs.read(rs1).rotate_left(shamt);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Ror { rd, rs1, rs2 } => {
                let shamt = regs.read(rs2) & 0x1f;
                let value = regs.read(rs1).rotate_right(shamt);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Rori { rd, rs1, shamt } => {
                let value = regs.read(rs1).rotate_right(u32::from(shamt & 0x1f));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Orcb { rd, rs1 } => {
                let src = regs.read(rs1);

                Ok(ControlFlow::Continue((rd, rv32_zbb_helpers::orc_b(src))))
            }
            Self::Rev8 { rd, rs1 } => {
                let value = regs.read(rs1).swap_bytes();
                Ok(ControlFlow::Continue((rd, value)))
            }
        }
    }
}
