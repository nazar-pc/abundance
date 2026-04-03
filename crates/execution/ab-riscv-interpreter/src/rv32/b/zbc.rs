//! RV32 Zbc extension

#[cfg(test)]
mod tests;
pub mod zbc_helpers;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv32::b::zbc::Rv32ZbcInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32ZbcInstruction<Reg>
where
    Reg: Register<Type = u32>,
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
                    all(not(miri), target_arch = "riscv32", target_feature = "zbkc") => {
                        core::arch::riscv32::clmul(a as usize, b as usize) as u32
                    }
                    _ => {{
                        let result = zbc_helpers::clmul_internal(a, b);
                        result as u32
                    }}
                };

                state.regs.write(rd, value);
            }
            Self::Clmulh { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zbkc") => {
                        core::arch::riscv32::clmulh(a as usize, b as usize) as u32
                    }
                    _ => {{
                        let result = zbc_helpers::clmul_internal(a, b);
                        (result >> 32) as u32
                    }}
                };
                state.regs.write(rd, value);
            }
            Self::Clmulr { rd, rs1, rs2 } => {
                let a = state.regs.read(rs1);
                let b = state.regs.read(rs2);

                // TODO: Miri is excluded because corresponding intrinsic is not implemented there
                let value = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zbc") => {
                        core::arch::riscv32::clmulr(a as usize, b as usize) as u32
                    }
                    _ => {{
                        let result = zbc_helpers::clmul_internal(a, b);
                        (result >> 31) as u32
                    }}
                };

                state.regs.write(rd, value);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
