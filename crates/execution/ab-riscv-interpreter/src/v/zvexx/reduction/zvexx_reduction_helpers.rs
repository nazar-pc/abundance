//! Opaque helpers for ZveXx extension
use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::arith::zvexx_arith_helpers::{
    read_element_u64, sign_extend, write_element_u64,
};
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::hint::cold_path;

/// Execute a single-width integer reduction.
///
/// # Safety
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32` (verified by caller)
/// - `vstart == 0` (verified by caller; reductions with non-zero vstart are illegal)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_reduce_op<Reg, ExtState, CustomError, F>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // Spec §5.4: when vstart >= vl, no element of vd is updated. For reductions this means
    // vl == 0 (since caller has verified vstart == 0). In that case we must not write vd and
    // must not mark vs dirty.
    if vl == 0 {
        cold_path();
        ext_state.reset_vstart();
        return;
    }
    // SAFETY: element 0 always fits within register vs1
    let init = unsafe { read_element_u64(ext_state.read_vregs(), vs1, 0, sew) };
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    let mut acc = init;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `i < vl <= group_regs * elems_per_reg`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        acc = op(acc, elem, sew);
    }
    // SAFETY: element 0 always fits within register vd
    unsafe {
        write_element_u64(ext_state.write_vregs(), vd, 0, sew, acc);
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a widening integer sum reduction.
///
/// # Safety
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32` (verified by caller)
/// - `sew.double_width().is_some()` (verified by caller)
/// - `vstart == 0` (verified by caller)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widening_reduce_op<
    const SIGN_EXTEND_SRC: bool,
    Reg,
    ExtState,
    CustomError,
    F,
>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let Some(wide_sew) = sew.double_width() else {
        // SAFETY: caller verified `2*SEW <= ELEN`; E64 widening is unreachable here
        unsafe { core::hint::unreachable_unchecked() }
    };
    if vl == 0 {
        cold_path();
        ext_state.reset_vstart();
        return;
    }
    // SAFETY: element 0 always fits within register vs1
    let init = unsafe { read_element_u64(ext_state.read_vregs(), vs1, 0, wide_sew) };
    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    let mut acc = init;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: same bounds argument as `execute_reduce_op`
        let raw = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let elem = if SIGN_EXTEND_SRC {
            sign_extend(raw, sew).cast_unsigned()
        } else {
            raw
        };
        acc = op(acc, elem, wide_sew);
    }
    // SAFETY: element 0 always fits within register vd
    unsafe {
        write_element_u64(ext_state.write_vregs(), vd, 0, wide_sew, acc);
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
