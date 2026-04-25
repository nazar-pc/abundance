//! RV64 Zkne extension

pub mod rv64_zkne_helpers;
// TODO: `llvm.aarch64.crypto.aes*` is not supported in Miri yet:
//  https://github.com/rust-lang/miri/issues/3172#issuecomment-3730602707
#[cfg(not(all(miri, target_arch = "aarch64")))]
#[cfg(test)]
mod tests;

use crate::rv64::zk::zkn::zknd::rv64_zknd_helpers;
use crate::{ExecutableInstruction, ExecutionError, RegisterFile};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Rv64ZkneInstruction<Reg>
where
    Reg: Register<Type = u64>,
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
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Aes64Es { rd, rs1, rs2 } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                regs.write(rd, rv64_zkne_helpers::aes64es(v1, v2));
            }
            Self::Aes64Esm { rd, rs1, rs2 } => {
                let v1 = regs.read(rs1);
                let v2 = regs.read(rs2);
                regs.write(rd, rv64_zkne_helpers::aes64esm(v1, v2));
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
