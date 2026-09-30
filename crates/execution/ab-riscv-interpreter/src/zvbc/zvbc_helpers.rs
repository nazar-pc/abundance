//! Opaque helpers for Zvbc extension

use crate::rv64::b::zbc::rv64_zbc_helpers;
use crate::v::vector_registers::{VRegGroup, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::OpSrc;
use crate::v::zvexx::arith::zvexx_arith_helpers::sew_mask;
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Lower SEW bits of the carry-less product of two SEW-wide values.
///
/// Both inputs are masked to SEW bits before the multiplication so that the VX form (where
/// the scalar register may carry bits above the SEW boundary) behaves identically to the VV
/// form (where `VectorRegisterFile::read()` already zero-extends elements to exactly SEW bits).
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn vclmul_element(a: u64, b: u64, sew: Vsew) -> u64 {
    let mask = sew_mask(sew);
    let a = a & mask;
    let b = b & mask;
    rv64_zbc_helpers::clmul(a, b) & mask
}

/// Upper SEW bits of the carry-less product of two SEW-wide values.
///
/// Both inputs are masked to SEW bits (see [`vclmul_element()`] for rationale).
///
/// For SEW < 64, the product fits in 64 bits; the upper half lives at bits
/// `[2*SEW-1 : SEW]` of `clmul(a, b)`. `clmulh` would return 0 for SEW-bit inputs
/// since the product never reaches bit 64.
/// For SEW = 64, `clmulh` directly returns the upper half of the 128-bit product.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn vclmulh_element(a: u64, b: u64, sew: Vsew) -> u64 {
    let mask = sew_mask(sew);
    let a = a & mask;
    let b = b & mask;
    if sew == Vsew::E64 {
        rv64_zbc_helpers::clmulh(a, b)
    } else {
        // The 2*SEW-bit product fits in the 64-bit return value of clmul; extract
        // bits [2*SEW-1 : SEW] and mask back to SEW bits.
        (rv64_zbc_helpers::clmul(a, b) >> sew.bits_width()) & mask
    }
}

/// Execute element-wise carry-less multiplication (lower half) over `0..vl`.
///
/// For each active element i: `vd[i] = lower_sew_bits(clmul(vs2[i], src[i]))`.
///
/// When `vm=true` all elements are active. When `vm=false` the mask register `v0` gates
/// each element; masked-off elements are left undisturbed (undisturbed policy).
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vclmul<Reg, Env>(
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
        let result = vclmul_element(a, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute element-wise carry-less multiplication (upper half) over `0..vl`.
///
/// For each active element i: `vd[i] = upper_sew_bits(clmul(vs2[i], src[i]))`.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_vclmulh<Reg, Env>(
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
        let result = vclmulh_element(a, b, sew);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}
