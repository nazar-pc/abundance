//! Opaque helpers for Zvbb extension

use crate::v::vector_registers::{VRegGroup, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::OpSrc;
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use crate::v::zvexx::zvexx_helpers::WideningSew;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Execute element-wise full bit-reversal over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: all bits within `vs2[i]` are reversed end-to-end
/// (bit 0 <-> bit SEW-1). This differs from `vbrev8`, which reverses bits within each byte while
/// preserving byte order; `vbrev` also inverts the byte order as a side effect of reversing the
/// whole element.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vbrev<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // `elem` is zero-extended from SEW bits to u64; reverse_bits() on the primitive type
        // of exactly SEW width naturally handles the upper zero bits from zero-extension
        let result = match sew {
            Vsew::E8 => u64::from((elem as u8).reverse_bits()),
            Vsew::E16 => u64::from((elem as u16).reverse_bits()),
            Vsew::E32 => u64::from((elem as u32).reverse_bits()),
            Vsew::E64 => elem.reverse_bits(),
        };
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise count-leading-zeros over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = clz(vs2[i])`, counting within the SEW-wide field. An
/// all-zero element produces SEW, not 64.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vclz<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let sew_bits = u32::from(sew.bits_width());
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // `elem` is zero-extended from SEW bits to u64; `leading_zeros()` on a u64 therefore counts
        // the extra (64 - SEW) upper zero bits introduced by zero-extension. Subtracting them gives
        // the count within the SEW-wide field.
        let clz = elem.leading_zeros() - (64 - sew_bits);
        env.write_vregs()
            .write(vd, i, u64::from(clz))
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise count-trailing-zeros over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = ctz(vs2[i])`, counting within the SEW-wide field. An
/// all-zero element produces SEW, not 64.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vctz<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let sew_bits = u32::from(sew.bits_width());
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // For non-zero `elem`, `trailing_zeros()` on the zero-extended u64 value is correct: the
        // upper zero bits do not affect the trailing count. For zero, `trailing_zeros()` returns
        // 64, but the spec result is SEW; cap at `sew_bits` handles both cases.
        let ctz = elem.trailing_zeros().min(sew_bits);
        env.write_vregs()
            .write(vd, i, u64::from(ctz))
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise population count over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = popcount(vs2[i])`, in range `[0, SEW]`.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vcpop<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // `elem` is zero-extended from SEW bits; upper bits are already zero, so `count_ones()`
        // directly gives the population count within the SEW-wide field
        let cpop = elem.count_ones();
        env.write_vregs()
            .write(vd, i, u64::from(cpop))
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise widening shift-left-logical over `0..vl`, writing 2*SEW-wide
/// results into `vd`.
///
/// For each active element i: `vd[i] = zero_extend_to_2SEW(vs2[i]) << (src[i] % (2*SEW))`.
/// The source operand width is SEW; the destination element width is `double_sew` (2*SEW).
///
/// The caller must ensure SEW <= E32 (i.e., `sew.double_width()` is `Some`); passing SEW=E64 is a
/// programming error that would produce a result wider than u64.
///
/// When `vm=false`, masked-off destination elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vwsll<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    src: OpSrc<VRegGroup<Env::Hart>>,
    sew: WideningSew<Env::Hart>,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let double_sew = sew.wide();
    let vl = vd.vl();
    let (Some(vs2), Some(src)) = (vs2.with_same_vl(vl), src.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    // `double_sew_bits` is always a power of two (16, 32, or 64); `& (bits - 1)` is equivalent to
    // `% bits` and avoids a division
    let double_sew_bits = u64::from(double_sew.bits_width());
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        let amount = match src.vreg {
            Some(vs1) => env
                .read_vregs()
                .read(vs1, i)
                .expect("`vs1` has the same `vl` as `vd`, checked above; qed"),
            None => src.scalar,
        };
        let shift = (amount % double_sew_bits) as u32;
        // `a` is zero-extended from SEW bits; `shift < double_sew_bits <= 64`, so this never shifts
        // by >= 64.
        let result = a << shift;
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}
