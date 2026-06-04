//! Zve64x widening, narrowing, and extension instructions

#[cfg(test)]
mod tests;
pub mod zve64x_widen_narrow_helpers;

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
impl<Reg> ExecutableInstructionOperands for Zve64xWidenNarrowInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xWidenNarrowInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xWidenNarrowInstruction<Reg>
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
            // vwaddu.vv - 2*SEW = zext(SEW) + zext(SEW)
            Self::VwadduVv { vd, vs2, vs1, vm } => {
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
                // Widening requires SEW < 64; 2*SEW must fit in ELEN=64
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                        true,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // Scalar is zero-extended to 2*SEW; the low SEW bits are what matter
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                        true,
                        u64::wrapping_add,
                    );
                }
            }
            // vwadd.vv - 2*SEW = sext(SEW) + sext(SEW)
            Self::VwaddVv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                        false,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // Scalar is sign-extended from XLEN to 64 bits
                let scalar = zve64x_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    u32::from(Reg::XLEN),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                        false,
                        u64::wrapping_add,
                    );
                }
            }
            // vwsubu.vv - 2*SEW = zext(SEW) - zext(SEW)
            Self::VwsubuVv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                        true,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                        true,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsub.vv - 2*SEW = sext(SEW) - sext(SEW)
            Self::VwsubVv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                        false,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = zve64x_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    u32::from(Reg::XLEN),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                        false,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwaddu.wv - 2*SEW = 2*SEW + zext(SEW)
            Self::VwadduWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                // vs2 is the wide source; vs1 is narrow
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                // For .wx scalar variants vd may alias vs2 (same wide group); no narrow vs1
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                        u64::wrapping_add,
                    );
                }
            }
            // vwadd.wv - 2*SEW = 2*SEW + sext(SEW)
            Self::VwaddWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = zve64x_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    u32::from(Reg::XLEN),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                        u64::wrapping_add,
                    );
                }
            }
            // vwsubu.wv - 2*SEW = 2*SEW - zext(SEW)
            Self::VwsubuWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                        u64::wrapping_sub,
                    );
                }
            }
            // vwsub.wv - 2*SEW = 2*SEW - sext(SEW)
            Self::VwsubWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = zve64x_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    u32::from(Reg::XLEN),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_widen_w_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                        u64::wrapping_sub,
                    );
                }
            }
            // vnsrl.wv - SEW = (2*SEW) >> SEW (logical)
            Self::VnsrlWv { vd, vs2, vs1, vm } => {
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
                // SEW must be < 64 so that 2*SEW fits in ELEN
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_narrow_shift(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_narrow_shift(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                    );
                }
            }
            // vnsrl.wi - SEW = (2*SEW) >> uimm (logical)
            Self::VnsrlWi { vd, vs2, uimm, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_narrow_shift(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                        vm,
                        vl,
                        vstart,
                        sew,
                        false,
                    );
                }
            }
            // vnsra.wv - SEW = (2*SEW) >> SEW (arithmetic)
            Self::VnsraWv { vd, vs2, vs1, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_narrow_shift(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_narrow_shift(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                    );
                }
            }
            // vnsra.wi - SEW = (2*SEW) >> uimm (arithmetic)
            Self::VnsraWi { vd, vs2, uimm, vm } => {
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
                if u32::from(sew.bits()) * 2 > ExtState::ELEN {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
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
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_narrow_shift(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                        vm,
                        vl,
                        vstart,
                        sew,
                        true,
                    );
                }
            }
            // vzext.vf2 - zero-extend SEW/2 -> SEW
            Self::VzextVf2 { vd, vs2, vm } => {
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
                // SEW must be >= 2*8 = 16
                if u32::from(sew.bits()) < 16 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                // EMUL for source = LMUL / 2; src_group = max(1, group_regs / 2)
                let src_group = group_regs.max(2) / 2;
                zve64x_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_extension(
                        ext_state, vd, vs2, vm, vl, vstart, sew, 2, false,
                    );
                }
            }
            // vzext.vf4 - zero-extend SEW/4 -> SEW
            Self::VzextVf4 { vd, vs2, vm } => {
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
                // SEW must be >= 4*8 = 32
                if u32::from(sew.bits()) < 32 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(4) / 4;
                zve64x_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_extension(
                        ext_state, vd, vs2, vm, vl, vstart, sew, 4, false,
                    );
                }
            }
            // vzext.vf8 - zero-extend SEW/8 -> SEW
            Self::VzextVf8 { vd, vs2, vm } => {
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
                // SEW must be >= 8*8 = 64; only SEW=64 qualifies in Zve64x
                if u32::from(sew.bits()) < 64 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(8) / 8;
                zve64x_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_extension(
                        ext_state, vd, vs2, vm, vl, vstart, sew, 8, false,
                    );
                }
            }
            // vsext.vf2 - sign-extend SEW/2 -> SEW
            Self::VsextVf2 { vd, vs2, vm } => {
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
                if u32::from(sew.bits()) < 16 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(2) / 2;
                zve64x_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_extension(
                        ext_state, vd, vs2, vm, vl, vstart, sew, 2, true,
                    );
                }
            }
            // vsext.vf4 - sign-extend SEW/4 -> SEW
            Self::VsextVf4 { vd, vs2, vm } => {
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
                if u32::from(sew.bits()) < 32 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(4) / 4;
                zve64x_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_extension(
                        ext_state, vd, vs2, vm, vl, vstart, sew, 4, true,
                    );
                }
            }
            // vsext.vf8 - sign-extend SEW/8 -> SEW
            Self::VsextVf8 { vd, vs2, vm } => {
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
                if u32::from(sew.bits()) < 64 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let src_group = group_regs.max(8) / 8;
                zve64x_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    src_group,
                    vd,
                    group_regs,
                )?;
                zve64x_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zve64x_widen_narrow_helpers::execute_extension(
                        ext_state, vd, vs2, vm, vl, vstart, sew, 8, true,
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
