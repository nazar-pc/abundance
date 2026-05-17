//! Zve64x configuration instructions

#[cfg(test)]
mod tests;
pub mod zve64x_config_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Zve64xConfigInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xConfigInstruction<Reg>
where
    Reg: Register,
    ExtState: Csrs<Reg, CustomError>,
{
    /// Validate reads to vector CSRs from Zicsr instructions.
    ///
    /// All vector CSRs are accessible from unprivileged code (U-mode).
    /// Reads are pass-through: the raw value stored in the CSR is the output value.
    fn prepare_csr_read(
        _ext_state: &ExtState,
        csr_index: u16,
        raw_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError<CustomError>> {
        if VCsr::from_index(csr_index).is_some() {
            *output_value = raw_value;
            Ok(true)
        } else {
            // Not a vector CSR
            Ok(false)
        }
    }

    /// Validate, sanitize, and mirror writes to vector CSRs from Zicsr instructions.
    ///
    /// Enforces WARL semantics and vcsr mirroring:
    /// - `vl`, `vtype`, `vlenb` are read-only: writes are rejected
    /// - `vxsat`: only bit 0 is writable; mirrors into `vcsr[0]`
    /// - `vxrm`: only bits `[1:0]` are writable; mirrors into `vcsr[2:1]`
    /// - `vcsr`: only bits `[2:0]` are writable; mirrors into `vxsat` and `vxrm`
    /// - `vstart`: full XLEN write allowed (WARL, implementation may restrict range)
    fn prepare_csr_write(
        ext_state: &mut ExtState,
        csr_index: u16,
        write_value: Reg::Type,
        output_value: &mut Reg::Type,
    ) -> Result<bool, CsrError<CustomError>> {
        if let Some(vcsr) = VCsr::from_index(csr_index) {
            // WARL: mask to valid bits, zero upper bits
            *output_value = match vcsr {
                VCsr::Vstart => {
                    // WARL: allow full XLEN write, but clamp to implementation-supported range
                    let max = Reg::Type::from(u16::MAX);
                    write_value.min(max)
                }
                VCsr::Vxsat => {
                    let masked = write_value & Reg::Type::from(1u8);
                    // Mirror `vxsat` into `vcsr[0]`, preserving `vcsr[2:1]` (`vxrm`)
                    let old_vcsr = ext_state.read_csr(VCsr::Vcsr as u16)?;
                    let new_vcsr = (old_vcsr & !Reg::Type::from(1u8)) | masked;
                    ext_state.write_csr(VCsr::Vcsr as u16, new_vcsr)?;
                    masked
                }
                VCsr::Vxrm => {
                    let masked = write_value & Reg::Type::from(0b11u8);
                    // Mirror `vxrm` into `vcsr[2:1]`, preserving `vcsr[0]` (`vxsat`)
                    let old_vcsr = ext_state.read_csr(VCsr::Vcsr as u16)?;
                    let new_vcsr = (old_vcsr & !Reg::Type::from(0b110u8)) | (masked << 1);
                    ext_state.write_csr(VCsr::Vcsr as u16, new_vcsr)?;
                    masked
                }
                VCsr::Vcsr => {
                    // Mirror `vcsr[0]` -> `vxsat`
                    let new_vxsat = write_value & Reg::Type::from(1u8);
                    ext_state.write_csr(VCsr::Vxsat as u16, new_vxsat)?;

                    // Mirror `vcsr[2:1]` -> `vxrm`
                    let new_vxrm = (write_value >> 1) & Reg::Type::from(0b11u8);
                    ext_state.write_csr(VCsr::Vxrm as u16, new_vxrm)?;

                    write_value & Reg::Type::from(0b111u8)
                }
                VCsr::Vl | VCsr::Vtype | VCsr::Vlenb => {
                    // Read-only CSRs (from Zicsr perspective)
                    Err(CsrError::ReadOnly { csr_index })?
                }
            };
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xConfigInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            Self::Vsetvli { rd, rs1, vtypei } => {
                zve64x_config_helpers::apply_vsetvl(
                    regs,
                    ext_state,
                    program_counter,
                    rd,
                    rs1,
                    rs1_value,
                    Reg::Type::from(vtypei),
                )?;
            }
            Self::Vsetivli { rd, uimm, vtypei } => {
                zve64x_config_helpers::apply_vsetivli(
                    regs,
                    ext_state,
                    program_counter,
                    rd,
                    uimm,
                    vtypei,
                )?;
            }
            Self::Vsetvl { rd, rs1, rs2: _ } => {
                let vtype_raw = rs2_value;
                zve64x_config_helpers::apply_vsetvl(
                    regs,
                    ext_state,
                    program_counter,
                    rd,
                    rs1,
                    rs1_value,
                    vtype_raw,
                )?;
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
