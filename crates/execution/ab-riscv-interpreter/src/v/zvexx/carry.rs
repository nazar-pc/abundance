//! ZveXx carry/borrow arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zvexx_carry_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::zvexx_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    PackedAddress, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZveXxCarryInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for ZveXxCarryInstruction<Reg> where Reg: Register
{}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxCarryInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory>,
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
        env: &mut Env,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            // vadc: add with carry-in from v0, data result
            Self::VadcVvm { vd, vs2, vs1 } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // vd must not be v0: v0 holds carry-in
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VadcVxm { vd, vs2, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VadcVim { vd, vs2, imm } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            // vmadc: add and write carry-out mask
            Self::VmadcVvm { vd, vs2, vs1 } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmadcVxm { vd, vs2, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmadcVim { vd, vs2, imm } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmadcVv { vd, vs2, vs1 } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<false, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmadcVx { vd, vs2, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<false, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmadcVi { vd, vs2, imm } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_add_mask::<false, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            // vsbc: subtract with borrow-in from v0, data result
            Self::VsbcVvm { vd, vs2, vs1 } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub::<Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VsbcVxm { vd, vs2, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments checked above; vd != v0 checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub::<Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            // vmsbc: subtract and write borrow-out mask
            Self::VmsbcVvm { vd, vs2, vs1 } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmsbcVxm { vd, vs2, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<true, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }

            Self::VmsbcVv { vd, vs2, vs1 } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<false, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Vreg(vs1),
                        sew,
                    );
                }
            }

            Self::VmsbcVx { vd, vs2, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_carry_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_carry_helpers::check_mask_dest_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignments and mask-destination overlap checked above
                unsafe {
                    zvexx_carry_helpers::execute_carry_sub_mask::<false, Reg, _>(
                        env,
                        vd,
                        vs2,
                        zvexx_carry_helpers::OpSrc::Scalar(scalar),
                        sew,
                    );
                }
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
