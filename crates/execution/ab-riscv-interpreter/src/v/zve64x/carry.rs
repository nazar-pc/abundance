//! Zve64x carry/borrow arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zve64x_carry_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Zve64xCarryInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xCarryInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xCarryInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
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
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // vd must not be v0: v0 holds carry-in
                if vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Vreg(vs1),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VadcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VadcVim { vd, vs2, imm } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            // vmadc: add and write carry-out mask
            Self::VmadcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Vreg(vs1),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmadcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmadcVim { vd, vs2, imm } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmadcVv { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Vreg(vs1),
                        false,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmadcVx { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        false,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmadcVi { vd, vs2, imm } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_add_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        false,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            // vsbc: subtract with borrow-in from v0, data result
            Self::VsbcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_sub::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Vreg(vs1),
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VsbcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_sub::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            // vmsbc: subtract and write borrow-out mask
            Self::VmsbcVvm { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_sub_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Vreg(vs1),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmsbcVxm { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_sub_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        true,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmsbcVv { vd, vs2, vs1 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_sub_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Vreg(vs1),
                        false,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }

            Self::VmsbcVx { vd, vs2, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_carry_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_carry_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignments and overlap checked above
                unsafe {
                    zve64x_carry_helpers::execute_carry_sub_mask::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_carry_helpers::OpSrc::Scalar(scalar),
                        false,
                        vl,
                        vstart,
                        sew,
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
