//! Zicsr extension

#[cfg(test)]
mod tests;
pub mod zicsr_helpers;

use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for ZicsrInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for ZicsrInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for ZicsrInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: Csrs<Reg, CustomError>,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        _program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            // Atomic read/write CSR.
            //
            // Reads old CSR value into rd (unless `rd == x0`, in which case no read side effects
            // occur per spec), then writes `rs1` unconditionally.
            Self::Csrrw {
                rd,
                rs1: _,
                csr_index,
            } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if csr_is_read_only {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly { csr_index }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let write_value = rs1_value;

                // Per spec: if `rd == x0`, the CSR read (and its side effects) must not occur
                let read_output = if rd == Reg::ZERO {
                    ::core::hint::cold_path();
                    Reg::Type::from(0u8)
                } else {
                    let read_value = match ext_state.read_csr(csr_index) {
                        Ok(read_value) => read_value,
                        Err(err) => {
                            ::core::hint::cold_path();
                            return Err(ExecutionError::CsrError(err));
                        }
                    };
                    ext_state.process_csr_read::<Self>(csr_index, read_value)?
                };

                let write_output = ext_state.process_csr_write::<Self>(csr_index, write_value)?;
                match ext_state.write_csr(csr_index, write_output) {
                    Ok(()) => Ok(ControlFlow::Continue((rd, read_output))),
                    Err(err) => {
                        ::core::hint::cold_path();
                        Err(ExecutionError::CsrError(err))
                    }
                }
            }

            // Atomic read and set bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old | rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrs { rd, rs1, csr_index } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if rs1 != Reg::ZERO && csr_is_read_only {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly { csr_index }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                };
                let read_output = ext_state.process_csr_read::<Self>(csr_index, read_value)?;

                if rs1 == Reg::ZERO {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value | rs1_value;
                    let write_output =
                        ext_state.process_csr_write::<Self>(csr_index, write_value)?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                }
                Ok(ControlFlow::Continue((rd, read_output)))
            }

            // Atomic read and clear bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old & !rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrc { rd, rs1, csr_index } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if rs1 != Reg::ZERO && csr_is_read_only {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly { csr_index }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                };
                let read_output = ext_state.process_csr_read::<Self>(csr_index, read_value)?;

                if rs1 == Reg::ZERO {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value & !rs1_value;
                    let write_output =
                        ext_state.process_csr_write::<Self>(csr_index, write_value)?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                }

                Ok(ControlFlow::Continue((rd, read_output)))
            }

            // Atomic read/write CSR immediate.
            //
            // Same `rd == x0` optimization as Csrrw. Writes zero-extended `zimm` unconditionally.
            Self::Csrrwi {
                rd,
                zimm,
                csr_index,
            } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if csr_is_read_only {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly { csr_index }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_output = if rd == Reg::ZERO {
                    ::core::hint::cold_path();
                    Reg::Type::from(0u8)
                } else {
                    let read_value = match ext_state.read_csr(csr_index) {
                        Ok(read_value) => read_value,
                        Err(error) => {
                            ::core::hint::cold_path();
                            return Err(ExecutionError::CsrError(error));
                        }
                    };
                    ext_state.process_csr_read::<Self>(csr_index, read_value)?
                };

                let write_output = ext_state.process_csr_write::<Self>(csr_index, zimm.into())?;
                match ext_state.write_csr(csr_index, write_output) {
                    Ok(()) => Ok(ControlFlow::Continue((rd, read_output))),
                    Err(error) => {
                        ::core::hint::cold_path();
                        Err(ExecutionError::CsrError(error))
                    }
                }
            }

            // Atomic read and set bits in CSR immediate.
            //
            // Always reads old value into `rd`. Writes `(old | zimm)` only if `zimm != 0`.
            // Accessing a read-only CSR with `zimm == 0` is legal (pure read).
            Self::Csrrsi {
                rd,
                zimm,
                csr_index,
            } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if zimm != 0 && csr_is_read_only {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly { csr_index }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                };
                let read_output = ext_state.process_csr_read::<Self>(csr_index, read_value)?;

                if zimm == 0 {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value | Reg::Type::from(zimm);
                    let write_output =
                        ext_state.process_csr_write::<Self>(csr_index, write_value)?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                }

                Ok(ControlFlow::Continue((rd, read_output)))
            }

            // Atomic read and clear bits in CSR immediate.
            //
            // Always reads old value into `rd`. Writes `(old & !zimm)` only if `zimm != 0`.
            // Accessing a read-only CSR with `zimm == 0` is legal (pure read).
            Self::Csrrci {
                rd,
                zimm,
                csr_index,
            } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if zimm != 0 && csr_is_read_only {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly { csr_index }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                };
                let read_output = ext_state.process_csr_read::<Self>(csr_index, read_value)?;

                if zimm == 0 {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value & !Reg::Type::from(zimm);
                    let write_output =
                        ext_state.process_csr_write::<Self>(csr_index, write_value)?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return Err(ExecutionError::CsrError(error));
                    }
                }

                Ok(ControlFlow::Continue((rd, read_output)))
            }
        }
    }
}
