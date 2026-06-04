//! Zve64x integer arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zve64x_arith_helpers;

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
impl<Reg> ExecutableInstructionOperands for Zve64xArithInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xArithInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xArithInstruction<Reg>
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
            // vadd
            Self::VaddVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above; `vl <= VLMAX = group_regs * VLENB / sew_bytes`;
                // masked vd != v0 checked above.
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_add(b),
                    );
                }
            }
            Self::VaddVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above; scalar source has no register constraints
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_add(b),
                    );
                }
            }
            Self::VaddVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // Sign-extend imm to u64 so wrapping_add works correctly for all SEW
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_add(b),
                    );
                }
            }
            // vsub / vrsub
            Self::VsubVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_sub(b),
                    );
                }
            }
            Self::VsubVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_sub(b),
                    );
                }
            }
            Self::VrsubVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // vrsub: result = src - vs2[i]
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| b.wrapping_sub(a),
                    );
                }
            }
            Self::VrsubVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| b.wrapping_sub(a),
                    );
                }
            }
            // vand
            Self::VandVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, _| a & b,
                    );
                }
            }
            Self::VandVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a & b,
                    );
                }
            }
            Self::VandVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a & b,
                    );
                }
            }
            // vor
            Self::VorVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, _| a | b,
                    );
                }
            }
            Self::VorVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a | b,
                    );
                }
            }
            Self::VorVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a | b,
                    );
                }
            }
            // vxor
            Self::VxorVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, _| a ^ b,
                    );
                }
            }
            Self::VxorVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a ^ b,
                    );
                }
            }
            Self::VxorVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a ^ b,
                    );
                }
            }
            // vsll
            Self::VsllVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // Shift amount masked to log2(SEW) bits per spec §12.6
                        |a, b, sew| a << (b & u64::from(sew.bits_width() - 1)),
                    );
                }
            }
            Self::VsllVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| a << (b & u64::from(sew.bits_width() - 1)),
                    );
                }
            }
            Self::VsllVi { vd, vs2, uimm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // Immediate is already unsigned 5-bit; mask to log2(SEW) here too
                let shamt = u64::from(uimm) & u64::from(sew.bits_width() - 1);
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(shamt),
                        vm,
                        sew,
                        |a, b, _| a << b,
                    );
                }
            }
            // vsrl
            Self::VsrlVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // Logical right shift; operate on the SEW-wide portion only
                        |a, b, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            let shamt = b & u64::from(sew.bits_width() - 1);
                            (a & mask) >> shamt
                        },
                    );
                }
            }
            Self::VsrlVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            let shamt = b & u64::from(sew.bits_width() - 1);
                            (a & mask) >> shamt
                        },
                    );
                }
            }
            Self::VsrlVi { vd, vs2, uimm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let shamt = u64::from(uimm) & u64::from(sew.bits_width() - 1);
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(shamt),
                        vm,
                        sew,
                        |a, b, sew| (a & zve64x_arith_helpers::sew_mask(sew)) >> b,
                    );
                }
            }
            // vsra
            Self::VsraVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            let shamt = b & u64::from(sew.bits_width() - 1);
                            let signed = zve64x_arith_helpers::sign_extend(a, sew);
                            (signed >> shamt).cast_unsigned()
                        },
                    );
                }
            }
            Self::VsraVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let shamt = b & u64::from(sew.bits_width() - 1);
                            let signed = zve64x_arith_helpers::sign_extend(a, sew);
                            (signed >> shamt).cast_unsigned()
                        },
                    );
                }
            }
            Self::VsraVi { vd, vs2, uimm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let shamt = u64::from(uimm) & u64::from(sew.bits_width() - 1);
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(shamt),
                        vm,
                        sew,
                        |a, b, sew| {
                            let signed = zve64x_arith_helpers::sign_extend(a, sew);
                            (signed >> b).cast_unsigned()
                        },
                    );
                }
            }
            // vminu / vmin
            Self::VminuVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if a & mask <= b & mask { a } else { b }
                        },
                    );
                }
            }
            Self::VminuVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if a & mask <= b & mask { a } else { b }
                        },
                    );
                }
            }
            Self::VminVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            if zve64x_arith_helpers::sign_extend(a, sew)
                                <= zve64x_arith_helpers::sign_extend(b, sew)
                            {
                                a
                            } else {
                                b
                            }
                        },
                    );
                }
            }
            Self::VminVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            if zve64x_arith_helpers::sign_extend(a, sew)
                                <= zve64x_arith_helpers::sign_extend(b, sew)
                            {
                                a
                            } else {
                                b
                            }
                        },
                    );
                }
            }
            // vmaxu / vmax
            Self::VmaxuVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if a & mask >= b & mask { a } else { b }
                        },
                    );
                }
            }
            Self::VmaxuVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if a & mask >= b & mask { a } else { b }
                        },
                    );
                }
            }
            Self::VmaxVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            if zve64x_arith_helpers::sign_extend(a, sew)
                                >= zve64x_arith_helpers::sign_extend(b, sew)
                            {
                                a
                            } else {
                                b
                            }
                        },
                    );
                }
            }
            Self::VmaxVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_arith_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            if zve64x_arith_helpers::sign_extend(a, sew)
                                >= zve64x_arith_helpers::sign_extend(b, sew)
                            {
                                a
                            } else {
                                b
                            }
                        },
                    );
                }
            }
            // vmseq
            Self::VmseqVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: `vs2` and `vs1` alignment checked; `vd` is a single mask register,
                // no alignment constraint; `vl <= VLMAX <= VLEN` so all element indices fit
                // within the mask register. Mask-dest overlap rule (§11.8) checked above.
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                == (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmseqVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                == (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmseqVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                == (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            // vmsne
            Self::VmsneVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                != (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmsneVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                != (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmsneVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                != (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            // vmsltu (unsigned <)
            Self::VmsltuVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                < (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmsltuVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                < (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            // vmslt (signed <)
            Self::VmsltVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                < zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
            Self::VmsltVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                < zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
            // vmsleu (unsigned <=)
            Self::VmsleuVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                <= (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmsleuVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                <= (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmsleuVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // Per spec §12.8: for vmsleu.vi, the immediate is sign-extended to XLEN
                // then the comparison is unsigned. A negative i8 immediate sign-extends to
                // a large u64 (e.g. -1 -> 0xFFFF...FF). Both operands are masked to SEW
                // before comparing, so the effective immediate is (0xFFFF...FF &
                // zve64x_arith_helpers::sew_mask), which equals
                // zve64x_arith_helpers::sew_mask (the maximum SEW-wide unsigned value). This means
                // vs2[i] <= imm is always true for SEW < XLEN when imm < 0.
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                <= (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            // vmsle (signed <=)
            Self::VmsleVv { vd, vs2, vs1, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                <= zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
            Self::VmsleVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                <= zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
            Self::VmsleVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                <= zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
            // vmsgtu (unsigned >): no vv form; vx and vi only
            Self::VmsgtuVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                > (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            Self::VmsgtuVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            (a & zve64x_arith_helpers::sew_mask(sew))
                                > (b & zve64x_arith_helpers::sew_mask(sew))
                        },
                    );
                }
            }
            // vmsgt (signed >): no vv form; vx and vi only
            Self::VmsgtVx {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                > zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
            Self::VmsgtVi { vd, vs2, imm, vm } => {
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
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_arith_helpers::check_mask_dest_no_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: see `VmseqVv`
                unsafe {
                    zve64x_arith_helpers::execute_compare_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_arith_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            zve64x_arith_helpers::sign_extend(a, sew)
                                > zve64x_arith_helpers::sign_extend(b, sew)
                        },
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
