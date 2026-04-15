//! Zve64x mask instructions

#[cfg(test)]
mod tests;
pub mod zve64x_mask_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Reg, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Zve64xMaskInstruction<Reg>
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
            // Mask-register logical instructions (§16.1).
            // These operate on the full VLENB bytes regardless of vtype/vl, but still require
            // vtype to be valid (vill=0). Any vector instruction must be rejected when vill is
            // set, regardless of whether it uses SEW or vl.
            Self::Vmandn { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: all VReg values are valid indices < 32; snapshot-before-write
                // inside the helper means vd may overlap vs2 or vs1 safely.
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| {
                        a & !b
                    });
                }
            }
            Self::Vmand { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| a & b);
                }
            }
            Self::Vmor { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| a | b);
                }
            }
            Self::Vmxor { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| a ^ b);
                }
            }
            Self::Vmorn { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| {
                        a | !b
                    });
                }
            }
            Self::Vmnand { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| {
                        !(a & b)
                    });
                }
            }
            Self::Vmnor { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| {
                        !(a | b)
                    });
                }
            }
            Self::Vmxnor { vd, vs2, vs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // SAFETY: see `Vmandn`
                unsafe {
                    zve64x_mask_helpers::execute_mask_logical_op(state, vd, vs2, vs1, |a, b| {
                        !(a ^ b)
                    });
                }
            }
            // vcpop.m (§16.2): count set bits in vs2 over active elements, write to GPR rd.
            Self::Vcpop { rd, vs2, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // vcpop/vfirst require a valid vtype to know vl, but do not use SEW.
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`; `vstart <= vl`
                // by spec invariant.
                unsafe {
                    zve64x_mask_helpers::execute_vcpop(state, rd, vs2, vm, vl, vstart);
                }
            }
            // vfirst.m (§16.3): find lowest-numbered active set bit in vs2, write index to rd.
            Self::Vfirst { rd, vs2, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: same as `Vcpop`
                unsafe {
                    zve64x_mask_helpers::execute_vfirst(state, rd, vs2, vm, vl, vstart);
                }
            }
            // vmsbf.m (§16.4): set-before-first mask bit.
            // Constraints: vd != vs2 (overlap illegal), vm=false implies vd != v0.
            Self::Vmsbf { vd, vs2, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // Spec §16.4: vmsbf/vmsif/vmsof with vstart != 0 raise an illegal instruction
                // exception.
                if state.ext_state.vstart() != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // Per spec §16.4: vd must not overlap vs2
                if vd.bits() == vs2.bits() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vl = state.ext_state.vl();
                // SAFETY: `vd != vs2` checked above; `vd != v0` when masked checked above;
                // `vstart == 0` checked above; `vl <= VLEN` so `vl.div_ceil(8) <= VLENB`.
                unsafe {
                    zve64x_mask_helpers::execute_vmsbf(state, vd, vs2, vm, vl);
                }
            }
            // vmsof.m (§16.5): set-only-first mask bit.
            // Same overlap constraints as vmsbf.
            Self::Vmsof { vd, vs2, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // Spec §16.4: vmsbf/vmsif/vmsof with vstart != 0 raise an illegal instruction
                // exception.
                if state.ext_state.vstart() != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if vd.bits() == vs2.bits() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vl = state.ext_state.vl();
                // SAFETY: see `Vmsbf`
                unsafe {
                    zve64x_mask_helpers::execute_vmsof(state, vd, vs2, vm, vl);
                }
            }
            // vmsif.m (§16.6): set-including-first mask bit.
            // Same overlap constraints as vmsbf.
            Self::Vmsif { vd, vs2, vm } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // Spec §16.4: vmsbf/vmsif/vmsof with vstart != 0 raise an illegal instruction
                // exception.
                if state.ext_state.vstart() != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if vd.bits() == vs2.bits() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vl = state.ext_state.vl();
                // SAFETY: see `Vmsbf`
                unsafe {
                    zve64x_mask_helpers::execute_vmsif(state, vd, vs2, vm, vl);
                }
            }
            // viota.m (§16.8): write prefix popcount of vs2 bits as SEW-wide elements into vd.
            // Constraints: vd must not overlap vs2 or v0 (when masked); vd alignment per LMUL;
            // vstart must be zero (mandatory trap per spec §16.8); SEW must be wide enough
            // to represent VLMAX-1.
            Self::Viota { vd, vs2, vm } => {
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
                // Spec §16.8: viota.m with vstart != 0 raises an illegal instruction exception.
                if u32::from(state.ext_state.vstart()) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let group_regs = vtype.vlmul().register_count();
                let vd_idx = vd.bits();
                if !vd_idx.is_multiple_of(group_regs) || vd_idx + group_regs > 32 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // vd must not overlap vs2; vs2 is always a single mask register (group size 1).
                let vd_start = u32::from(vd.bits());
                let vs2_start = u32::from(vs2.bits());
                if vd_start < vs2_start + 1 && vs2_start < vd_start + u32::from(group_regs) {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let sew_bits = u32::from(sew.bits());
                let vlmax = vtype.vlmul().vlmax(ExtState::VLEN, sew_bits);
                if u64::from(vlmax).unbounded_shr(sew_bits) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vl = state.ext_state.vl();
                // SAFETY: vd alignment checked above; vd group does not overlap vs2 checked above;
                // `vm=false` implies `vd != v0` checked above; vstart == 0 checked above;
                // SEW wide enough to hold VLMAX-1 checked above;
                // `vl <= VLMAX = group_regs * VLENB / sew_bytes`, all element indices valid.
                unsafe {
                    zve64x_mask_helpers::execute_viota(state, vd, vs2, vm, vl, sew);
                }
            }
            // vid.v (§16.9): write element index i as SEW-wide integer into vd[i].
            // Constraints: vm=false implies vd != v0; vd alignment per LMUL.
            Self::Vid { vd, vm } => {
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
                let vd_idx = vd.bits();
                if !vd_idx.is_multiple_of(group_regs) || vd_idx + group_regs > 32 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && vd.bits() == 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let sew = vtype.vsew();
                let vl = state.ext_state.vl();
                let vstart = u32::from(state.ext_state.vstart());
                // SAFETY: vd alignment checked above; `vm=false` implies `vd != v0` checked above;
                // `vl <= VLMAX = group_regs * VLENB / sew_bytes`, all element indices valid.
                unsafe {
                    zve64x_mask_helpers::execute_vid(state, vd, vm, vl, vstart, sew);
                }
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
