//! Zve64x fixed-point arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zve64x_fixed_point_helpers;

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
impl<Reg> ExecutableInstructionOperands for Zve64xFixedPointInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xFixedPointInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xFixedPointInstruction<Reg>
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
            // vsaddu.vv / vsaddu.vx / vsaddu.vi - saturating unsigned add
            Self::VsadduVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_addu(a, b, sew, vxsat)
                        },
                    );
                }
            }
            Self::VsadduVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_addu(a, b, sew, vxsat)
                        },
                    );
                }
            }
            Self::VsadduVi { vd, vs2, imm, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // Per v-spec §12.1 / §11.1: the 5-bit immediate is sign-extended to SEW,
                // then interpreted as an unsigned SEW-wide value for the saturating add.
                // Sign-extend i8 -> i64 -> bit-cast to u64; sat_addu masks to SEW internally.
                let scalar = i64::from(imm).cast_unsigned();
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_addu(a, b, sew, vxsat)
                        },
                    );
                }
            }
            // vsadd.vv / vsadd.vx / vsadd.vi - saturating signed add
            Self::VsaddVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_add(a, b, sew, vxsat)
                        },
                    );
                }
            }
            Self::VsaddVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_add(a, b, sew, vxsat)
                        },
                    );
                }
            }
            Self::VsaddVi { vd, vs2, imm, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // Sign-extend 5-bit immediate for signed sat add
                let scalar = i64::from(imm).cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_add(a, b, sew, vxsat)
                        },
                    );
                }
            }
            // vssubu.vv / vssubu.vx - saturating unsigned subtract
            Self::VssubuVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_subu(a, b, sew, vxsat)
                        },
                    );
                }
            }
            Self::VssubuVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_subu(a, b, sew, vxsat)
                        },
                    );
                }
            }
            // vssub.vv / vssub.vx - saturating signed subtract
            Self::VssubVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_sub(a, b, sew, vxsat)
                        },
                    );
                }
            }
            Self::VssubVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, _vxrm, vxsat| {
                            zve64x_fixed_point_helpers::sat_sub(a, b, sew, vxsat)
                        },
                    );
                }
            }
            // vaaddu.vv / vaaddu.vx - averaging unsigned add
            Self::VaadduVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_addu(a, b, sew, vxrm)
                        },
                    );
                }
            }
            Self::VaadduVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_addu(a, b, sew, vxrm)
                        },
                    );
                }
            }
            // vaadd.vv / vaadd.vx - averaging signed add
            Self::VaaddVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_add(a, b, sew, vxrm)
                        },
                    );
                }
            }
            Self::VaaddVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_add(a, b, sew, vxrm)
                        },
                    );
                }
            }
            // vasubu.vv / vasubu.vx - averaging unsigned subtract
            Self::VasubuVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_subu(a, b, sew, vxrm)
                        },
                    );
                }
            }
            Self::VasubuVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_subu(a, b, sew, vxrm)
                        },
                    );
                }
            }
            // vasub.vv / vasub.vx - averaging signed subtract
            Self::VasubVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_sub(a, b, sew, vxrm)
                        },
                    );
                }
            }
            Self::VasubVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            zve64x_fixed_point_helpers::avg_sub(a, b, sew, vxrm)
                        },
                    );
                }
            }
            // vsmul.vv / vsmul.vx - fractional multiply with rounding and saturation
            Self::VsmulVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::smul(a, b, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            Self::VsmulVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::smul(a, b, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            // vssrl.vv / vssrl.vx / vssrl.vi - scaling shift right logical
            Self::VssrlVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            // Shift amount masked to log2(SEW) bits per spec §12.7
                            let shamt = (b & u64::from(sew.bits() - 1)) as u32;
                            let masked_a = a & zve64x_fixed_point_helpers::sew_mask(sew);
                            zve64x_fixed_point_helpers::rounded_srl(masked_a, shamt, vxrm)
                                & zve64x_fixed_point_helpers::sew_mask(sew)
                        },
                    );
                }
            }
            Self::VssrlVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            let shamt = (b & u64::from(sew.bits() - 1)) as u32;
                            let masked_a = a & zve64x_fixed_point_helpers::sew_mask(sew);
                            zve64x_fixed_point_helpers::rounded_srl(masked_a, shamt, vxrm)
                                & zve64x_fixed_point_helpers::sew_mask(sew)
                        },
                    );
                }
            }
            Self::VssrlVi { vd, vs2, imm, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // Immediate is unsigned 5-bit; mask to log2(SEW) here too
                let shamt = (u64::from(imm) & u64::from(sew.bits() - 1)) as u32;
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(u64::from(shamt)),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            let shamt = b as u32;
                            let masked_a = a & zve64x_fixed_point_helpers::sew_mask(sew);
                            zve64x_fixed_point_helpers::rounded_srl(masked_a, shamt, vxrm)
                                & zve64x_fixed_point_helpers::sew_mask(sew)
                        },
                    );
                }
            }
            // vssra.vv / vssra.vx / vssra.vi - scaling shift right arithmetic
            Self::VssraVv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            let shamt = (b & u64::from(sew.bits() - 1)) as u32;
                            zve64x_fixed_point_helpers::rounded_sra(a, shamt, vxrm, sew)
                                & zve64x_fixed_point_helpers::sew_mask(sew)
                        },
                    );
                }
            }
            Self::VssraVx {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            let shamt = (b & u64::from(sew.bits() - 1)) as u32;
                            zve64x_fixed_point_helpers::rounded_sra(a, shamt, vxrm, sew)
                                & zve64x_fixed_point_helpers::sew_mask(sew)
                        },
                    );
                }
            }
            Self::VssraVi { vd, vs2, imm, vm } => {
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
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
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
                let shamt = (u64::from(imm) & u64::from(sew.bits() - 1)) as u32;
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_fixed_point_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(u64::from(shamt)),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew, vxrm, _vxsat| {
                            let shamt = b as u32;
                            zve64x_fixed_point_helpers::rounded_sra(a, shamt, vxrm, sew)
                                & zve64x_fixed_point_helpers::sew_mask(sew)
                        },
                    );
                }
            }
            // vnclipu.wv / vnclipu.wx / vnclipu.wi - narrowing unsigned clip
            Self::VnclipuWv { vd, vs2, vs1, vm } => {
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
                // Destination SEW must be <= 32 so that 2*SEW fits in 64 bits
                let sew = vtype.vsew();
                zve64x_fixed_point_helpers::check_narrowing_sew::<Reg, _, _, _>(
                    program_counter,
                    sew,
                )?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // vs2 holds 2*SEW elements; its register group is double-width
                zve64x_fixed_point_helpers::check_vs2_narrowing_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // vs1 is a normal SEW-wide source for the shift amount
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: sew <= 32 checked; alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_narrowing_clip_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |wide, shamt, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::nclipu(wide, shamt, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            Self::VnclipuWx {
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
                let sew = vtype.vsew();
                zve64x_fixed_point_helpers::check_narrowing_sew::<Reg, _, _, _>(
                    program_counter,
                    sew,
                )?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs2_narrowing_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: sew <= 32 checked; alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_narrowing_clip_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |wide, shamt, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::nclipu(wide, shamt, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            Self::VnclipuWi { vd, vs2, imm, vm } => {
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
                zve64x_fixed_point_helpers::check_narrowing_sew::<Reg, _, _, _>(
                    program_counter,
                    sew,
                )?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs2_narrowing_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: sew <= 32 checked; alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_narrowing_clip_op(
                        ext_state,
                        vd,
                        vs2,
                        // Immediate is the shift amount directly; masking done inside the helper
                        zve64x_fixed_point_helpers::OpSrc::Scalar(u64::from(imm)),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |wide, shamt, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::nclipu(wide, shamt, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            // vnclip.wv / vnclip.wx / vnclip.wi - narrowing signed clip
            Self::VnclipWv { vd, vs2, vs1, vm } => {
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
                zve64x_fixed_point_helpers::check_narrowing_sew::<Reg, _, _, _>(
                    program_counter,
                    sew,
                )?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs2_narrowing_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: sew <= 32 checked; alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_narrowing_clip_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |wide, shamt, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::nclip(wide, shamt, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            Self::VnclipWx {
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
                let sew = vtype.vsew();
                zve64x_fixed_point_helpers::check_narrowing_sew::<Reg, _, _, _>(
                    program_counter,
                    sew,
                )?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs2_narrowing_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: sew <= 32 checked; alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_narrowing_clip_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |wide, shamt, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::nclip(wide, shamt, sew, vxrm, vxsat)
                        },
                    );
                }
            }
            Self::VnclipWi { vd, vs2, imm, vm } => {
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
                zve64x_fixed_point_helpers::check_narrowing_sew::<Reg, _, _, _>(
                    program_counter,
                    sew,
                )?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_fixed_point_helpers::check_vd::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_fixed_point_helpers::check_vs2_narrowing_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: sew <= 32 checked; alignment checked above
                unsafe {
                    zve64x_fixed_point_helpers::execute_narrowing_clip_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_fixed_point_helpers::OpSrc::Scalar(u64::from(imm)),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |wide, shamt, sew, vxrm, vxsat| {
                            zve64x_fixed_point_helpers::nclip(wide, shamt, sew, vxrm, vxsat)
                        },
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
