//! Opaque helpers for ZveXx extension
use crate::v::vector_registers::{VRegGroup, VectorRegistersExt};
use crate::v::zvexx::arith::zvexx_arith_helpers::sign_extend;
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::WideningSew;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Execute a single-width integer reduction.
///
/// `vd` and `vs1` are single registers regardless of `LMUL`, element 0 of which holds the scalar
/// operand and result.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_reduce_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VRegGroup<Env::Hart>,
    vs1: VReg,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // Spec §5.4: when vstart >= vl, no element of vd is updated. For reductions this means
    // vl == 0 (since caller has verified vstart == 0). In that case we must not write vd and
    // must not mark vs dirty.
    let vl = vs2.vl();
    if vl.get() == Vl::ZERO {
        cold_path();
        return;
    }
    let init = env
        .read_vregs()
        .read_first(vs1, sew.as_eew())
        .expect("Element width never exceeds `ELEN`, which never exceeds `VLEN`; qed");
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    let mut acc = init;
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`i < vl` of `vs2`; qed");
        acc = op(acc, elem, sew);
    }
    env.write_vregs()
        .write_first(vd, sew.as_eew(), acc)
        .expect("Element width never exceeds `ELEN`, which never exceeds `VLEN`; qed");
    env.mark_vs_dirty();
}

/// Execute a widening integer sum reduction.
///
/// `vd` and `vs1` are single registers regardless of `LMUL`, element 0 of which holds the scalar
/// operand and result.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_widening_reduce_op<const SIGN_EXTEND_SRC: bool, Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VRegGroup<Env::Hart>,
    vs1: VReg,
    vm: bool,
    sew: WideningSew<Env::Hart>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let wide_sew = sew.wide();
    let sew = sew.narrow();
    let vl = vs2.vl();
    if vl.get() == Vl::ZERO {
        cold_path();
        return;
    }
    let init = env
        .read_vregs()
        .read_first(vs1, wide_sew.as_eew())
        .expect("Element width never exceeds `ELEN`, which never exceeds `VLEN`; qed");
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    let mut acc = init;
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let raw = env
            .read_vregs()
            .read(vs2, i)
            .expect("`i < vl` of `vs2`; qed");
        let elem = if SIGN_EXTEND_SRC {
            sign_extend(raw, sew).cast_unsigned()
        } else {
            raw
        };
        acc = op(acc, elem, wide_sew);
    }
    env.write_vregs()
        .write_first(vd, wide_sew.as_eew(), acc)
        .expect("Element width never exceeds `ELEN`, which never exceeds `VLEN`; qed");
    env.mark_vs_dirty();
}
