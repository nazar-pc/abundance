//! Opaque helpers for Zvkb extension

use crate::v::vector_registers::{VRegGroup, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::OpSrc;
use crate::v::zvexx::arith::zvexx_arith_helpers::sew_mask;
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Execute element-wise and-not over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = ~src[i] & vs2[i]`.
///
/// When `vm=true` all elements are active. When `vm=false` the mask register `v0` gates each
/// element; masked-off elements are left undisturbed (undisturbed policy).
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vandn<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    vm: bool,
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
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
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
        // `a` is zero-extended to SEW bits by `VectorRegisterFile::read()`; `!b` may have high bits
        // set, but AND with `a` (whose upper bits are zero) zeros them out naturally
        let result = !b & a;
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise bit-reversal within bytes over `0..vl`, writing results into `vd`.
///
/// For each active element i: the bits within each byte of `vs2[i]` are reversed. The byte order
/// within the element is preserved; only the bit order within each byte changes.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vbrev8<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    sew: Vsew,
    vm: bool,
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
    let sew_bytes = u32::from(sew.bytes_width());
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // Decompose into bytes (LE = index 0 is least-significant), reverse bits within each active
        // byte, then reassemble; bytes beyond sew_bytes are already zero because
        // `VectorRegisterFile::read()` zero-extends to u64
        let mut bytes = elem.to_le_bytes();
        for byte in &mut bytes[..sew_bytes as usize] {
            *byte = byte.reverse_bits();
        }
        let result = u64::from_le_bytes(bytes);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise byte reversal over `0..vl`, writing results into `vd`.
///
/// For each active element i: the bytes within `vs2[i]` are reversed.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vrev8<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    sew: Vsew,
    vm: bool,
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
    let sew_bytes = u32::from(sew.bytes_width());
    for i in vl.indices() {
        if !vm && !mask_bit(env.read_vregs().get(VReg::V0), i) {
            continue;
        }
        let elem = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // Reverse the byte slice covering exactly the SEW-wide element; bytes beyond sew_bytes are
        // zero (from zero-extension) and are left untouched
        let mut bytes = elem.to_le_bytes();
        bytes[..sew_bytes as usize].reverse();
        let result = u64::from_le_bytes(bytes);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise rotate-left over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = rotate_left(vs2[i], src[i] % SEW)`.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vrol<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    sew: Vsew,
    vm: bool,
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
    let sew_bits = u64::from(sew.bits_width());
    let mask = sew_mask(sew);
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
        // `shift < sew_bits`, so `a << shift` never shifts by >= 64 and is safe.
        // When shift == 0, `sew_bits - shift` == sew_bits; `unbounded_shr` defines
        // shifts >= bit-width as 0, which is correct: a zero rotation contributes no low bits.
        let shift = (amount % sew_bits) as u32;
        let hi = (a << shift) & mask;
        let lo = a.unbounded_shr(sew_bits as u32 - shift);
        let result = hi | lo;
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise rotate-right over `0..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = rotate_right(vs2[i], src[i] % SEW)`.
///
/// Pass `vm=true` for `vror.vi` (bit[25] is consumed as imm[5]; no mask bit exists).
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vror<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    sew: Vsew,
    vm: bool,
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
    let sew_bits = u64::from(sew.bits_width());
    let mask = sew_mask(sew);
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
        // `shift < sew_bits`, so `a >> shift` never shifts by >= 64 and is safe.
        // When shift == 0, `sew_bits - shift` == sew_bits; `unbounded_shl` defines
        // shifts >= bit-width as 0, which is correct: a zero rotation contributes no high bits.
        let shift = (amount % sew_bits) as u32;
        let lo = a >> shift;
        let hi = a.unbounded_shl(sew_bits as u32 - shift) & mask;
        let result = lo | hi;
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}
