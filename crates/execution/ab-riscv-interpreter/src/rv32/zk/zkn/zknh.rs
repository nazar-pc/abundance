//! RV32 Zknh extension

pub mod rv32_zknh_helpers;
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
    for Rv32ZknhInstruction<Reg>
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
            // SHA-256 (single-register)
            Self::Sha256Sig0 { rd, rs1: _ } => {
                let x = rs1_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha256sig0(x),
                )))
            }
            Self::Sha256Sig1 { rd, rs1: _ } => {
                let x = rs1_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha256sig1(x),
                )))
            }
            Self::Sha256Sum0 { rd, rs1: _ } => {
                let x = rs1_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha256sum0(x),
                )))
            }
            Self::Sha256Sum1 { rd, rs1: _ } => {
                let x = rs1_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha256sum1(x),
                )))
            }

            // SHA-512 (two-register R-type)
            //
            // Register conventions (from the RISC-V scalar crypto spec, Sail pseudocode):
            //
            //   sha512sig0l, sha512sig1l : rs1 = LOW word,  rs2 = HIGH word
            //   sha512sig0h, sha512sig1h : rs1 = HIGH word, rs2 = LOW word
            //   sha512sum0r, sha512sum1r : rs1 = LOW word,  rs2 = HIGH word
            //
            // The Sail model for sum0r/sum1r assembles the operand as:
            //   x[63:32] = X(rs2),  x[31:0] = X(rs1)
            // and writes x[31:0] of the result to rd.
            //
            // The helpers receive (rs1, rs2) exactly as read from the register file;
            // they handle the asymmetric convention internally.
            Self::Sha512Sig0h { rd, rs1: _, rs2: _ } => {
                let rs1_val = rs1_value;
                let rs2_val = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha512sig0h(rs1_val, rs2_val),
                )))
            }
            Self::Sha512Sig0l { rd, rs1: _, rs2: _ } => {
                let rs1_val = rs1_value;
                let rs2_val = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha512sig0l(rs1_val, rs2_val),
                )))
            }
            Self::Sha512Sig1h { rd, rs1: _, rs2: _ } => {
                let rs1_val = rs1_value;
                let rs2_val = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha512sig1h(rs1_val, rs2_val),
                )))
            }
            Self::Sha512Sig1l { rd, rs1: _, rs2: _ } => {
                let rs1_val = rs1_value;
                let rs2_val = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha512sig1l(rs1_val, rs2_val),
                )))
            }
            Self::Sha512Sum0r { rd, rs1: _, rs2: _ } => {
                let rs1_val = rs1_value;
                let rs2_val = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha512sum0r(rs1_val, rs2_val),
                )))
            }
            Self::Sha512Sum1r { rd, rs1: _, rs2: _ } => {
                let rs1_val = rs1_value;
                let rs2_val = rs2_value;
                Ok(ControlFlow::Continue((
                    rd,
                    rv32_zknh_helpers::sha512sum1r(rs1_val, rs2_val),
                )))
            }
        }
    }
}
