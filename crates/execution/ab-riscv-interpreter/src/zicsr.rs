//! Zicsr extension

#[cfg(test)]
mod tests;
pub mod zicsr_helpers;

use crate::{CsrError, Csrs, ExecutableInstruction, ExecutionError, InterpreterState};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::zicsr::ZicsrInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for ZicsrInstruction<Reg>
where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: Csrs<Reg, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
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
                zicsr_helpers::check_csr_privilege_level(&state.ext_state, csr)?;

                let write_value = state.regs.read(rs1);

                // Per spec: if `rd == x0`, the CSR read (and its side effects) must not occur
                if !rd.is_zero() {
                    let raw_value = state.ext_state.read_csr(csr)?;
                    let output_value = state.ext_state.process_csr_read(csr, raw_value)?;
                    state.regs.write(rd, output_value);
                }

                let output_value = state.ext_state.process_csr_write(csr, write_value)?;
                state.ext_state.write_csr(csr, output_value)?;
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
                zicsr_helpers::check_csr_privilege_level(&state.ext_state, csr)?;

                let rs1_value = state.regs.read(rs1);

                let raw_value = state.ext_state.read_csr(csr)?;
                let read_output = state.ext_state.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if !rs1.is_zero() {
                    let write_value = raw_value | rs1_value;
                    let write_output = state.ext_state.process_csr_write(csr, write_value)?;
                    state.ext_state.write_csr(csr, write_output)?;
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
                zicsr_helpers::check_csr_privilege_level(&state.ext_state, csr)?;

                let rs1_value = state.regs.read(rs1);

                let raw_value = state.ext_state.read_csr(csr)?;
                let read_output = state.ext_state.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if !rs1.is_zero() {
                    let write_value = raw_value & !rs1_value;
                    let write_output = state.ext_state.process_csr_write(csr, write_value)?;
                    state.ext_state.write_csr(csr, write_output)?;
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
                zicsr_helpers::check_csr_privilege_level(&state.ext_state, csr)?;

                if !rd.is_zero() {
                    let raw_value = state.ext_state.read_csr(csr)?;
                    let output_value = state.ext_state.process_csr_read(csr, raw_value)?;
                    state.regs.write(rd, output_value);
                }

                let output_value = state.ext_state.process_csr_write(csr, zimm.into())?;
                state.ext_state.write_csr(csr, output_value)?;
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
                zicsr_helpers::check_csr_privilege_level(&state.ext_state, csr)?;

                let raw_value = state.ext_state.read_csr(csr)?;
                let read_output = state.ext_state.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if zimm != 0 {
                    let write_value = raw_value | Reg::Type::from(zimm);
                    let write_output = state.ext_state.process_csr_write(csr, write_value)?;
                    state.ext_state.write_csr(csr, write_output)?;
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
                zicsr_helpers::check_csr_privilege_level(&state.ext_state, csr)?;

                let raw_value = state.ext_state.read_csr(csr)?;
                let read_output = state.ext_state.process_csr_read(csr, raw_value)?;
                state.regs.write(rd, read_output);

                if zimm != 0 {
                    let write_value = raw_value & !Into::<Reg::Type>::into(zimm);
                    let write_output = state.ext_state.process_csr_write(csr, write_value)?;
                    state.ext_state.write_csr(csr, write_output)?;
                }
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
