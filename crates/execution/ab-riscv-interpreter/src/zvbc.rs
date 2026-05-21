//! Zvbc extension

#[cfg(test)]
mod tests;
pub mod zvbc_helpers;

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
    ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for ZvbcInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for ZvbcInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for ZvbcInstruction<Reg>
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
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            // vclmul: vd[i] = lower SEW bits of clmul(vs2[i], vs1[i])
            Self::VclmulVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                if !vm && vd == VReg::V0 {
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
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvbc_helpers::execute_vclmul::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbc_helpers::OpSrc::Vreg(vs1),
                        sew,
                        vm,
                    );
                }
            }
            Self::VclmulVx {
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
                if !vm && vd == VReg::V0 {
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
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above
                unsafe {
                    zvbc_helpers::execute_vclmul::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbc_helpers::OpSrc::Scalar(scalar),
                        sew,
                        vm,
                    );
                }
            }
            // vclmulh: vd[i] = upper SEW bits of clmul(vs2[i], vs1[i])
            Self::VclmulhVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                if !vm && vd == VReg::V0 {
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
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments checked above
                unsafe {
                    zvbc_helpers::execute_vclmulh::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbc_helpers::OpSrc::Vreg(vs1),
                        sew,
                        vm,
                    );
                }
            }
            Self::VclmulhVx {
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
                if !vm && vd == VReg::V0 {
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
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvbc_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above
                unsafe {
                    zvbc_helpers::execute_vclmulh::<Reg, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvbc_helpers::OpSrc::Scalar(scalar),
                        sew,
                        vm,
                    );
                }
            }
        }
        Ok(ControlFlow::Continue(Default::default()))
    }
}
