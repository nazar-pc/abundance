//! ZveXx carry/borrow arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zvexx_carry_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::zvexx_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for ZveXxCarryInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for ZveXxCarryInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for ZveXxCarryInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
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
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            // vadc: add with carry-in from v0, data result
            Self::VadcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // vd must not be v0: v0 holds carry-in
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VadcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VadcVim { vd, vs2, imm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            // vmadc: add and write carry-out mask
            Self::VmadcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmadcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmadcVim { vd, vs2, imm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmadcVv { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<false, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmadcVx { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<false, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmadcVi { vd, vs2, imm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<false, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            // vsbc: subtract with borrow-in from v0, data result
            Self::VsbcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VsbcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            // vmsbc: subtract and write borrow-out mask
            Self::VmsbcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmsbcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<true, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmsbcVv { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<false, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmsbcVx { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<false, Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
