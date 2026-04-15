//! Opaque helpers for Zve64x extension
use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::arith::zve64x_arith_helpers::{
    read_element_u64, sign_extend, write_element_u64,
};
use crate::v::zve64x::load::zve64x_load_helpers::{mask_bit, snapshot_mask};
use crate::{InterpreterState, ProgramCounter, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Execute a single-width integer reduction.
///
/// # Safety
/// - `vs2.bits() % group_regs == 0` and `vs2.bits() + group_regs <= 32` (verified by caller)
/// - `vstart == 0` (verified by caller; reductions with non-zero vstart are illegal)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_reduce_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // Spec §5.4: when vstart >= vl, no element of vd is updated. For reductions this means
    // vl == 0 (since caller has verified vstart == 0). In that case we must not write vd and
    // must not mark vs dirty.
    if vl == 0 {
        state.ext_state.reset_vstart();
        return;
    }
    // SAFETY: element 0 always fits within register vs1
    let init =
        unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(vs1.bits()), 0, sew) };
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vs2_base = usize::from(vs2.bits());
    let mut acc = init;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2_base % group_regs == 0` and `i < vl <= group_regs * elems_per_reg`
        let elem = unsafe { read_element_u64(state.ext_state.read_vreg(), vs2_base, i, sew) };
        acc = op(acc, elem, sew);
    }
    // SAFETY: element 0 always fits within register vd
    unsafe {
        write_element_u64(state.ext_state.write_vreg(), vd.bits(), 0, sew, acc);
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a widening integer sum reduction.
///
/// # Safety
/// - `vs2.bits() % group_regs == 0` and `vs2.bits() + group_regs <= 32` (verified by caller)
/// - `2 * sew.bits() <= ELEN` (verified by caller)
/// - `vstart == 0` (verified by caller)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widening_reduce_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: u32,
    sew: Vsew,
    op: F,
    sign_extend_src: bool,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let wide_sew = match sew {
        Vsew::E8 => Vsew::E16,
        Vsew::E16 => Vsew::E32,
        Vsew::E32 => Vsew::E64,
        // SAFETY: caller verified `2*SEW <= ELEN`; E64 widening is unreachable here
        Vsew::E64 => unsafe { core::hint::unreachable_unchecked() },
    };
    if vl == 0 {
        state.ext_state.reset_vstart();
        return;
    }
    // SAFETY: element 0 always fits within register vs1
    let init = unsafe {
        read_element_u64(
            state.ext_state.read_vreg(),
            usize::from(vs1.bits()),
            0,
            wide_sew,
        )
    };
    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vs2_base = usize::from(vs2.bits());
    let mut acc = init;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: same bounds argument as `execute_reduce_op`
        let raw = unsafe { read_element_u64(state.ext_state.read_vreg(), vs2_base, i, sew) };
        let elem = if sign_extend_src {
            sign_extend(raw, sew).cast_unsigned()
        } else {
            raw
        };
        acc = op(acc, elem, wide_sew);
    }
    // SAFETY: element 0 always fits within register vd
    unsafe {
        write_element_u64(state.ext_state.write_vreg(), vd.bits(), 0, wide_sew, acc);
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}
