//! RV64 Zbb extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_primitives::instruction::rv64::b::zbb::Rv64ZbbInstruction;
use ab_riscv_primitives::registers::{Register, Registers};
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
        execute_zbb(&mut state.regs, self);

        Ok(ControlFlow::Continue(()))
    }
}

/// Execute instructions from Zbb extension
#[inline(always)]
pub fn execute_zbb<Reg>(regs: &mut Registers<Reg>, instruction: Rv64ZbbInstruction<Reg>)
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    match instruction {
        Rv64ZbbInstruction::Andn { rd, rs1, rs2 } => {
            let value = regs.read(rs1) & !regs.read(rs2);
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Orn { rd, rs1, rs2 } => {
            let value = regs.read(rs1) | !regs.read(rs2);
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Xnor { rd, rs1, rs2 } => {
            let value = !(regs.read(rs1) ^ regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Clz { rd, rs1 } => {
            let value = regs.read(rs1).leading_zeros() as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Clzw { rd, rs1 } => {
            let value = (regs.read(rs1) as u32).leading_zeros() as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Ctz { rd, rs1 } => {
            let value = regs.read(rs1).trailing_zeros() as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Ctzw { rd, rs1 } => {
            let value = (regs.read(rs1) as u32).trailing_zeros() as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Cpop { rd, rs1 } => {
            let value = regs.read(rs1).count_ones() as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Cpopw { rd, rs1 } => {
            let value = (regs.read(rs1) as u32).count_ones() as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Max { rd, rs1, rs2 } => {
            let a = regs.read(rs1).cast_signed();
            let b = regs.read(rs2).cast_signed();
            let value = a.max(b).cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Maxu { rd, rs1, rs2 } => {
            let value = regs.read(rs1).max(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Min { rd, rs1, rs2 } => {
            let a = regs.read(rs1).cast_signed();
            let b = regs.read(rs2).cast_signed();
            let value = a.min(b).cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Minu { rd, rs1, rs2 } => {
            let value = regs.read(rs1).min(regs.read(rs2));
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Sextb { rd, rs1 } => {
            let value = ((regs.read(rs1) as i8) as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Sexth { rd, rs1 } => {
            let value = ((regs.read(rs1) as i16) as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Zexth { rd, rs1 } => {
            let value = (regs.read(rs1) as u16) as u64;
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Rol { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x3f) as u32;
            let value = regs.read(rs1).rotate_left(shamt);
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Rolw { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x1f) as u32;
            let value =
                ((regs.read(rs1) as u32).rotate_left(shamt).cast_signed() as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Ror { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x3f) as u32;
            let value = regs.read(rs1).rotate_right(shamt);
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Rori { rd, rs1, shamt } => {
            let value = regs.read(rs1).rotate_right((shamt & 0x3f) as u32);
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Roriw { rd, rs1, shamt } => {
            let value = ((regs.read(rs1) as u32)
                .rotate_right((shamt & 0x1f) as u32)
                .cast_signed() as i64)
                .cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Rorw { rd, rs1, rs2 } => {
            let shamt = (regs.read(rs2) & 0x1f) as u32;
            let value =
                ((regs.read(rs1) as u32).rotate_right(shamt).cast_signed() as i64).cast_unsigned();
            regs.write(rd, value);
        }
        Rv64ZbbInstruction::Orcb { rd, rs1 } => {
            let src = regs.read(rs1);
            let mut result = 0u64;
            for i in 0..8 {
                let byte = (src >> (i * 8)) & 0xFF;
                if byte != 0 {
                    result |= 0xFFu64 << (i * 8);
                }
            }
            regs.write(rd, result);
        }
        Rv64ZbbInstruction::Rev8 { rd, rs1 } => {
            let value = regs.read(rs1).swap_bytes();
            regs.write(rd, value);
        }
    }
}
