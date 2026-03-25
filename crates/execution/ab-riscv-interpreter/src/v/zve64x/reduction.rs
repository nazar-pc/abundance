//! Zve64x integer reduction instructions

#[cfg(test)]
mod tests;
pub mod zve64x_reduction_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::arith::zve64x_arith_helpers;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::instructions::v::zve64x::reduction::Zve64xReductionInstruction;
use ab_riscv_primitives::registers::general_purpose::Register;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Zve64xReductionInstruction<Reg>
where
    Reg: Register,
    [(); Reg::N]:,
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
        state: &mut InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            Self::Vredsum { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: `vs2` alignment checked; `vs1` and `vd` are single-register scalar
                // operands with no LMUL alignment constraint; `vl <= VLMAX`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, elem, _sew| acc.wrapping_add(elem),
                    );
                }
            }
            Self::Vredand { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, elem, _sew| acc & elem,
                    );
                }
            }
            Self::Vredor { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, elem, _sew| acc | elem,
                    );
                }
            }
            Self::Vredxor { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, elem, _sew| acc ^ elem,
                    );
                }
            }
            Self::Vredminu { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, elem, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if elem & mask < acc & mask { elem } else { acc }
                        },
                    );
                }
            }
            Self::Vredmin { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
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
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        |acc, elem, sew| {
                            let mask = zve64x_arith_helpers::sew_mask(sew);
                            if elem & mask > acc & mask { elem } else { acc }
                        },
                    );
                }
            }
            Self::Vredmax { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vredsum`
                unsafe {
                    zve64x_reduction_helpers::execute_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
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
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // Widening: 2*SEW must fit in ELEN
                if u32::from(vtype.vsew().bits()) * 2 > ExtState::ELEN {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: `vs2` alignment checked; widening SEW constraint checked above;
                // `vd` and `vs1` are single-register 2*SEW scalar operands
                unsafe {
                    zve64x_reduction_helpers::execute_widening_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        // Zero-extend vs2 elements then accumulate
                        |acc, elem, _sew| acc.wrapping_add(elem),
                        false,
                    );
                }
            }
            Self::Vwredsum { vd, vs2, vs1, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // Widening: 2*SEW must fit in ELEN
                if u32::from(vtype.vsew().bits()) * 2 > ExtState::ELEN {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                zve64x_arith_helpers::check_vreg_group_alignment(state, vs2, group_regs)?;
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: see `Vwredsumu`
                unsafe {
                    zve64x_reduction_helpers::execute_widening_reduce_op(
                        state,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        vstart,
                        sew,
                        // Sign-extend vs2 elements then accumulate
                        |acc, elem, _sew| acc.wrapping_add(elem),
                        true,
                    );
                }
            }
            Self::PhantomZve64xReduction(_) => unreachable!("Never constructed"),
        }

        Ok(ControlFlow::Continue(()))
    }
}
