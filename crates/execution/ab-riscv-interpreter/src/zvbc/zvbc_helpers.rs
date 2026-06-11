//! Opaque helpers for Zvbc extension

use crate::rv64::b::zbc::rv64_zbc_helpers;
use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{OpSrc, check_vreg_group_alignment};
use crate::v::zvexx::arith::zvexx_arith_helpers::{read_element_u64, sew_mask, write_element_u64};
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Lower SEW bits of the carry-less product of two SEW-wide values.
///
/// Both inputs are masked to SEW bits before the multiplication so that the VX form (where
/// the scalar register may carry bits above the SEW boundary) behaves identically to the VV
/// form (where `read_element_u64` already zero-extends elements to exactly SEW bits).
#[inline(always)]
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

/// Execute element-wise carry-less multiplication (lower half) over `vstart..vl`.
///
/// For each active element i: `vd[i] = lower_sew_bits(clmul(vs2[i], src[i]))`.
///
/// When `vm=true` all elements are active. When `vm=false` the mask register `v0` gates
/// each element; masked-off elements are left undisturbed (undisturbed policy).
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0` and `vd.to_bits() + group_regs <= 32`
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32`
/// - `src` register (if `Vreg`) satisfies the same alignment as `vs2`
/// - `vl <= group_regs * VLENB / sew_bytes`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vclmul<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32` (caller precondition);
        // `i < vl <= group_regs * elems_per_reg`, so
        // `vs2 + i / elems_per_reg < vs2 + group_regs <= 32`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: caller verified the vs1 register group satisfies the same alignment
                // constraint as vs2; the index argument is identical, so the same bound holds
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        let result = vclmul_element(a, b, sew);
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32` (caller precondition);
        // `i < vl <= group_regs * elems_per_reg`, so
        // `vd + i / elems_per_reg < vd + group_regs <= 32`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise carry-less multiplication (upper half) over `vstart..vl`.
///
/// For each active element i: `vd[i] = upper_sew_bits(clmul(vs2[i], src[i]))`.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vclmul`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vclmulh<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same alignment constraint as vs2; same index bound
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        let result = vclmulh_element(a, b, sew);
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
