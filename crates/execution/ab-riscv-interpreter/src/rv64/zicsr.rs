//! RV64 Zicsr extension

#[cfg(test)]
mod tests;

use crate::rv64::Rv64InterpreterState;
use crate::{CsrError, Csrs, ExecutableInstruction, ExecutionError};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::rv64::zicsr::Rv64ZicsrInstruction;
use ab_riscv_primitives::privilege::PrivilegeLevel;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtRegs, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        Rv64InterpreterState<Reg, ExtRegs, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Rv64ZicsrInstruction<Reg>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    ExtRegs: Csrs<Reg, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut Rv64InterpreterState<Reg, ExtRegs, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, Self, CustomError>> {
        match self {
            // Atomic read/write CSR.
            //
            // Reads old CSR value into rd (unless `rd == x0`, in which case no read side effects
            // occur per spec), then writes `rs1` unconditionally.
            Self::Csrrw { rd, rs1, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                check_csr_privilege_level(&state.ext_regs, csr)?;

                let write_value = state.regs.read(rs1);

                // Per spec: if `rd == x0`, the CSR read (and its side effects) must not occur
                if !rd.is_zero() {
                    let raw_value = state.ext_regs.read_csr(csr)?;
                    let output_value = state.ext_regs.process_csr_read(csr, raw_value)?;
                    state.regs.write(rd, output_value);
                }

                let output_value = state.ext_regs.process_csr_write(csr, write_value)?;
                state.ext_regs.write_csr(csr, output_value)?;
            }

            // Atomic read and set bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old | rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrs { rd, rs1, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if !rs1.is_zero() && csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                check_csr_privilege_level(&state.ext_regs, csr)?;

                let rs1_value = state.regs.read(rs1);

                let raw_value = state.ext_regs.read_csr(csr)?;
                let read_output = state.ext_regs.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if !rs1.is_zero() {
                    let write_value = raw_value | rs1_value;
                    let write_output = state.ext_regs.process_csr_write(csr, write_value)?;
                    state.ext_regs.write_csr(csr, write_output)?;
                }
            }

            // Atomic read and clear bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old & !rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrc { rd, rs1, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if !rs1.is_zero() && csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                check_csr_privilege_level(&state.ext_regs, csr)?;

                let rs1_value = state.regs.read(rs1);

                let raw_value = state.ext_regs.read_csr(csr)?;
                let read_output = state.ext_regs.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if !rs1.is_zero() {
                    let write_value = raw_value & !rs1_value;
                    let write_output = state.ext_regs.process_csr_write(csr, write_value)?;
                    state.ext_regs.write_csr(csr, write_output)?;
                }
            }

            // Atomic read/write CSR immediate.
            //
            // Same `rd == x0` optimization as Csrrw. Writes zero-extended `zimm` unconditionally.
            Self::Csrrwi { rd, zimm, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                check_csr_privilege_level(&state.ext_regs, csr)?;

                let write_value = zimm as u64;

                if !rd.is_zero() {
                    let raw_value = state.ext_regs.read_csr(csr)?;
                    let output_value = state.ext_regs.process_csr_read(csr, raw_value)?;
                    state.regs.write(rd, output_value);
                }

                let output_value = state.ext_regs.process_csr_write(csr, write_value)?;
                state.ext_regs.write_csr(csr, output_value)?;
            }

            // Atomic read and set bits in CSR immediate.
            //
            // Always reads old value into `rd`. Writes `(old | zimm)` only if `zimm != 0`.
            // Accessing a read-only CSR with `zimm == 0` is legal (pure read).
            Self::Csrrsi { rd, zimm, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if zimm != 0 && csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                check_csr_privilege_level(&state.ext_regs, csr)?;

                let raw_value = state.ext_regs.read_csr(csr)?;
                let read_output = state.ext_regs.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if zimm != 0 {
                    let write_value = raw_value | (zimm as u64);
                    let write_output = state.ext_regs.process_csr_write(csr, write_value)?;
                    state.ext_regs.write_csr(csr, write_output)?;
                }
            }

            // Atomic read and clear bits in CSR immediate.
            //
            // Always reads old value into `rd`. Writes `(old & !zimm)` only if `zimm != 0`.
            // Accessing a read-only CSR with `zimm == 0` is legal (pure read).
            Self::Csrrci { rd, zimm, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if zimm != 0 && csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                check_csr_privilege_level(&state.ext_regs, csr)?;

                let raw_value = state.ext_regs.read_csr(csr)?;
                let read_output = state.ext_regs.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if zimm != 0 {
                    let write_value = raw_value & !(zimm as u64);
                    let write_output = state.ext_regs.process_csr_write(csr, write_value)?;
                    state.ext_regs.write_csr(csr, write_output)?;
                }
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}

/// CSR privilege level check helper.
///
/// Returns `Err` if `current` is below the privilege level encoded in `csr_index` bits `[9:8]`
#[inline(always)]
pub fn check_csr_privilege_level<Reg, C, CustomError>(
    csrs: &C,
    csr_index: u16,
) -> Result<(), CsrError<CustomError>>
where
    Reg: Register<Type = u64>,
    [(); Reg::N]:,
    C: Csrs<Reg, CustomError>,
{
    let current = csrs.privilege_level();
    let required_bits = ((csr_index >> 8) & 0b11) as u8;
    let required =
        PrivilegeLevel::from_bits(required_bits).ok_or(CsrError::Unknown { csr_index })?;

    if current >= required {
        Ok(())
    } else {
        Err(CsrError::InsufficientPrivilege {
            csr_index,
            required,
            current,
        })
    }
}
