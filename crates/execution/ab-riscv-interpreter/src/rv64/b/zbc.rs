//! RV64 Zbc extension

#[cfg(test)]
mod tests;
pub mod zbc_helpers;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::b::zbc::Rv64ZbcInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZbcInstruction<Reg>
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
            Self::Clmul { rd, rs1, rs2 } => {
                // Only here to prevent compiler warnings about unused `zbc_helpers` module
                let () = zbc_helpers::PLACEHOLDER;
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv64", target_feature = "zbkc") => {
                        core::arch::riscv64::clmul(a as usize, b as usize) as u64
                    }
                    _ => {{
                        let result = zbc_helpers::clmul_internal(a, b);
                        result as u64
                    }}
                };

                state.regs.write(rd, value);
            }
            Self::Clmulh { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv64", target_feature = "zbkc") => {
                        core::arch::riscv64::clmulh(a as usize, b as usize) as u64
                    }
                    _ => {{
                        let result = zbc_helpers::clmul_internal(a, b);
                        (result >> 64) as u64
                    }}
                };

                state.regs.write(rd, value);
            }
            Self::Clmulr { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv64", target_feature = "zbc") => {
                        core::arch::riscv64::clmulr(a as usize, b as usize) as u64
                    }
                    _ => {{
                        let result = zbc_helpers::clmul_internal(a, b);
                        (result >> 63) as u64
                    }}
                };

                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
