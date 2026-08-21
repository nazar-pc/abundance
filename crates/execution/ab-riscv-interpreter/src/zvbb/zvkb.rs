//! Zvkb extension

#[cfg(test)]
mod tests;
pub mod zvkb_helpers;

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
use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, ExecutionResult, FetchInstructionResult, InstructionFetcher,
    OpaqueThreadedExecutionResult, PackedAddress, ProgramCounter, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction, ThreadedExecutionResult,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZvkbInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for ZvkbInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler> for ZvkbInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ ExtState::ELEN }, { ExtState::VLEN }>]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            // vandn: vd[i] = ~vs1[i] & vs2[i]  (or ~rs1 & vs2[i])
            Self::VandnVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vandn::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Vreg(vs1),
                        sew,
                        vm,
                    );
                }
            }
            Self::VandnVx {
                vm,
                vd,
                vs2,
                rs1: _,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vandn::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Scalar(scalar),
                        sew,
                        vm,
                    );
                }
            }
            // vbrev8: reverse bits within each byte of each element
            Self::Vbrev8V { vd, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vbrev8::<Reg, _>(ext_state, vd, vs2, sew, vm);
                }
            }
            // vrev8: reverse bytes within each element
            Self::Vrev8V { vd, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vrev8::<Reg, _>(ext_state, vd, vs2, sew, vm);
                }
            }
            // vrol: vd[i] = rotate_left(vs2[i], src[i] % SEW)
            Self::VrolVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vrol::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Vreg(vs1),
                        sew,
                        vm,
                    );
                }
            }
            Self::VrolVx {
                vm,
                vd,
                vs2,
                rs1: _,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vrol::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Scalar(scalar),
                        sew,
                        vm,
                    );
                }
            }
            // vror: vd[i] = rotate_right(vs2[i], src[i] % SEW)
            Self::VrorVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vror::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Vreg(vs1),
                        sew,
                        vm,
                    );
                }
            }
            Self::VrorVx {
                vm,
                vd,
                vs2,
                rs1: _,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vror::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Scalar(scalar),
                        sew,
                        vm,
                    );
                }
            }
            // vror.vi: 5-bit immediate in vs1[19:15]; bit[25] is the standard vm mask-control bit
            Self::VrorVi { vd, vs2, uimm, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvkb_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvkb_helpers::execute_vror::<Reg, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvkb_helpers::OpSrc::Scalar(u64::from(uimm)),
                        sew,
                        vm,
                    );
                }
            }
        }
        ExecutionResult::ContinueNoWrite
    }
}
