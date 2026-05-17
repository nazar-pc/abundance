//! RV32 Zbkb extension

pub mod rv32_zbkb_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Rv32ZbkbInstruction<Reg> where Reg: Register<Type = u32> {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Rv32ZbkbInstruction<Reg>
where
    Reg: Register<Type = u32>,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv32ZbkbInstruction<Reg>
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
            Self::Pack { rd, rs1: _, rs2: _ } => {
                // Pack low 16 bits of rs1 into rd[15:0],
                // low 16 bits of rs2 into rd[31:16].
                let lo = rs1_value & 0x0000_FFFFu32;
                let hi = (rs2_value & 0x0000_FFFFu32) << 16;
                Ok(ControlFlow::Continue((rd, lo | hi)))
            }
            Self::Packh { rd, rs1: _, rs2: _ } => {
                // Pack low byte of rs1 into bits [7:0], low byte of rs2 into bits [15:8].
                // Upper bits of rd are zeroed.
                let lo = rs1_value & 0xFF;
                let hi = (rs2_value & 0xFF) << 8;
                Ok(ControlFlow::Continue((rd, lo | hi)))
            }
            Self::Brev8 { rd, rs1: _ } => {
                // Reverse bits within each byte of rs1
                let src = rs1_value;
                let mut bytes = src.to_le_bytes();
                for byte in &mut bytes {
                    *byte = byte.reverse_bits();
                }
                Ok(ControlFlow::Continue((rd, u32::from_le_bytes(bytes))))
            }
            Self::Zip { rd, rs1: _ } => {
                // Bit-interleave: scatter bits of rs1 so that
                // rs1[i]    -> rd[2*i]   (even positions, lower half source)
                // rs1[i+16] -> rd[2*i+1] (odd positions, upper half source)
                // for i in 0..16.
                let src = rs1_value;

                Ok(ControlFlow::Continue((rd, rv32_zbkb_helpers::zip(src))))
            }
            Self::Unzip { rd, rs1: _ } => {
                // Inverse of zip: gather bits of rs1 so that
                // rs1[2*i]   -> rd[i]    (even-position bits -> lower half)
                // rs1[2*i+1] -> rd[i+16] (odd-position bits -> upper half)
                // for i in 0..16.
                let src = rs1_value;

                Ok(ControlFlow::Continue((rd, rv32_zbkb_helpers::unzip(src))))
            }
        }
    }
}
