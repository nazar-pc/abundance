//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VRegGroup, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{OpSrc, sew_mask, sign_extend};
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::WideningSew;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Execute a single-width element-wise arithmetic operation over `0..vl`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew)` and returns the `u64` result.
/// Only the low `sew.bytes()` of the result are written back.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_arith_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let vl = vd.vl();
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let b = match src.vreg {
            Some(vs1) => env
                .read_vregs()
                .read(vs1, i)
                .expect("`vs1` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let result = op(a, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a single-width widening operation over `0..vl`.
///
/// Reads SEW-wide elements from `vs2` and `src`, computes `op`, and writes a 2*SEW-wide result
/// into `vd`.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_widening_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: WideningSew<Env::Hart>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let sew = sew.narrow();
    let vl = vd.vl();
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let b = match src.vreg {
            Some(vs1) => env
                .read_vregs()
                .read(vs1, i)
                .expect("`vs1` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let result = op(a, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a single-width multiply-add where the first multiplier is a vector register group.
///
/// `op` receives `(acc: u64, a: u64, b: u64, sew: Vsew)` where `acc` is the current `vd[i]`,
/// `a` is the element from `a_reg`, and `b` is the element from `src`. Returns the new `vd[i]`.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_muladd_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    a_reg: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    let vl = vd.vl();
    let (Some(a_reg), Some(src)) = (a_reg.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let acc = env.read_vregs().read(vd, i).expect("`i < vl` of `vd`; qed");
        let a = env
            .read_vregs()
            .read(a_reg, i)
            .expect("`a_reg` has the same `vl` as `vd`, checked above; qed");
        let b = match src.vreg {
            Some(b_reg) => env
                .read_vregs()
                .read(b_reg, i)
                .expect("`b_reg` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let result = op(acc, a, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a single-width multiply-add where the first multiplier is a scalar.
///
/// Analogous to [`execute_muladd_op`] but `a` is a fixed scalar instead of a register element.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_muladd_scalar_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    scalar: u64,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    let vl = vd.vl();
    let Some(src) = src.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let acc = env.read_vregs().read(vd, i).expect("`i < vl` of `vd`; qed");
        let b = match src.vreg {
            Some(b_reg) => env
                .read_vregs()
                .read(b_reg, i)
                .expect("`b_reg` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let result = op(acc, scalar, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a widening multiply-add where the first multiplier is a vector register group.
///
/// Reads SEW-wide `acc` from the widened `vd` group, SEW-wide `a` from `a_reg`, and SEW-wide
/// `b` from `src`. Writes a 2*SEW-wide result back into `vd`.
///
/// `op` receives `(acc: u64, a: u64, b: u64, sew: Vsew)`.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_widening_muladd_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    a_reg: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: WideningSew<Env::Hart>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    let sew = sew.narrow();
    let vl = vd.vl();
    let (Some(a_reg), Some(src)) = (a_reg.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Read the existing 2*SEW accumulator from vd
        let acc = env.read_vregs().read(vd, i).expect("`i < vl` of `vd`; qed");
        let a = env
            .read_vregs()
            .read(a_reg, i)
            .expect("`a_reg` has the same `vl` as `vd`, checked above; qed");
        let b = match src.vreg {
            Some(b_reg) => env
                .read_vregs()
                .read(b_reg, i)
                .expect("`b_reg` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let result = op(acc, a, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a widening multiply-add where the first multiplier is a scalar.
///
/// Analogous to [`execute_widening_muladd_op`] but `a` is a fixed scalar.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_widening_muladd_scalar_op<Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    scalar: u64,
    src: OpSrc<VRegGroup<Env::Hart>>,
    vm: bool,
    sew: WideningSew<Env::Hart>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    let sew = sew.narrow();
    let vl = vd.vl();
    let Some(src) = src.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let acc = env.read_vregs().read(vd, i).expect("`i < vl` of `vd`; qed");
        let b = match src.vreg {
            Some(b_reg) => env
                .read_vregs()
                .read(b_reg, i)
                .expect("`b_reg` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let result = op(acc, scalar, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Signed × signed high half.
///
/// Both operands are sign-extended to i64, multiplied as i128, and the upper SEW bits of the
/// 2*SEW product are returned (zero-extended to u64 for writeback into a SEW-wide element slot).
///
/// Valid for every SEW including 64, because the intermediate product is formed in i128. Whether
/// SEW=64 is reachable at all is an extension-level decision made by the caller (Zve64x excludes
/// it, the full "V" extension does not).
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn mulh_ss(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    let product = sa.widening_mul(sb);
    // Extract bits [2*SEW-1 : SEW] of the product
    let high = (product >> sew.bits_width()).cast_unsigned() as u64;
    high & sew_mask(sew)
}

/// Unsigned × unsigned high half.
///
/// Valid for every SEW including 64; see [`mulh_ss()`] for the extension-level caveat.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn mulhu_uu(a: u64, b: u64, sew: Vsew) -> u64 {
    let ua = a & sew_mask(sew);
    let ub = b & sew_mask(sew);
    let product = ua.widening_mul(ub);
    let high = (product >> sew.bits_width()) as u64;
    high & sew_mask(sew)
}

/// Signed × unsigned high half.
///
/// `a` (vs2) is the signed operand; `b` (vs1/rs1) is the unsigned operand.
///
/// Valid for every SEW including 64; see [`mulh_ss()`] for the extension-level caveat. Note that
/// the unsigned operand is widened to i128 rather than i64, so a full-width unsigned value at
/// SEW=64 keeps its magnitude instead of being reinterpreted as negative.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn mulhsu_su(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = i128::from(sign_extend(a, sew));
    let ub = i128::from(b & sew_mask(sew));
    let product = sa.wrapping_mul(ub);
    let high = (product >> sew.bits_width()).cast_unsigned() as u64;
    high & sew_mask(sew)
}

/// Signed divide with division-by-zero and signed-overflow semantics from the RISC-V V spec §12.11.
///
/// - Division by zero: result = all-ones (i.e., −1 as signed SEW-wide integer)
/// - Signed overflow (MIN / −1): result = MIN (i.e., `1 << (SEW-1)`)
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn sdiv(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Division by zero: return all-ones in the SEW-wide slot (= −1 signed)
    if sb == 0 {
        return sew_mask(sew);
    }
    sa.wrapping_div(sb).cast_unsigned() & sew_mask(sew)
}

/// Signed remainder with division-by-zero and signed-overflow semantics from the RISC-V V spec
/// §12.11.
///
/// - Division by zero: remainder = dividend
/// - Signed overflow (MIN % −1): remainder = 0
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn srem(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Division by zero: remainder = dividend
    if sb == 0 {
        return a & sew_mask(sew);
    }
    sa.wrapping_rem(sb).cast_unsigned() & sew_mask(sew)
}
