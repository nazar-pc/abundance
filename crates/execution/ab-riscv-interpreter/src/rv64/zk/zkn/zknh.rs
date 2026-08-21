//! RV64 Zknh extension

pub mod rv64_zknh_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, ThreadedExecutableInstruction, ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZknhInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZknhInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZknhInstruction<Reg>
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
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Sha256Sig0 { rd, rs1: _ } => {
                let x = rs1_value as u32;

                let res32 = rv64_zknh_helpers::sha256sig0(x);

                ExecutionResult::Continue {
                    rd,
                    value: i64::from(res32.cast_signed()).cast_unsigned(),
                }
            }
            Self::Sha256Sig1 { rd, rs1: _ } => {
                let x = rs1_value as u32;

                let res32 = rv64_zknh_helpers::sha256sig1(x);

                ExecutionResult::Continue {
                    rd,
                    value: i64::from(res32.cast_signed()).cast_unsigned(),
                }
            }
            Self::Sha256Sum0 { rd, rs1: _ } => {
                let x = rs1_value as u32;

                let res32 = rv64_zknh_helpers::sha256sum0(x);

                ExecutionResult::Continue {
                    rd,
                    value: i64::from(res32.cast_signed()).cast_unsigned(),
                }
            }
            Self::Sha256Sum1 { rd, rs1: _ } => {
                let x = rs1_value as u32;

                let res32 = rv64_zknh_helpers::sha256sum1(x);

                ExecutionResult::Continue {
                    rd,
                    value: i64::from(res32.cast_signed()).cast_unsigned(),
                }
            }
            Self::Sha512Sig0 { rd, rs1: _ } => {
                let x = rs1_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknh_helpers::sha512sig0(x),
                }
            }
            Self::Sha512Sig1 { rd, rs1: _ } => {
                let x = rs1_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknh_helpers::sha512sig1(x),
                }
            }
            Self::Sha512Sum0 { rd, rs1: _ } => {
                let x = rs1_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknh_helpers::sha512sum0(x),
                }
            }
            Self::Sha512Sum1 { rd, rs1: _ } => {
                let x = rs1_value;

                ExecutionResult::Continue {
                    rd,
                    value: rv64_zknh_helpers::sha512sum1(x),
                }
            }
        }
    }
}
