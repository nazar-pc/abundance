//! Zve64x integer reduction instructions

#[cfg(test)]
mod tests;
pub mod zve64x_reduction_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::arith::zve64x_arith_helpers;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutionError, ProgramCounter, RegisterFile, Rs1Rs2Operands,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xReductionInstruction<Reg>
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
            Self::Vredsum { vd, vs2, vs1, vm } => {
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
                // Spec §14: reductions with vstart > 0 are reserved; raise illegal instruction
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: `vs2` alignment checked; `vstart == 0` checked;
                // `vs1` and `vd` are single-register scalar operands
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc.wrapping_add(elem),
                    );
                }
            }
            Self::Vredand { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc & elem,
                    );
                }
            }
            Self::Vredor { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc | elem,
                    );
                }
            }
            Self::Vredxor { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc ^ elem,
                    );
                }
            }
            Self::Vredminu { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if elem & mask < acc & mask { elem } else { acc }
                        },
                    );
                }
            }
            Self::Vredmin { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, sew| {
                            if zve64x_arith_helpers::sign_extend(elem, sew)
                                < zve64x_arith_helpers::sign_extend(acc, sew)
                            {
                                elem
                            } else {
                                acc
                            }
                        },
                    );
                }
            }
            Self::Vredmaxu { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if elem & mask > acc & mask { elem } else { acc }
                        },
                    );
                }
            }
            Self::Vredmax { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, sew| {
                            if zve64x_arith_helpers::sign_extend(elem, sew)
                                > zve64x_arith_helpers::sign_extend(acc, sew)
                            {
                                elem
                            } else {
                                acc
                            }
                        },
                    );
                }
            }
            Self::Vwredsumu { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // Widening: 2*SEW must fit in ELEN
                if u32::from(vtype.vsew().bits()) * 2 > ExtState::ELEN {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: `vs2` alignment checked; widening SEW constraint checked above;
                // `vstart == 0` checked; `vd` and `vs1` are single-register 2*SEW scalar operands
                unsafe {
                    zve64x_reduction_helpers::execute_widening_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        // Zero-extend vs2 elements then accumulate
                        |acc, elem, _sew| acc.wrapping_add(elem),
                        false,
                    );
                }
            }
            Self::Vwredsum { vd, vs2, vs1, vm } => {
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
                if u32::from(ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if u32::from(vtype.vsew().bits()) * 2 > ExtState::ELEN {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = ext_state.vl();
                // SAFETY: see `Vwredsumu`
                unsafe {
                    zve64x_reduction_helpers::execute_widening_reduce_op(
                        ext_state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        // Sign-extend vs2 elements then accumulate
                        |acc, elem, _sew| acc.wrapping_add(elem),
                        true,
                    );
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
