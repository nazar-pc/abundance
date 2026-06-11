//! Opaque helpers for Zvkb extension

use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{OpSrc, check_vreg_group_alignment};
use crate::v::zvexx::arith::zvexx_arith_helpers::{read_element_u64, sew_mask, write_element_u64};
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Execute element-wise and-not over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = ~src[i] & vs2[i]`.
///
/// When `vm=true` all elements are active. When `vm=false` the mask register `v0` gates each
/// element; masked-off elements are left undisturbed (undisturbed policy).
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0` and `vd.to_bits() + group_regs <= 32`
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32`
/// - `src` register (if `Vreg`) satisfies the same alignment as `vs2`
/// - `vl <= group_regs * VLENB / sew_bytes`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vandn<Reg, ExtState, CustomError>(
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
                // SAFETY: caller verified that the vs1 register group satisfies the same alignment
                // constraint as vs2; the index argument is identical, so the same bound holds
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        // `a` is zero-extended to SEW bits by `read_element_u64`; `!b` may have high bits set, but
        // AND with `a` (whose upper bits are zero) zeros them out naturally
        let result = !b & a;
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

/// Execute element-wise bit-reversal within bytes over `vstart..vl`, writing results into `vd`.
///
/// For each active element i: the bits within each byte of `vs2[i]` are reversed. The byte order
/// within the element is preserved; only the bit order within each byte changes.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vandn`], minus the `src` constraint.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vbrev8<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
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
    let sew_bytes = u32::from(sew.bytes_width());
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // Decompose into bytes (LE = index 0 is least-significant), reverse bits within each active
        // byte, then reassemble; bytes beyond sew_bytes are already zero because `read_element_u64`
        // zero-extends to u64
        let mut bytes = elem.to_le_bytes();
        for byte in &mut bytes[..sew_bytes as usize] {
            *byte = byte.reverse_bits();
        }
        let result = u64::from_le_bytes(bytes);
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise byte reversal over `vstart..vl`, writing results into `vd`.
///
/// For each active element i: the bytes within `vs2[i]` are reversed.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vandn`], minus the `src` constraint.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vrev8<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
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
    let sew_bytes = u32::from(sew.bytes_width());
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // Reverse the byte slice covering exactly the SEW-wide element; bytes beyond sew_bytes are
        // zero (from zero-extension) and are left untouched
        let mut bytes = elem.to_le_bytes();
        bytes[..sew_bytes as usize].reverse();
        let result = u64::from_le_bytes(bytes);
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise rotate-left over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = rotate_left(vs2[i], src[i] % SEW)`.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vandn`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vrol<Reg, ExtState, CustomError>(
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
    let sew_bits = u64::from(sew.bits_width());
    let mask = sew_mask(sew);
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let amount = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same alignment constraint as vs2; same index bound
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        // `shift < sew_bits`, so `a << shift` never shifts by >= 64 and is safe.
        // When shift == 0, `sew_bits - shift` == sew_bits; `unbounded_shr` defines
        // shifts >= bit-width as 0, which is correct: a zero rotation contributes no low bits.
        let shift = (amount % sew_bits) as u32;
        let hi = (a << shift) & mask;
        let lo = a.unbounded_shr(sew_bits as u32 - shift);
        let result = hi | lo;
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise rotate-right over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = rotate_right(vs2[i], src[i] % SEW)`.
///
/// Pass `vm=true` for `vror.vi` (bit[25] is consumed as imm[5]; no mask bit exists).
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vandn`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vror<Reg, ExtState, CustomError>(
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
    let sew_bits = u64::from(sew.bits_width());
    let mask = sew_mask(sew);
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let amount = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same alignment constraint as vs2; same index bound
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        // `shift < sew_bits`, so `a >> shift` never shifts by >= 64 and is safe.
        // When shift == 0, `sew_bits - shift` == sew_bits; `unbounded_shl` defines
        // shifts >= bit-width as 0, which is correct: a zero rotation contributes no high bits.
        let shift = (amount % sew_bits) as u32;
        let lo = a >> shift;
        let hi = a.unbounded_shl(sew_bits as u32 - shift) & mask;
        let result = lo | hi;
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
