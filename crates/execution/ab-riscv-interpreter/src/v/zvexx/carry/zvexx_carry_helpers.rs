//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VectorRegisterFile, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{
    OpSrc, check_mask_dest_no_overlap, check_vreg_group_alignment,
};
use crate::v::zvexx::arith::zvexx_arith_helpers::{
    read_element_u64, sew_mask, write_element_u64, write_mask_bit,
};
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::fmt;

// TODO: Safety comment here doesn't make sense
/// Read a single mask bit from vector register `v0` at element index `i`.
///
/// Used to retrieve the per-element carry-in or borrow-in for vadc/vsbc.
///
/// # Safety
/// `i / 8 < VLENB` must hold, guaranteed when `i < vl <= VLEN`.
#[inline(always)]
pub(in super::super) unsafe fn carry_bit<const VLENB: usize>(
    vregs: &VectorRegisterFile<VLENB>,
    i: u32,
) -> u64 {
    let v0 = vregs.get(VReg::V0);
    u64::from(mask_bit(v0, i))
}

/// Execute an element-wise add-with-carry over `vstart..vl`, writing SEW-wide data results into
/// `vd`.
///
/// Carry-in for each element is read from `v0[i]` when `WITH_CARRY` is true. All elements in
/// `vstart..vl` are processed unconditionally (no execution mask).
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0` and `vd.to_bits() + group_regs <= 32`
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32`
/// - `src` register satisfies the same alignment (verified by caller)
/// - `vd.to_bits() != 0` (vd must not overlap v0, which holds the carry-in)
/// - `vl <= group_regs * VLENB / sew_bytes`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_carry_add<const WITH_CARRY: bool, Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
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
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32` (caller precondition);
        // `i < vl <= group_regs * elems_per_reg`, so
        // `vs2 + i / elems_per_reg < vs2 + group_regs <= 32`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: caller verified that the vs1 register group satisfies the same alignment
                // constraint as vs2; the index argument is identical, so the same bound holds:
                // `vs1_base + i / elems_per_reg < 32`
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        let c = if WITH_CARRY {
            // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
            unsafe { carry_bit(ext_state.read_vregs(), i) }
        } else {
            0
        };

        // Wrap naturally: write_element_u64 writes only the low sew_bytes
        let result = a.wrapping_add(b).wrapping_add(c);
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

/// Execute an element-wise subtract-with-borrow over `vstart..vl`, writing SEW-wide data results
/// into `vd`.
///
/// Borrow-in for each element is read from `v0[i]` (always true for vsbc). All elements in
/// `vstart..vl` are processed unconditionally.
///
/// # Safety
/// Same as [`execute_carry_add()`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_carry_sub<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
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
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32` (caller precondition);
        // `i < vl <= group_regs * elems_per_reg`, so
        // `vs2 + i / elems_per_reg < vs2 + group_regs <= 32`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: caller verified that the vs1 register group satisfies the same alignment
                // constraint as vs2; the index argument is identical, so the same bound holds:
                // `vs1_base + i / elems_per_reg < 32`
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        let borrow = unsafe { carry_bit(ext_state.read_vregs(), i) };

        let result = a.wrapping_sub(b).wrapping_sub(borrow);
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

/// Execute an element-wise add-with-carry over `vstart..vl`, writing the carry-out as a single mask
/// bit per element into `vd`.
///
/// When `WITH_CARRY` is true, carry-in for element `i` is read from `v0[i]`. When false, carry-in
/// is treated as zero.
///
/// All elements are processed unconditionally (no execution mask).
///
/// Tail mask bits (indices `>= vl`) are left undisturbed per spec §5.3.
///
/// # Safety
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32`
/// - `src` register satisfies the same alignment
/// - `vl <= group_regs * VLENB / sew_bytes` and `vl <= VLEN`
/// - vd overlap constraints checked by caller
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_carry_add_mask<const WITH_CARRY: bool, Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
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
    let mask = sew_mask(sew);

    for i in u32::from(vstart)..vl {
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32` (caller precondition);
        // `i < vl <= group_regs * elems_per_reg`, so
        // `vs2 + i / elems_per_reg < vs2 + group_regs <= 32`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: caller verified that the vs1 register group satisfies the same alignment
                // constraint as vs2; the index argument is identical, so the same bound holds:
                // `vs1_base + i / elems_per_reg < 32`
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        let c = if WITH_CARRY {
            // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
            unsafe { carry_bit(ext_state.read_vregs(), i) }
        } else {
            0
        };

        // Use u128 to capture the carry-out bit beyond SEW
        let sum = u128::from(a & mask) + u128::from(b & mask) + u128::from(c);
        let carry_out = (sum >> sew.bits_width()) & 1 != 0;

        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vregs(), vd, i, carry_out);
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute an element-wise subtract-with-borrow over `vstart..vl`, writing the borrow-out as a
/// single mask bit per element into `vd`.
///
/// When `WITH_BORROW` is true, borrow-in for element `i` is read from `v0[i]`. When false,
/// borrow-in is treated as zero.
///
/// Borrow-out is 1 when the subtraction underflows unsigned:
/// `borrow_out = (b + borrow_in) > a` (compared as SEW-wide unsigned values).
///
/// # Safety
/// Same as [`execute_carry_add_mask()`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_carry_sub_mask<const WITH_BORROW: bool, Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
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
    let mask = sew_mask(sew);

    for i in u32::from(vstart)..vl {
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32` (caller precondition);
        // `i < vl <= group_regs * elems_per_reg`, so
        // `vs2 + i / elems_per_reg < vs2 + group_regs <= 32`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: caller verified that the vs1 register group satisfies the same alignment
                // constraint as vs2; the index argument is identical, so the same bound holds:
                // `vs1_base + i / elems_per_reg < 32`
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        let borrow_in = if WITH_BORROW {
            // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
            unsafe { carry_bit(ext_state.read_vregs(), i) }
        } else {
            0
        };

        let a_m = u128::from(a & mask);
        let rhs = u128::from(b & mask) + u128::from(borrow_in);
        let borrow_out = a_m < rhs;

        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vregs(), vd, i, borrow_out);
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
