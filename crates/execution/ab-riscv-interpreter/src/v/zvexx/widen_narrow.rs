//! ZveXx widening, narrowing, and extension instructions

#[cfg(test)]
mod tests;
pub mod zvexx_widen_narrow_helpers;

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
impl<Reg> ExecutableInstructionOperands for ZveXxWidenNarrowInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for ZveXxWidenNarrowInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for ZveXxWidenNarrowInstruction<Reg>
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
            // vwaddu.vv - 2*SEW = zext(SEW) + zext(SEW)
            Self::VwadduVv { vd, vs2, vs1, vm } => {
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
                // Widening requires SEW < 64; 2*SEW must fit in ELEN=64
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwaddu.vx - 2*SEW = zext(SEW) + zext(xlen->SEW)
            Self::VwadduVx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // Scalar is zero-extended to 2*SEW; the low SEW bits are what matter
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwadd.vv - 2*SEW = sext(SEW) + sext(SEW)
            Self::VwaddVv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwadd.vx - 2*SEW = sext(SEW) + sext(rs1)
            Self::VwaddVx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // Scalar is sign-extended from XLEN to 64 bits
                let scalar =
                    zvexx_widen_narrow_helpers::sign_extend_bits(rs1_value.as_u64(), Reg::XLEN)
                        .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwsubu.vv - 2*SEW = zext(SEW) - zext(SEW)
            Self::VwsubuVv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsubu.vx - 2*SEW = zext(SEW) - zext(rs1)
            Self::VwsubuVx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsub.vv - 2*SEW = sext(SEW) - sext(SEW)
            Self::VwsubVv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsub.vx - 2*SEW = sext(SEW) - sext(rs1)
            Self::VwsubVx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar =
                    zvexx_widen_narrow_helpers::sign_extend_bits(rs1_value.as_u64(), Reg::XLEN)
                        .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwaddu.wv - 2*SEW = 2*SEW + zext(SEW)
            Self::VwadduWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                // vs2 is the wide source; vs1 is narrow
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwaddu.wx - 2*SEW = 2*SEW + zext(rs1)
            Self::VwadduWx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                // For .wx scalar variants vd may alias vs2 (same wide group); no narrow vs1
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwadd.wv - 2*SEW = 2*SEW + sext(SEW)
            Self::VwaddWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwadd.wx - 2*SEW = 2*SEW + sext(rs1)
            Self::VwaddWx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar =
                    zvexx_widen_narrow_helpers::sign_extend_bits(rs1_value.as_u64(), Reg::XLEN)
                        .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_add,
                    );
                }
            }
            // vwsubu.wv - 2*SEW = 2*SEW - zext(SEW)
            Self::VwsubuWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsubu.wx - 2*SEW = 2*SEW - zext(rs1)
            Self::VwsubuWx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsub.wv - 2*SEW = 2*SEW - sext(SEW)
            Self::VwsubWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsub.wx - 2*SEW = 2*SEW - sext(rs1)
            Self::VwsubWx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar =
                    zvexx_widen_narrow_helpers::sign_extend_bits(rs1_value.as_u64(), Reg::XLEN)
                        .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        u64::wrapping_sub,
                    );
                }
            }
            // vnsrl.wv - SEW = (2*SEW) >> SEW (logical)
            Self::VnsrlWv { vd, vs2, vs1, vm } => {
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
                // SEW must be < 64 so that 2*SEW fits in ELEN
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                    );
                }
            }
            // vnsrl.wx - SEW = (2*SEW) >> rs1 (logical)
            Self::VnsrlWx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                    );
                }
            }
            // vnsrl.wi - SEW = (2*SEW) >> uimm (logical)
            Self::VnsrlWi { vd, vs2, uimm, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                        vm,
                        sew,
                    );
                }
            }
            // vnsra.wv - SEW = (2*SEW) >> SEW (arithmetic)
            Self::VnsraWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                    );
                }
            }
            // vnsra.wx - SEW = (2*SEW) >> rs1 (arithmetic)
            Self::VnsraWx {
                vd,
                vs2,
                rs1: _,
                vm,
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                    );
                }
            }
            // vnsra.wi - SEW = (2*SEW) >> uimm (arithmetic)
            Self::VnsraWi { vd, vs2, uimm, vm } => {
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
                if u32::from(sew.bits_width()) * 2 > ExtState::ELEN {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let wide_eew = match sew {
                    Vsew::E8 => Eew::E16,
                    Vsew::E16 => Eew::E32,
                    Vsew::E32 => Eew::E64,
                    Vsew::E64 => unreachable!("SEW=64 already rejected above"),
                };
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                        vm,
                        sew,
                    );
                }
            }
            // vzext.vf2 - zero-extend SEW/2 -> SEW
            Self::VzextVf2 { vd, vs2, vm } => {
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
                // SEW must be >= 2*8 = 16
                if u32::from(sew.bits_width()) < 16 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                // EMUL for source = LMUL / 2; src_group = max(1, group_regs / 2)
                let src_group = group_regs.max(2) / 2;
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<false, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        vm,
                        sew,
                        VsewFactor::F2,
                    );
                }
            }
            // vzext.vf4 - zero-extend SEW/4 -> SEW
            Self::VzextVf4 { vd, vs2, vm } => {
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
                // SEW must be >= 4*8 = 32
                if u32::from(sew.bits_width()) < 32 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(4) / 4;
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<false, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        vm,
                        sew,
                        VsewFactor::F4,
                    );
                }
            }
            // vzext.vf8 - zero-extend SEW/8 -> SEW
            Self::VzextVf8 { vd, vs2, vm } => {
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
                // SEW must be >= 8*8 = 64; only SEW=64 qualifies in Zve64x
                if u32::from(sew.bits_width()) < 64 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(8) / 8;
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<false, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        vm,
                        sew,
                        VsewFactor::F8,
                    );
                }
            }
            // vsext.vf2 - sign-extend SEW/2 -> SEW
            Self::VsextVf2 { vd, vs2, vm } => {
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
                if u32::from(sew.bits_width()) < 16 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(2) / 2;
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<true, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        vm,
                        sew,
                        VsewFactor::F2,
                    );
                }
            }
            // vsext.vf4 - sign-extend SEW/4 -> SEW
            Self::VsextVf4 { vd, vs2, vm } => {
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
                if u32::from(sew.bits_width()) < 32 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(4) / 4;
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<true, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        vm,
                        sew,
                        VsewFactor::F4,
                    );
                }
            }
            // vsext.vf8 - sign-extend SEW/8 -> SEW
            Self::VsextVf8 { vd, vs2, vm } => {
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
                if u32::from(sew.bits_width()) < 64 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(8) / 8;
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<true, _, _, _>(
                        ext_state,
                        vd,
                        vs2,
                        vm,
                        sew,
                        VsewFactor::F8,
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
