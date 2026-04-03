//! RV32 Zknh extension

#[cfg(test)]
mod tests;
pub mod zknh_helpers;

use crate::{ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv32::zk::zkn::zknh::Rv32ZknhInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv32ZknhInstruction<Reg>
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
            // SHA-256 (single-register)
            Self::Sha256Sig0 { rd, rs1 } => {
                let x = state.regs.read(rs1);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha256sig0(x) }
                    }
                    _ => {
                        x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha256Sig1 { rd, rs1 } => {
                let x = state.regs.read(rs1);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha256sig1(x) }
                    }
                    _ => {
                        x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha256Sum0 { rd, rs1 } => {
                let x = state.regs.read(rs1);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha256sum0(x) }
                    }
                    _ => {
                        x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha256Sum1 { rd, rs1 } => {
                let x = state.regs.read(rs1);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha256sum1(x) }
                    }
                    _ => {
                        x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
                    }
                };
                state.regs.write(rd, res);
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
            Self::Sha512Sig0h { rd, rs1, rs2 } => {
                // Only here to prevent compiler warnings about unused `zknh_helpers` module
                let () = zknh_helpers::PLACEHOLDER;
                let rs1_val = state.regs.read(rs1);
                let rs2_val = state.regs.read(rs2);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha512sig0h(rs1_val, rs2_val) }
                    }
                    _ => {
                        zknh_helpers::sha512sig0h(rs1_val, rs2_val)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha512Sig0l { rd, rs1, rs2 } => {
                let rs1_val = state.regs.read(rs1);
                let rs2_val = state.regs.read(rs2);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha512sig0l(rs1_val, rs2_val) }
                    }
                    _ => {
                        zknh_helpers::sha512sig0l(rs1_val, rs2_val)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha512Sig1h { rd, rs1, rs2 } => {
                let rs1_val = state.regs.read(rs1);
                let rs2_val = state.regs.read(rs2);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha512sig1h(rs1_val, rs2_val) }
                    }
                    _ => {
                        zknh_helpers::sha512sig1h(rs1_val, rs2_val)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha512Sig1l { rd, rs1, rs2 } => {
                let rs1_val = state.regs.read(rs1);
                let rs2_val = state.regs.read(rs2);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha512sig1l(rs1_val, rs2_val) }
                    }
                    _ => {
                        zknh_helpers::sha512sig1l(rs1_val, rs2_val)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha512Sum0r { rd, rs1, rs2 } => {
                let rs1_val = state.regs.read(rs1);
                let rs2_val = state.regs.read(rs2);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha512sum0r(rs1_val, rs2_val) }
                    }
                    _ => {
                        zknh_helpers::sha512sum0r(rs1_val, rs2_val)
                    }
                };
                state.regs.write(rd, res);
            }
            Self::Sha512Sum1r { rd, rs1, rs2 } => {
                let rs1_val = state.regs.read(rs1);
                let rs2_val = state.regs.read(rs2);
                let res = cfg_select! {
                    all(not(miri), target_arch = "riscv32", target_feature = "zknh") => {
                        // SAFETY: Just an intrinsic, no undefined behavior
                        unsafe { core::arch::riscv32::sha512sum1r(rs1_val, rs2_val) }
                    }
                    _ => {
                        zknh_helpers::sha512sum1r(rs1_val, rs2_val)
                    }
                };
                state.regs.write(rd, res);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
