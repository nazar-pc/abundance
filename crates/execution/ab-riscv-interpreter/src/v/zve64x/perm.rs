//! Zve64x permutation instructions

#[cfg(test)]
mod tests;
pub mod zve64x_perm_helpers;

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
impl<Reg> ExecutableInstructionOperands for Zve64xPermInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xPermInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xPermInstruction<Reg>
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
            // vmv.x.s rd, vs2
            // Copies sign-extended element 0 of vs2 (at current SEW) to GPR rd.
            // Requires valid vtype (needs SEW to know element width).
            // Does not use vl or masking; always reads element 0.
            // Resets vstart per spec §6.3.
            Self::VmvXS { rd, vs2 } => {
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
                let sew = vtype.vsew();
                // SAFETY: element 0 is always within register v(vs2_base), byte offset 0;
                // VLENB >= sew.bytes() for all legal vtype configurations.
                let raw = unsafe {
                    zve64x_perm_helpers::read_element_0_u64(ext_state.read_vreg(), vs2.bits(), sew)
                };
                let sign_extended = zve64x_perm_helpers::sign_extend_to_reg::<Reg>(raw, sew);
                regs.write(rd, sign_extended);
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }
            // vmv.s.x vd, rs1
            // Copies scalar GPR rs1 (zero-extended / truncated to SEW) into element 0 of vd.
            // When vl == 0, the write is suppressed but vstart is still reset.
            // Resets vstart per spec §6.3.
            Self::VmvSX { vd, rs1: _ } => {
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
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // Per spec §16.1: update only when vstart < vl.
                if vstart < vl {
                    let scalar = rs1_value.as_u64();
                    // SAFETY: element 0 always fits.
                    unsafe {
                        zve64x_perm_helpers::write_element_0_u64(
                            ext_state.write_vreg(),
                            vd.bits(),
                            sew,
                            scalar,
                        );
                    }
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }
            // vslideup.vx vd, vs2, rs1: _, vm
            // Slides elements of vs2 up by the scalar offset in rs1.
            // Elements vd[0..offset] are unchanged (tail-undisturbed for those positions).
            // Elements vd[i] for offset <= i < vl get vs2[i - offset].
            // Per spec §16.3.1: vd must not overlap vs2.
            Self::VslideupVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // vd must not overlap vs2
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let offset = rs1_value.as_u64();
                // SAFETY: alignment and no-overlap verified above; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_slideup(
                        ext_state, vd, vs2, vm, vl, vstart, sew, offset,
                    );
                }
            }
            // vslideup.vi vd, vs2, uimm, vm
            // Same as vslideup.vx but offset is a 5-bit unsigned immediate.
            Self::VslideupVi { vd, vs2, uimm, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let offset = u64::from(uimm);
                // SAFETY: same as VslideupVx.
                unsafe {
                    zve64x_perm_helpers::execute_slideup(
                        ext_state, vd, vs2, vm, vl, vstart, sew, offset,
                    );
                }
            }
            // vslidedown.vx vd, vs2, rs1: _, vm
            // Element vd[i] = vs2[i + offset] if i + offset < VLMAX, else 0.
            // vd may overlap vs2 for slidedown.
            Self::VslidedownVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let vlmax = ext_state.vlmax_for_vtype(vtype);
                let offset = rs1_value.as_u64();
                // SAFETY: alignment verified above; vl <= VLMAX; offset clamped in helper.
                unsafe {
                    zve64x_perm_helpers::execute_slidedown(
                        ext_state, vd, vs2, vm, vl, vstart, sew, vlmax, offset,
                    );
                }
            }
            // vslidedown.vi vd, vs2, uimm, vm
            // Same as vslidedown.vx but offset is a 5-bit unsigned immediate.
            Self::VslidedownVi { vd, vs2, uimm, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let vlmax = ext_state.vlmax_for_vtype(vtype);
                let offset = u64::from(uimm);
                // SAFETY: same as VslidedownVx.
                unsafe {
                    zve64x_perm_helpers::execute_slidedown(
                        ext_state, vd, vs2, vm, vl, vstart, sew, vlmax, offset,
                    );
                }
            }
            // vslide1up.vx vd, vs2, rs1: _, vm
            // Element 0 of vd gets the scalar value rs1 (written at SEW width).
            // Elements vd[i] for 1 <= i < vl get vs2[i - 1].
            // vd must not overlap vs2.
            Self::Vslide1upVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and no-overlap verified; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_slide1up(
                        ext_state, vd, vs2, vm, vl, vstart, sew, scalar,
                    );
                }
            }
            // vslide1down.vx vd, vs2, rs1: _, vm
            // Element vd[i] = vs2[i + 1] for 0 <= i < vl - 1.
            // Element vd[vl - 1] gets the scalar value rs1.
            // vd may overlap vs2 for slide1down.
            Self::Vslide1downVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment verified; vl <= VLMAX; overlap permitted by spec.
                unsafe {
                    zve64x_perm_helpers::execute_slide1down(
                        ext_state, vd, vs2, vm, vl, vstart, sew, scalar,
                    );
                }
            }
            // vrgather.vv vd, vs2, vs1, vm
            // vd[i] = (vs1[i] < VLMAX) ? vs2[vs1[i]] : 0
            // vd must not overlap vs1 or vs2.
            Self::VrgatherVv { vd, vs2, vs1, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let vlmax = ext_state.vlmax_for_vtype(vtype);
                // SAFETY: all alignment and overlap constraints verified above; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_rgather_vv(
                        ext_state, vd, vs2, vs1, vm, vl, vstart, sew, vlmax,
                    );
                }
            }
            // vrgather.vx vd, vs2, rs1: _, vm
            // All active elements of vd get vs2[rs1] if rs1 < VLMAX, else 0.
            // vd must not overlap vs2.
            Self::VrgatherVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let vlmax = ext_state.vlmax_for_vtype(vtype);
                let index = rs1_value.as_u64();
                // SAFETY: alignment and no-overlap verified; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_rgather_scalar(
                        ext_state, vd, vs2, vm, vl, vstart, sew, vlmax, index,
                    );
                }
            }
            // vrgather.vi vd, vs2, uimm, vm
            // Same as vrgather.vx but index is a 5-bit unsigned immediate.
            Self::VrgatherVi { vd, vs2, uimm, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let vlmax = ext_state.vlmax_for_vtype(vtype);
                let index = u64::from(uimm);
                // SAFETY: same as VrgatherVx.
                unsafe {
                    zve64x_perm_helpers::execute_rgather_scalar(
                        ext_state, vd, vs2, vm, vl, vstart, sew, vlmax, index,
                    );
                }
            }
            // vrgatherei16.vv vd, vs2, vs1, vm
            // Like vrgather.vv but vs1 always uses EEW=16 (regardless of SEW).
            // EMUL_vs1 = (16 / SEW) * LMUL; must be in [1/8, 8] else illegal.
            // vd must not overlap vs1 or vs2.
            Self::Vrgatherei16Vv { vd, vs2, vs1, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // Compute EMUL for vs1 index register (EEW=16).
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(
                        ab_riscv_primitives::instructions::v::Eew::E16,
                        vtype.vsew(),
                    )
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    index_group_regs,
                )?;
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                // vd and vs1 have different group sizes (group_regs vs index_group_regs),
                // so the symmetric helper would use the wrong size for one of the intervals.
                zve64x_perm_helpers::check_no_overlap_asymmetric::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs1,
                    index_group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let vlmax = ext_state.vlmax_for_vtype(vtype);
                // SAFETY: all alignment and overlap constraints verified; vl <= VLMAX;
                // vs1 uses EEW=16 with computed index_group_regs.
                unsafe {
                    zve64x_perm_helpers::execute_rgatherei16(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        vlmax,
                        index_group_regs,
                    );
                }
            }
            // vmerge.vvm / vmv.v.v
            // When vm=true: vmv.v.v vd, vs1 - broadcast all active elements from vs1.
            //   vs2 is ignored; no overlap restriction on vd/vs2.
            // When vm=false: vmerge.vvm vd, vs2, vs1, v0
            //   vd[i] = v0[i] ? vs1[i] : vs2[i]
            //   vd must not overlap v0 (mask source).
            Self::VmergeVvm { vd, vs2, vs1, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm {
                    // vmerge: vs2 is read, vd must not overlap v0
                    zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                        program_counter,
                        vs2,
                        group_regs,
                    )?;
                    zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                        program_counter,
                        vd,
                        VReg::V0,
                        group_regs,
                    )?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap verified above; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_merge_vv(
                        ext_state, vd, vs2, vs1, vm, vl, vstart, sew,
                    );
                }
            }
            // vmerge.vxm / vmv.v.x
            // When vm=true: vmv.v.x vd, rs1 - broadcast scalar to all active elements.
            // When vm=false: vmerge.vxm - vd[i] = v0[i] ? rs1 : vs2[i]
            Self::VmergeVxm {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm {
                    zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                        program_counter,
                        vs2,
                        group_regs,
                    )?;
                    zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                        program_counter,
                        vd,
                        VReg::V0,
                        group_regs,
                    )?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap verified above; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_merge_scalar(
                        ext_state, vd, vs2, vm, vl, vstart, sew, scalar,
                    );
                }
            }
            // vmerge.vim / vmv.v.i
            // When vm=true: vmv.v.i vd, simm5 - broadcast sign-extended immediate.
            // When vm=false: vmerge.vim - vd[i] = v0[i] ? simm5 : vs2[i]
            Self::VmergeVim { vd, vs2, simm5, vm } => {
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
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm {
                    zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                        program_counter,
                        vs2,
                        group_regs,
                    )?;
                    zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                        program_counter,
                        vd,
                        VReg::V0,
                        group_regs,
                    )?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // Sign-extend imm to u64 so the low sew_bytes are correct for all SEW.
                let scalar = i64::from(simm5).cast_unsigned();
                // SAFETY: alignment and overlap verified above; vl <= VLMAX.
                unsafe {
                    zve64x_perm_helpers::execute_merge_scalar(
                        ext_state, vd, vs2, vm, vl, vstart, sew, scalar,
                    );
                }
            }
            // vcompress.vm vd, vs2, vs1
            // Packs active elements of vs2 (where vs1 mask bit is set) sequentially into vd.
            // Always unmasked (vm=1 in encoding); vs1 is the explicit mask operand.
            // vd must not overlap vs1 or vs2.
            Self::VcompressVm { vd, vs2, vs1 } => {
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
                // Spec §16.5: vstart must be zero.
                if ext_state.vstart() != 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_perm_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // vs1 is always a single mask register (no LMUL grouping)
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                // vs1 is a mask register; check it doesn't overlap vd
                zve64x_perm_helpers::check_no_overlap::<Reg, _, _, _>(program_counter, vd, vs1, 1)?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                unsafe {
                    zve64x_perm_helpers::execute_compress(ext_state, vd, vs2, vs1, vl, sew);
                }
            }
            // vmv1r.v vd, vs2
            // Whole register move: copies 1 register.
            // No masking, no vtype/vl dependency.
            Self::Vmv1rV { vd, vs2 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: both vd.bits() and vs2.bits() are always in [0, 32) by VReg invariant;
                // copying 1 register always fits.
                unsafe {
                    zve64x_perm_helpers::execute_whole_reg_move(
                        ext_state.write_vreg(),
                        vd.bits(),
                        vs2.bits(),
                        1,
                    );
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }
            // vmv2r.v vd, vs2
            // Whole register move: copies 2 registers.
            // vd and vs2 must be aligned to 2 (checked here per spec §17.6).
            Self::Vmv2rV { vd, vs2 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                if !vd.bits().is_multiple_of(2) || !vs2.bits().is_multiple_of(2) {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment verified; 2 registers from aligned base always stay in [0, 32).
                unsafe {
                    zve64x_perm_helpers::execute_whole_reg_move(
                        ext_state.write_vreg(),
                        vd.bits(),
                        vs2.bits(),
                        2,
                    );
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }
            // vmv4r.v vd, vs2
            // Whole register move: copies 4 registers.
            Self::Vmv4rV { vd, vs2 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                if !vd.bits().is_multiple_of(4) || !vs2.bits().is_multiple_of(4) {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment verified; 4 registers from aligned base always stay in [0, 32).
                unsafe {
                    zve64x_perm_helpers::execute_whole_reg_move(
                        ext_state.write_vreg(),
                        vd.bits(),
                        vs2.bits(),
                        4,
                    );
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }
            // vmv8r.v vd, vs2
            // Whole register move: copies 8 registers.
            Self::Vmv8rV { vd, vs2 } => {
                if !ext_state.vector_instructions_allowed() {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                if !vd.bits().is_multiple_of(8) || !vs2.bits().is_multiple_of(8) {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment verified; 8 registers from aligned base always stay in [0, 32).
                unsafe {
                    zve64x_perm_helpers::execute_whole_reg_move(
                        ext_state.write_vreg(),
                        vd.bits(),
                        vs2.bits(),
                        8,
                    );
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
