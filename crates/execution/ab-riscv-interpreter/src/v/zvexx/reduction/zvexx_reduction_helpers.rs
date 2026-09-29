//! Opaque helpers for ZveXx extension
use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::arith::zvexx_arith_helpers::sign_extend;
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::WideningSew;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Execute a single-width integer reduction.
///
/// # Safety
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32` (verified by caller)
/// - `vstart == 0` (verified by caller; reductions with non-zero vstart are illegal)
/// - `vl <= group_regs * VLEN.bytes() / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_reduce_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: Vl,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // Spec §5.4: when vstart >= vl, no element of vd is updated. For reductions this means
    // vl == 0 (since caller has verified vstart == 0). In that case we must not write vd and
    // must not mark vs dirty.
    if vl == Vl::ZERO {
        cold_path();
        return;
    }
    // SAFETY: element 0 always fits within register vs1
    let init = unsafe { env.read_vregs().read_element(vs1, 0, sew) };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    let mut acc = init;
    for i in Vstart::ZERO.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `i < vl <= group_regs * elems_per_reg`
        let elem = unsafe { env.read_vregs().read_element(vs2, i, sew) };
        acc = op(acc, elem, sew);
    }
    // SAFETY: element 0 always fits within register vd
    unsafe {
        env.write_vregs().write_element(vd, 0, sew, acc);
    }
    env.mark_vs_dirty();
}

/// Execute a widening integer sum reduction.
///
/// # Safety
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32` (verified by caller)
/// - `vstart == 0` (verified by caller)
/// - `vl <= group_regs * VLEN.bytes() / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_widening_reduce_op<const SIGN_EXTEND_SRC: bool, Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: Vl,
    sew: WideningSew<{ Env::ELEN }>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let wide_sew = sew.wide();
    let sew = sew.narrow();
    if vl == Vl::ZERO {
        cold_path();
        return;
    }
    // SAFETY: element 0 always fits within register vs1
    let init = unsafe { env.read_vregs().read_element(vs1, 0, wide_sew) };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    let mut acc = init;
    for i in Vstart::ZERO.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: same bounds argument as `execute_reduce_op`
        let raw = unsafe { env.read_vregs().read_element(vs2, i, sew) };
        let elem = if SIGN_EXTEND_SRC {
            sign_extend(raw, sew).cast_unsigned()
        } else {
            raw
        };
        acc = op(acc, elem, wide_sew);
    }
    // SAFETY: element 0 always fits within register vd
    unsafe {
        env.write_vregs().write_element(vd, 0, wide_sew, acc);
    }
    env.mark_vs_dirty();
}
