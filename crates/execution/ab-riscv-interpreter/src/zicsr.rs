//! Zicsr extension

#[cfg(test)]
mod tests;
pub mod zicsr_helpers;

use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutionError, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::ops::ControlFlow;

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
        regs: &mut Regs,
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
            Self::Csrrw { rd, rs1: _, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr)?;

                let write_value = rs1_value;

                // Per spec: if `rd == x0`, the CSR read (and its side effects) must not occur
                if rd != Reg::ZERO {
                    let raw_value = ext_state.read_csr(csr)?;
                    let output_value = ext_state.process_csr_read(csr, raw_value)?;
                    regs.write(rd, output_value);
                }

                let output_value = ext_state.process_csr_write(csr, write_value)?;
                ext_state.write_csr(csr, output_value)?;
            }

            // Atomic read and set bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old | rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrs { rd, rs1, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if rs1 != Reg::ZERO && csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr)?;

                let raw_value = ext_state.read_csr(csr)?;
                let read_output = ext_state.process_csr_read(csr, raw_value)?;
                regs.write(rd, read_output);

                if rs1 != Reg::ZERO {
                    let write_value = raw_value | rs1_value;
                    let write_output = ext_state.process_csr_write(csr, write_value)?;
                    ext_state.write_csr(csr, write_output)?;
                }
            }

            // Atomic read and clear bits in CSR.
            //
            // Always reads old value into `rd`. Writes `(old & !rs1)` only if `rs1 != x0`.
            // Accessing a read-only CSR with `rs1 == x0` is legal (pure read).
            Self::Csrrc { rd, rs1, csr } => {
                let csr_is_read_only = (csr >> 10) == 0b11;
                if rs1 != Reg::ZERO && csr_is_read_only {
                    return Err(ExecutionError::CsrError(CsrError::ReadOnly {
                        csr_index: csr,
                    }));
                }
                zicsr_helpers::check_csr_privilege_level(ext_state, csr)?;

                let raw_value = ext_state.read_csr(csr)?;
                let read_output = ext_state.process_csr_read(csr, raw_value)?;
                regs.write(rd, read_output);

                if rs1 != Reg::ZERO {
                    let write_value = raw_value & !rs1_value;
                    let write_output = ext_state.process_csr_write(csr, write_value)?;
                    ext_state.write_csr(csr, write_output)?;
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
                zicsr_helpers::check_csr_privilege_level(ext_state, csr)?;

                if rd != Reg::ZERO {
                    let raw_value = ext_state.read_csr(csr)?;
                    let output_value = ext_state.process_csr_read(csr, raw_value)?;
                    regs.write(rd, output_value);
                }

                let output_value = ext_state.process_csr_write(csr, zimm.into())?;
                ext_state.write_csr(csr, output_value)?;
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
                zicsr_helpers::check_csr_privilege_level(ext_state, csr)?;

                let raw_value = ext_state.read_csr(csr)?;
                let read_output = ext_state.process_csr_read(csr, raw_value)?;
                regs.write(rd, read_output);

                if zimm != 0 {
                    let write_value = raw_value | Reg::Type::from(zimm);
                    let write_output = ext_state.process_csr_write(csr, write_value)?;
                    ext_state.write_csr(csr, write_output)?;
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
                zicsr_helpers::check_csr_privilege_level(ext_state, csr)?;

                let raw_value = ext_state.read_csr(csr)?;
                let read_output = ext_state.process_csr_read(csr, raw_value)?;
                regs.write(rd, read_output);

                if zimm != 0 {
                    let write_value = raw_value & !Reg::Type::from(zimm);
                    let write_output = ext_state.process_csr_write(csr, write_value)?;
                    ext_state.write_csr(csr, write_output)?;
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
