//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VRegGroup, VectorRegisterFile, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{OpSrc, check_mask_dest_overlap};
use crate::v::zvexx::arith::zvexx_arith_helpers::{sew_mask, write_mask_bit};
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Read a single mask bit from vector register `v0` at element index `i`.
///
/// Used to retrieve the per-element carry-in or borrow-in for vadc/vsbc.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(in super::super) fn carry_bit<const VLEN: Vlen>(
    vregs: &VectorRegisterFile<VLEN>,
    i: u16,
) -> u64 {
    let v0 = vregs.get(VReg::V0);
    u64::from(mask_bit(v0, i))
}

/// Execute an element-wise add-with-carry over `0..vl`, writing SEW-wide data results into
/// `vd`.
///
/// Carry-in for each element is read from `v0[i]` when `WITH_CARRY` is true. All elements in
/// `0..vl` are processed unconditionally (no execution mask).
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_carry_add<const WITH_CARRY: bool, Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
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
        let c = if WITH_CARRY {
            carry_bit(env.read_vregs(), i)
        } else {
            0
        };

        // Wrap naturally: `VectorRegisterFile::write()` writes only the low `SEW` bits
        let result = a.wrapping_add(b).wrapping_add(c);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }

    env.mark_vs_dirty();
}

/// Execute an element-wise subtract-with-borrow over `0..vl`, writing SEW-wide data results
/// into `vd`.
///
/// Borrow-in for each element is read from `v0[i]` (always true for vsbc). All elements in
/// `0..vl` are processed unconditionally.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_carry_sub<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
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
        let borrow = carry_bit(env.read_vregs(), i);

        let result = a.wrapping_sub(b).wrapping_sub(borrow);
        env.write_vregs()
            .write(vd, i, result)
            .expect("`i < vl` of `vd`; qed");
    }

    env.mark_vs_dirty();
}

/// Execute an element-wise add-with-carry over `0..vl`, writing the carry-out as a single mask
/// bit per element into `vd`.
///
/// When `WITH_CARRY` is true, carry-in for element `i` is read from `v0[i]`. When false, carry-in
/// is treated as zero.
///
/// All elements are processed unconditionally (no execution mask).
///
/// Tail mask bits (indices `>= vl`) are left undisturbed per spec §5.3.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_carry_add_mask<const WITH_CARRY: bool, Reg, Env>(
    env: &mut Env,
    vd: VReg,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    sew: Vsew,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
{
    let vl = vs2.vl();
    let Some(src) = src.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask = sew_mask(sew);

    for i in vl.indices() {
        let a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`i < vl` of `vs2`; qed");
        let b = match src.vreg {
            Some(vs1) => env
                .read_vregs()
                .read(vs1, i)
                .expect("`vs1` has the same `vl` as `vs2`, checked above; qed"),
            None => src.scalar,
        };
        let c = if WITH_CARRY {
            carry_bit(env.read_vregs(), i)
        } else {
            0
        };

        // Use u128 to capture the carry-out bit beyond SEW
        let sum = u128::from(a & mask) + u128::from(b & mask) + u128::from(c);
        let carry_out = (sum >> sew.bits_width()) & 1 != 0;

        write_mask_bit(env.write_vregs(), vd, i, carry_out)
            .expect("`i < vl <= VLEN`, so the mask bit is within `vd`; qed");
    }

    env.mark_vs_dirty();
}

/// Execute an element-wise subtract-with-borrow over `0..vl`, writing the borrow-out as a
/// single mask bit per element into `vd`.
///
/// When `WITH_BORROW` is true, borrow-in for element `i` is read from `v0[i]`. When false,
/// borrow-in is treated as zero.
///
/// Borrow-out is 1 when the subtraction underflows unsigned:
/// `borrow_out = (b + borrow_in) > a` (compared as SEW-wide unsigned values).
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_carry_sub_mask<const WITH_BORROW: bool, Reg, Env>(
    env: &mut Env,
    vd: VReg,
    vs2: VRegGroup<{ Env::VLEN }>,
    src: OpSrc<VRegGroup<{ Env::VLEN }>>,
    sew: Vsew,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
{
    let vl = vs2.vl();
    let Some(src) = src.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask = sew_mask(sew);

    for i in vl.indices() {
        let a = env
            .read_vregs()
            .read(vs2, i)
            .expect("`i < vl` of `vs2`; qed");
        let b = match src.vreg {
            Some(vs1) => env
                .read_vregs()
                .read(vs1, i)
                .expect("`vs1` has the same `vl` as `vs2`, checked above; qed"),
            None => src.scalar,
        };
        let borrow_in = if WITH_BORROW {
            carry_bit(env.read_vregs(), i)
        } else {
            0
        };

        let a_m = u128::from(a & mask);
        let rhs = u128::from(b & mask) + u128::from(borrow_in);
        let borrow_out = a_m < rhs;

        write_mask_bit(env.write_vregs(), vd, i, borrow_out)
            .expect("`i < vl <= VLEN`, so the mask bit is within `vd`; qed");
    }

    env.mark_vs_dirty();
}
