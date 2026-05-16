//! RV32 Zbb extension

pub mod rv32_zbb_helpers;
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
    for Rv32ZbbInstruction<Reg>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
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
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::Andn { rd, rs1: _, rs2: _ } => {
                let value = rs1_value & !rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Orn { rd, rs1: _, rs2: _ } => {
                let value = rs1_value | !rs2_value;
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Xnor { rd, rs1: _, rs2: _ } => {
                let value = !(rs1_value ^ rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Clz { rd, rs1: _ } => {
                let value = rs1_value.leading_zeros();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Ctz { rd, rs1: _ } => {
                let value = rs1_value.trailing_zeros();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Cpop { rd, rs1: _ } => {
                let value = rs1_value.count_ones();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Max { rd, rs1: _, rs2: _ } => {
                let a = rs1_value.cast_signed();
                let b = rs2_value.cast_signed();
                let value = a.max(b).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Maxu { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.max(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Min { rd, rs1: _, rs2: _ } => {
                let a = rs1_value.cast_signed();
                let b = rs2_value.cast_signed();
                let value = a.min(b).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Minu { rd, rs1: _, rs2: _ } => {
                let value = rs1_value.min(rs2_value);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sextb { rd, rs1: _ } => {
                let value = i32::from(rs1_value as i8).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Sexth { rd, rs1: _ } => {
                let value = i32::from(rs1_value as i16).cast_unsigned();
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Zexth { rd, rs1: _ } => {
                let value = u32::from(rs1_value as u16);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Rol { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let value = rs1_value.rotate_left(shamt);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Ror { rd, rs1: _, rs2: _ } => {
                let shamt = rs2_value & 0x1f;
                let value = rs1_value.rotate_right(shamt);
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Rori { rd, rs1: _, shamt } => {
                let value = rs1_value.rotate_right(u32::from(shamt & 0x1f));
                Ok(ControlFlow::Continue((rd, value)))
            }
            Self::Orcb { rd, rs1: _ } => {
                let src = rs1_value;

                Ok(ControlFlow::Continue((rd, rv32_zbb_helpers::orc_b(src))))
            }
            Self::Rev8 { rd, rs1: _ } => {
                let value = rs1_value.swap_bytes();
                Ok(ControlFlow::Continue((rd, value)))
            }
        }
    }
}
