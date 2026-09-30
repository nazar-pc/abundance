//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VRegGroup, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::OpSrc;
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::{ExtensionSew, WideningSew};
use ab_riscv_primitives::instructions::v::Vsew;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Sign-extend the low `sew.bits_width()` of `val` to `i64`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn sign_extend_bits(val: u64, sew: Vsew) -> i64 {
    let shift = u64::BITS - u32::from(sew.bits_width());
    (val.cast_signed() << shift) >> shift
}

/// Interpret a scalar operand as an unsigned SEW-wide value.
///
/// RVV widening scalar instructions (.vx/.wx) conceptually use a scalar
/// operand whose width matches the current SEW, not the full XLEN width.
///
/// For example on RV64:
///
/// SEW=8:
///     val = 0x0000_0000_0000_01ff
///     result = 0x0000_0000_0000_00ff
///
/// SEW=16:
///     val = 0x0000_0000_0000_01ff
///     result = 0x0000_0000_0000_01ff
///
/// SEW=32:
///     val = 0xffff_ffff_1234_5678
///     result = 0x0000_0000_1234_5678
///
/// This helper performs that SEW-width truncation without sign extension.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn scalar_unsigned_for_sew(val: u64, sew: Vsew) -> u64 {
    val & (u64::MAX >> (u64::BITS - u32::from(sew.bits_width())))
}

/// Interpret a scalar operand as a signed SEW-wide value.
///
/// The scalar is first truncated to SEW bits, then sign-extended back to
/// 64 bits.
///
/// For example on RV64:
///
/// SEW=8:
///     val = 0x0000_0000_0000_00ff
///     result = 0xffff_ffff_ffff_ffff (-1)
///
/// SEW=8:
///     val = 0x0000_0000_0000_007f
///     result = 0x0000_0000_0000_007f (+127)
///
/// SEW=16:
///     val = 0x0000_0000_0000_ffff
///     result = 0xffff_ffff_ffff_ffff (-1)
///
/// This matches the signed widening behavior required by instructions such
/// as vwadd.vx and vwsub.vx.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn scalar_signed_for_sew(val: u64, sew: Vsew) -> u64 {
    sign_extend_bits(val, sew).cast_unsigned()
}

/// Execute a widening integer add/subtract.
///
/// Each source element is SEW-wide; the destination element is 2×SEW-wide.
/// `ZERO_EXTEND_AB` selects unsigned or signed widening for sources (unsigned = zero-extend,
/// signed = sign-extend).
///
/// `op` receives `(wide_a: u64, wide_b: u64) -> u64`.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_widen_op<const ZERO_EXTEND_AB: bool, Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    vm: bool,
    sew: WideningSew<{ Env::ELEN }>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64) -> u64,
{
    let vl = vd.vl();
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let sew = sew.narrow();

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let raw_a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let wide_a = if ZERO_EXTEND_AB {
            raw_a
        } else {
            sign_extend_bits(raw_a, sew).cast_unsigned()
        };
        let wide_b = match src.vreg {
            Some(vs1) => {
                let raw_b = env
                    .read_vregs()
                    .read(vs1, i)
                    .expect("`vs1` has the same `vl` as `vd`, checked above; qed");
                if ZERO_EXTEND_AB {
                    raw_b
                } else {
                    sign_extend_bits(raw_b, sew).cast_unsigned()
                }
            }
            None => {
                if ZERO_EXTEND_AB {
                    scalar_unsigned_for_sew(src.scalar, sew)
                } else {
                    scalar_signed_for_sew(src.scalar, sew)
                }
            }
        };
        let result = op(wide_a, wide_b);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a widening add/subtract where `vs2` is already 2×SEW wide.
///
/// `vs2` is read at `wide_sew.bytes_width()`; `src` (narrow) is read at `sew.bytes_width()` and
/// widened. `ZERO_EXTEND_B` selects unsigned vs signed widening for the narrow source operand.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_widen_w_op<const ZERO_EXTEND_B: bool, Reg, Env, F>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    vm: bool,
    sew: WideningSew<{ Env::ELEN }>,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64) -> u64,
{
    let vl = vd.vl();
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let sew = sew.narrow();

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // vs2 is already 2×SEW; read at wide width
        let wide_a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let wide_b = match src.vreg {
            Some(vs1) => {
                let raw_b = env
                    .read_vregs()
                    .read(vs1, i)
                    .expect("`vs1` has the same `vl` as `vd`, checked above; qed");
                if ZERO_EXTEND_B {
                    raw_b
                } else {
                    sign_extend_bits(raw_b, sew).cast_unsigned()
                }
            }
            None => {
                if ZERO_EXTEND_B {
                    scalar_unsigned_for_sew(src.scalar, sew)
                } else {
                    scalar_signed_for_sew(src.scalar, sew)
                }
            }
        };
        let result = op(wide_a, wide_b);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a narrowing right-shift.
///
/// `vs2` is 2×SEW wide; the shift amount comes from `src` (SEW-wide or scalar).
/// The shift amount is masked to `log2(2*SEW)` bits per spec §12.6.
/// `ARITHMETIC` selects sign-extending (true) vs zero-extending (false) before shifting.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_narrow_shift<const ARITHMETIC: bool, Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    vm: bool,
    sew: WideningSew<{ Env::ELEN }>,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
{
    let vl = vd.vl();
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    let wide_sew = sew.wide();
    let sew = sew.narrow();
    // Shift amount mask: log2(2*SEW) bits = log2(SEW) + 1 bits
    let shamt_mask = u64::from(wide_sew.bits_width() - 1);

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let wide_val = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let shamt = match src.vreg {
            Some(vs1) => {
                let raw = env
                    .read_vregs()
                    .read(vs1, i)
                    .expect("`vs1` has the same `vl` as `vd`, checked above; qed");
                raw & shamt_mask
            }
            // Scalar shift amount: only the low log2(2*SEW) bits are used per spec
            None => src.scalar & shamt_mask,
        };
        let result_wide = if ARITHMETIC {
            // Sign-extend to i64 first, then shift arithmetically as i64 to
            // preserve sign bits, then cast back. Shifting u64 after cast_unsigned()
            // would be a logical shift and lose sign bits.
            (sign_extend_bits(wide_val, wide_sew) >> shamt).cast_unsigned()
        } else {
            wide_val >> shamt
        };
        // Truncate to SEW bits
        let result = result_wide & ((1u64 << sew.bits_width()) - 1);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute an integer extension (vzext/vsext).
///
/// Source element width is `sew.source()`, destination is `sew.dest()`. `SIGN` selects sign- or
/// zero-extension.
///
/// The source EMUL = LMUL / factor; the source register group is `max(1, group_regs / factor)`
/// registers.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_extension<const SIGN: bool, Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    vm: bool,
    sew: ExtensionSew,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let src_sew = sew.source();

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let raw = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let result = if SIGN {
            sign_extend_bits(raw, src_sew).cast_unsigned()
        } else {
            raw
        };
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}
