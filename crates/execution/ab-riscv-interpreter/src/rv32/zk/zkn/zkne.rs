//! RV32 Zkne extension

pub mod rv32_zkne_helpers;
#[cfg(test)]
mod tests;

use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv32ZkneInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv32ZkneInstruction<Reg> where
    Reg: Register<Type = u32>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv32ZkneInstruction<Reg>
where
    Reg: Register<Type = u32>,
    Regs: RegisterFile<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Aes32Esi {
                rd,
                rs1: _,
                rs2: _,
                bs,
            } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv32_zkne_helpers::aes32esi(v1, v2, bs),
                }
            }
            Self::Aes32Esmi {
                rd,
                rs1: _,
                rs2: _,
                bs,
            } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv32_zkne_helpers::aes32esmi(v1, v2, bs),
                }
            }
        }
    }
}
