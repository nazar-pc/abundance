//! RV64 Zkne extension

pub mod rv64_zkne_helpers;
// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[cfg(test)]
mod tests;

use crate::rv64::zk::zkn::zknd::rv64_zknd_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for Rv64ZkneInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for Rv64ZkneInstruction<Reg> where
    Reg: Register<Type = u64>
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for Rv64ZkneInstruction<Reg>
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
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        _ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Aes64Es { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zkne_helpers::aes64es(v1, v2),
                }
            }
            Self::Aes64Esm { rd, rs1: _, rs2: _ } => {
                let v1 = rs1_value;
                let v2 = rs2_value;
                ExecutionResult::Continue {
                    rd,
                    value: rv64_zkne_helpers::aes64esm(v1, v2),
                }
            }
        }
    }
}
