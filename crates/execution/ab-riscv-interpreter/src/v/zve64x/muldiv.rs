//! Zve64x multiply and divide instructions

#[cfg(test)]
mod tests;
pub mod zve64x_muldiv_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2OperandValues,
    Rs1Rs2Operands, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xMulDivInstruction<Reg>
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
        _rs1rs2_values: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
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
            // vmul.vv / vmul.vx - signed multiply, low half
            Self::VmulVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, _| a.wrapping_mul(b),
                    );
                }
            }
            Self::VmulVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, _| a.wrapping_mul(b),
                    );
                }
            }
            // vmulh.vv / vmulh.vx - signed×signed multiply, high half; illegal for SEW=64
            Self::VmulhVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // vmulh is not supported for SEW=64 in Zve64x (would need 128-bit result)
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::mulh_ss,
                    );
                }
            }
            Self::VmulhVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::mulh_ss,
                    );
                }
            }
            // vmulhu.vv / vmulhu.vx - unsigned×unsigned multiply, high half; illegal for SEW=64
            Self::VmulhuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::mulhu_uu,
                    );
                }
            }
            Self::VmulhuVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::mulhu_uu,
                    );
                }
            }
            // vmulhsu.vv / vmulhsu.vx - signed×unsigned multiply, high half; illegal for SEW=64
            Self::VmulhsuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vs2 is signed, vs1 is unsigned
                        zve64x_muldiv_helpers::mulhsu_su,
                    );
                }
            }
            Self::VmulhsuVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // scalar from rs1 is the unsigned operand; vs2 elements are signed
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vs2 is signed, scalar (rs1) is unsigned
                        zve64x_muldiv_helpers::mulhsu_su,
                    );
                }
            }
            // vdivu.vv / vdivu.vx - unsigned divide
            Self::VdivuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // Division by zero: quotient = all-ones for the SEW width (spec §12.11)
                        |a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            dividend.checked_div(divisor).unwrap_or(mask)
                        },
                    );
                }
            }
            Self::VdivuVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            dividend.checked_div(divisor).unwrap_or(mask)
                        },
                    );
                }
            }
            // vdiv.vv / vdiv.vx - signed divide
            Self::VdivVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::sdiv,
                    );
                }
            }
            Self::VdivVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::sdiv,
                    );
                }
            }
            // vremu.vv / vremu.vx - unsigned remainder
            Self::VremuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // Division by zero: remainder = dividend (spec §12.11)
                        |a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            if divisor == 0 {
                                dividend
                            } else {
                                dividend % divisor
                            }
                        },
                    );
                }
            }
            Self::VremuVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            if divisor == 0 {
                                dividend
                            } else {
                                dividend % divisor
                            }
                        },
                    );
                }
            }
            // vrem.vv / vrem.vx - signed remainder
            Self::VremVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::srem,
                    );
                }
            }
            Self::VremVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        zve64x_muldiv_helpers::srem,
                    );
                }
            }
            // vwmulu.vv / vwmulu.vx - unsigned widening multiply; illegal for SEW=64
            Self::VwmuluVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // Widening produces 2*SEW result; SEW=64 would require 128-bit output
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                // dest_group_regs encodes EMUL=2*LMUL; None means EMUL>8, which is illegal
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // vd and vs2/vs1 must not overlap
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            (a & mask).wrapping_mul(b & mask)
                        },
                    );
                }
            }
            Self::VwmuluVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            (a & mask).wrapping_mul(b & mask)
                        },
                    );
                }
            }
            // vwmulsu.vv / vwmulsu.vx - signed×unsigned widening multiply; illegal for SEW=64
            Self::VwmulsuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vs2 is signed, vs1 is unsigned; widen both to full u64 before multiply
                        |a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zve64x_muldiv_helpers::sew_mask(sew);
                            sa.cast_unsigned().wrapping_mul(ub)
                        },
                    );
                }
            }
            Self::VwmulsuVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // scalar from rs1 is the unsigned operand; vs2 elements are signed
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zve64x_muldiv_helpers::sew_mask(sew);
                            sa.cast_unsigned().wrapping_mul(ub)
                        },
                    );
                }
            }
            // vwmul.vv / vwmul.vx - signed widening multiply; illegal for SEW=64
            Self::VwmulVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs1.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // Both operands sign-extended; full 2*SEW product fits in u64
                        |a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let sb = zve64x_muldiv_helpers::sign_extend(b, sew);
                            sa.cast_unsigned().wrapping_mul(sb.cast_unsigned())
                        },
                    );
                }
            }
            Self::VwmulVx { vd, vs2, rs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // scalar from rs1 is sign-extended to XLEN; treat as signed SEW-wide
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zve64x_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let sb = zve64x_muldiv_helpers::sign_extend(b, sew);
                            sa.cast_unsigned().wrapping_mul(sb.cast_unsigned())
                        },
                    );
                }
            }
            // vmacc.vv / vmacc.vx - vd = vd + vs1 * vs2
            Self::VmaccVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vmacc: vd[i] = vd[i] + vs1[i] * vs2[i]
                        |acc, a, b, _| acc.wrapping_add(a.wrapping_mul(b)),
                    );
                }
            }
            Self::VmaccVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, a, b, _| acc.wrapping_add(a.wrapping_mul(b)),
                    );
                }
            }
            // vnmsac.vv / vnmsac.vx - vd = vd - vs1 * vs2
            Self::VnmsacVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vnmsac: vd[i] = vd[i] - vs1[i] * vs2[i]
                        |acc, a, b, _| acc.wrapping_sub(a.wrapping_mul(b)),
                    );
                }
            }
            Self::VnmsacVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, a, b, _| acc.wrapping_sub(a.wrapping_mul(b)),
                    );
                }
            }
            // vmadd.vv / vmadd.vx - vd = vs1 * vd + vs2
            Self::VmaddVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vmadd: vd[i] = vs1[i] * vd[i] + vs2[i]; acc=vd, a=vs1, b=vs2
                        |acc, a, b, _| a.wrapping_mul(acc).wrapping_add(b),
                    );
                }
            }
            Self::VmaddVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vmadd: vd[i] = rs1 * vd[i] + vs2[i]
                        |acc, a, b, _| a.wrapping_mul(acc).wrapping_add(b),
                    );
                }
            }
            // vnmsub.vv / vnmsub.vx - vd = -(vs1 * vd) + vs2
            Self::VnmsubVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vnmsub: vd[i] = -(vs1[i] * vd[i]) + vs2[i]; acc=vd, a=vs1, b=vs2
                        |acc, a, b, _| b.wrapping_sub(a.wrapping_mul(acc)),
                    );
                }
            }
            Self::VnmsubVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vnmsub: vd[i] = -(rs1 * vd[i]) + vs2[i]
                        |acc, a, b, _| b.wrapping_sub(a.wrapping_mul(acc)),
                    );
                }
            }
            // vwmaccu.vv / vwmaccu.vx - unsigned widening multiply-add; illegal for SEW=64
            Self::VwmaccuVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                // vd holds the 2*SEW accumulator
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vwmaccu: vd[i] = vd[i] + zext(vs1[i]) * zext(vs2[i])
                        |acc, a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add((a & mask).wrapping_mul(b & mask))
                        },
                    );
                }
            }
            Self::VwmaccuVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, a, b, sew| {
                            let mask = zve64x_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add((a & mask).wrapping_mul(b & mask))
                        },
                    );
                }
            }
            // vwmacc.vv / vwmacc.vx - signed widening multiply-add; illegal for SEW=64
            Self::VwmaccVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vwmacc: vd[i] = vd[i] + sext(vs1[i]) * sext(vs2[i])
                        |acc, a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let sb = zve64x_muldiv_helpers::sign_extend(b, sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(sb.cast_unsigned()))
                        },
                    );
                }
            }
            Self::VwmaccVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let sb = zve64x_muldiv_helpers::sign_extend(b, sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(sb.cast_unsigned()))
                        },
                    );
                }
            }
            // vwmaccsu.vv / vwmaccsu.vx - signed×unsigned widening multiply-add; illegal for SEW=64
            Self::VwmaccsuVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_op(
                        ext_state,
                        vd,
                        vs1.bits(),
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vwmaccsu: vd[i] = vd[i] + sext(vs1[i]) * zext(vs2[i])
                        |acc, a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zve64x_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(ub))
                        },
                    );
                }
            }
            Self::VwmaccsuVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // scalar (rs1) is the signed operand; vs2 elements are unsigned
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vwmaccsu.vx: vd[i] = vd[i] + sext(vs2[i]) * zext(rs1)
                        // Helper passes (acc, scalar_as_a, vs2_as_b, sew): a=rs1 (unsigned), b=vs2
                        // (signed)
                        |acc, a, b, sew| {
                            let ua = a & zve64x_muldiv_helpers::sew_mask(sew);
                            let sb = zve64x_muldiv_helpers::sign_extend(b, sew);
                            acc.wrapping_add(sb.cast_unsigned().wrapping_mul(ua))
                        },
                    );
                }
            }
            // vwmaccus.vx - unsigned×signed widening multiply-add (vx only); illegal for SEW=64
            Self::VwmaccusVx { vd, rs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                if u32::from(vtype.vsew().bits()) == u64::BITS {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zve64x_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                })?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zve64x_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                zve64x_muldiv_helpers::check_no_widening_overlap::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                let vstart = u32::from(ext_state.vstart());
                // scalar (rs1) is the unsigned operand; vs2 elements are signed
                let scalar = regs.read(rs1).as_u64();
                // SAFETY: alignment and overlap checked above; SEW < 64 checked above
                unsafe {
                    zve64x_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zve64x_muldiv_helpers::OpSrc::Vreg(vs2.bits()),
                        vm,
                        vl,
                        vstart,
                        sew,
                        // vwmaccus.vx: vd[i] = vd[i] + sext(rs1) * zext(vs2[i])
                        // Helper passes (acc, scalar_as_a, vs2_as_b, sew): a=rs1 (signed), b=vs2
                        // (unsigned)
                        |acc, a, b, sew| {
                            let sa = zve64x_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zve64x_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(ub))
                        },
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
