//! RV64 Zknh extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{ExecutableInstruction, ExecutionError};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instruction::rv64::zk::zkn::zknh::Rv64ZknhInstruction;
use ab_riscv_primitives::registers::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZknhInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            Self::Sha256Sig0 { rd, rs1 } => {
                let x = state.regs.read(rs1) as u32;

                let res32 = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha256sig0(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
                    }
                };

                state
                    .regs
                    .write(rd, i64::from(res32.cast_signed()).cast_unsigned());
            }
            Self::Sha256Sig1 { rd, rs1 } => {
                let x = state.regs.read(rs1) as u32;

                let res32 = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha256sig1(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
                    }
                };

                state
                    .regs
                    .write(rd, i64::from(res32.cast_signed()).cast_unsigned());
            }
            Self::Sha256Sum0 { rd, rs1 } => {
                let x = state.regs.read(rs1) as u32;

                let res32 = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha256sum0(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
                    }
                };

                state
                    .regs
                    .write(rd, i64::from(res32.cast_signed()).cast_unsigned());
            }
            Self::Sha256Sum1 { rd, rs1 } => {
                let x = state.regs.read(rs1) as u32;

                let res32 = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha256sum1(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
                    }
                };

                state
                    .regs
                    .write(rd, i64::from(res32.cast_signed()).cast_unsigned());
            }
            Self::Sha512Sig0 { rd, rs1 } => {
                let x = state.regs.read(rs1);

                let res = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha512sig0(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(1) ^ x.rotate_right(8) ^ (x >> 7)
                    }
                };

                state.regs.write(rd, res);
            }
            Self::Sha512Sig1 { rd, rs1 } => {
                let x = state.regs.read(rs1);

                let res = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha512sig1(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(19) ^ x.rotate_right(61) ^ (x >> 6)
                    }
                };

                state.regs.write(rd, res);
            }
            Self::Sha512Sum0 { rd, rs1 } => {
                let x = state.regs.read(rs1);

                let res = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha512sum0(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(28) ^ x.rotate_right(34) ^ x.rotate_right(39)
                    }
                };

                state.regs.write(rd, res);
            }
            Self::Sha512Sum1 { rd, rs1 } => {
                let x = state.regs.read(rs1);

                let res = {
                    // SAFETY: Just an intrinsic, no undefined behavior
                    #[cfg(all(not(miri), target_arch = "riscv64", target_feature = "zknh"))]
                    unsafe {
                        core::arch::riscv64::sha512sum1(x)
                    }

                    #[cfg(not(all(not(miri), target_arch = "riscv64", target_feature = "zknh")))]
                    {
                        x.rotate_right(14) ^ x.rotate_right(18) ^ x.rotate_right(41)
                    }
                };

                state.regs.write(rd, res);
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
