//! Zvbb extension

#[cfg(test)]
mod tests;
pub mod zvbb_helpers;
pub mod zvkb;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::arith::zvexx_arith_helpers;
use crate::v::zvexx::carry::zvexx_carry_helpers;
use crate::v::zvexx::config::zvexx_config_helpers;
use crate::v::zvexx::fixed_point::zvexx_fixed_point_helpers;
use crate::v::zvexx::load::zvexx_load_helpers;
use crate::v::zvexx::mask::zvexx_mask_helpers;
use crate::v::zvexx::muldiv::zvexx_muldiv_helpers;
use crate::v::zvexx::perm::zvexx_perm_helpers;
use crate::v::zvexx::reduction::zvexx_reduction_helpers;
use crate::v::zvexx::store::zvexx_store_helpers;
use crate::v::zvexx::widen_narrow::zvexx_widen_narrow_helpers;
use crate::v::zvexx::zvexx_helpers;
use crate::zicsr::zicsr_helpers;
use crate::zvbb::zvkb::zvkb_helpers;
use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for ZvbbInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for ZvbbInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for ZvbbInstruction<Reg>
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
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            // vbrev: reverse all bits within each SEW-wide element
            Self::VbrevV { vd, vs2, vm } => {
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
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vbrev::<Reg, _, _>(ext_state, vd, vs2, sew, vm);
                }
            }
            // vclz: count leading zeros within each SEW-wide element; result in [0, SEW]
            Self::VclzV { vd, vs2, vm } => {
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
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vclz::<Reg, _, _>(ext_state, vd, vs2, sew, vm);
                }
            }
            // vctz: count trailing zeros within each SEW-wide element; result in [0, SEW]
            Self::VctzV { vd, vs2, vm } => {
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
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vctz::<Reg, _, _>(ext_state, vd, vs2, sew, vm);
                }
            }
            // vcpop: population count (number of set bits) within each SEW-wide element
            Self::VcpopV { vd, vs2, vm } => {
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
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vcpop::<Reg, _, _>(ext_state, vd, vs2, sew, vm);
                }
            }
            // vwsll: widening shift-left-logical; vd is 2*SEW wide, vs2/src are SEW wide.
            // SEW=E64 is illegal (cannot double); LMUL=M8 is illegal (EMUL(vd)=16 out of range).
            Self::VwsllVv { vd, vs2, vs1, vm } => {
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
                let sew = vtype.vsew();
                let Some(double_sew) = sew.double_width() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                let Some(dest_group_regs) =
                    vtype.vlmul().data_register_count(double_sew.as_eew(), sew)
                else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vwsll::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbb_helpers::OpSrc::Vreg(vs1),
                        sew,
                        double_sew,
                        vm,
                    );
                }
            }
            Self::VwsllVx {
                vm,
                vd,
                vs2,
                rs1: _,
            } => {
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
                let sew = vtype.vsew();
                let Some(double_sew) = sew.double_width() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                let Some(dest_group_regs) =
                    vtype.vlmul().data_register_count(double_sew.as_eew(), sew)
                else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vwsll::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbb_helpers::OpSrc::Scalar(scalar),
                        sew,
                        double_sew,
                        vm,
                    );
                }
            }
            // vwsll.vi: standard 5-bit immediate; vm is the normal mask-control bit
            Self::VwsllVi { vd, vs2, uimm, vm } => {
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
                let sew = vtype.vsew();
                let Some(double_sew) = sew.double_width() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                let Some(dest_group_regs) =
                    vtype.vlmul().data_register_count(double_sew.as_eew(), sew)
                else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvbb_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // SAFETY: alignments checked above
                unsafe {
                    zvbb_helpers::execute_vwsll::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbb_helpers::OpSrc::Scalar(u64::from(uimm)),
                        sew,
                        double_sew,
                        vm,
                    );
                }
            }
        }
        Ok(ControlFlow::Continue(Default::default()))
    }
}
