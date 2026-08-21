//! Zicsr extension

#[cfg(test)]
mod tests;
pub mod zicsr_helpers;

use crate::{
    Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, ExecutionResult, FetchInstructionResult, InstructionFetcher,
    OpaqueThreadedExecutionResult, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    ThreadedExecutableInstruction, ThreadedExecutionResult,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZicsrInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for ZicsrInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
const impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for ZicsrInstruction<Reg>
where
    Reg: [const] Register,
    Regs: [const] RegisterFile<Reg>,
    ExtState: [const] Csrs<Reg>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
    ) -> ExecutionResult<Self::Reg> {
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
                    return ExecutionResult::Err(ExecutionError::CsrReadOnly { csr_index });
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
                            return ExecutionResult::Err(ExecutionError::from(err));
                        }
                    };
                    zicsr_helpers::process_csr_read::<Reg, ExtState, Self>(
                        ext_state, csr_index, true, read_value,
                    )?
                };

                let write_output = zicsr_helpers::process_csr_write::<Reg, ExtState, Self>(
                    ext_state,
                    csr_index,
                    write_value,
                )?;
                match ext_state.write_csr(csr_index, write_output) {
                    Ok(()) => ExecutionResult::Continue {
                        rd,
                        value: read_output,
                    },
                    Err(err) => {
                        ::core::hint::cold_path();
                        ExecutionResult::Err(ExecutionError::from(err))
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
                    return ExecutionResult::Err(ExecutionError::CsrReadOnly { csr_index });
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                };
                let read_output = zicsr_helpers::process_csr_read::<Reg, ExtState, Self>(
                    ext_state,
                    csr_index,
                    rs1 != Reg::ZERO,
                    read_value,
                )?;

                if rs1 == Reg::ZERO {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value | rs1_value;
                    let write_output = zicsr_helpers::process_csr_write::<Reg, ExtState, Self>(
                        ext_state,
                        csr_index,
                        write_value,
                    )?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                }
                ExecutionResult::Continue {
                    rd,
                    value: read_output,
                }
            }

            // Atomic read and clear bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old & !rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrc { rd, rs1, csr_index } => {
                let csr_is_read_only = (csr_index >> 10) == 0b11;
                if rs1 != Reg::ZERO && csr_is_read_only {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::CsrReadOnly { csr_index });
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                };
                let read_output = zicsr_helpers::process_csr_read::<Reg, ExtState, Self>(
                    ext_state,
                    csr_index,
                    rs1 != Reg::ZERO,
                    read_value,
                )?;

                if rs1 == Reg::ZERO {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value & !rs1_value;
                    let write_output = zicsr_helpers::process_csr_write::<Reg, ExtState, Self>(
                        ext_state,
                        csr_index,
                        write_value,
                    )?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                }

                ExecutionResult::Continue {
                    rd,
                    value: read_output,
                }
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
                    return ExecutionResult::Err(ExecutionError::CsrReadOnly { csr_index });
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
                            return ExecutionResult::Err(ExecutionError::from(error));
                        }
                    };
                    zicsr_helpers::process_csr_read::<Reg, ExtState, Self>(
                        ext_state, csr_index, true, read_value,
                    )?
                };

                let write_output = zicsr_helpers::process_csr_write::<Reg, ExtState, Self>(
                    ext_state,
                    csr_index,
                    zimm.into(),
                )?;
                match ext_state.write_csr(csr_index, write_output) {
                    Ok(()) => ExecutionResult::Continue {
                        rd,
                        value: read_output,
                    },
                    Err(error) => {
                        ::core::hint::cold_path();
                        ExecutionResult::Err(ExecutionError::from(error))
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
                    return ExecutionResult::Err(ExecutionError::CsrReadOnly { csr_index });
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                };
                let read_output = zicsr_helpers::process_csr_read::<Reg, ExtState, Self>(
                    ext_state,
                    csr_index,
                    zimm != 0,
                    read_value,
                )?;

                if zimm == 0 {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value | Reg::Type::from(zimm);
                    let write_output = zicsr_helpers::process_csr_write::<Reg, ExtState, Self>(
                        ext_state,
                        csr_index,
                        write_value,
                    )?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                }

                ExecutionResult::Continue {
                    rd,
                    value: read_output,
                }
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
                    return ExecutionResult::Err(ExecutionError::CsrReadOnly { csr_index });
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr_index)?;

                let read_value = match ext_state.read_csr(csr_index) {
                    Ok(read_value) => read_value,
                    Err(error) => {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                };
                let read_output = zicsr_helpers::process_csr_read::<Reg, ExtState, Self>(
                    ext_state,
                    csr_index,
                    zimm != 0,
                    read_value,
                )?;

                if zimm == 0 {
                    ::core::hint::cold_path();
                } else {
                    let write_value = read_value & !Reg::Type::from(zimm);
                    let write_output = zicsr_helpers::process_csr_write::<Reg, ExtState, Self>(
                        ext_state,
                        csr_index,
                        write_value,
                    )?;
                    if let Err(error) = ext_state.write_csr(csr_index, write_output) {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::from(error));
                    }
                }

                ExecutionResult::Continue {
                    rd,
                    value: read_output,
                }
            }
        }
    }
}
