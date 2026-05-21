//! Opaque helpers for Zvbb extension

use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{OpSrc, check_vreg_group_alignment};
use crate::v::zvexx::arith::zvexx_arith_helpers::{read_element_u64, write_element_u64};
use crate::v::zvexx::load::zvexx_load_helpers::mask_bit;
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Execute element-wise full bit-reversal over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: all bits within `vs2[i]` are reversed end-to-end
/// (bit 0 <-> bit SEW-1). This differs from `vbrev8`, which reverses bits within each byte while
/// preserving byte order; `vbrev` also inverts the byte order as a side effect of reversing the
/// whole element.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0` and `vd.to_bits() + group_regs <= 32`
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32`
/// - `vl <= group_regs * VLENB / sew_bytes`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vbrev<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // `elem` is zero-extended from SEW bits to u64; reverse_bits() on the primitive type
        // of exactly SEW width naturally handles the upper zero bits from zero-extension
        let result = match sew {
            Vsew::E8 => u64::from((elem as u8).reverse_bits()),
            Vsew::E16 => u64::from((elem as u16).reverse_bits()),
            Vsew::E32 => u64::from((elem as u32).reverse_bits()),
            Vsew::E64 => elem.reverse_bits(),
        };
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise count-leading-zeros over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = clz(vs2[i])`, counting within the SEW-wide field. An
/// all-zero element produces SEW, not 64.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vbrev`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vclz<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let sew_bits = u32::from(sew.bits_width());
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // `elem` is zero-extended from SEW bits to u64; `leading_zeros()` on a u64 therefore counts
        // the extra (64 - SEW) upper zero bits introduced by zero-extension. Subtracting them gives
        // the count within the SEW-wide field.
        let clz = elem.leading_zeros() - (64 - sew_bits);
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, u64::from(clz));
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise count-trailing-zeros over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = ctz(vs2[i])`, counting within the SEW-wide field. An
/// all-zero element produces SEW, not 64.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vbrev`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vctz<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let sew_bits = u32::from(sew.bits_width());
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // For non-zero `elem`, `trailing_zeros()` on the zero-extended u64 value is correct: the
        // upper zero bits do not affect the trailing count. For zero, `trailing_zeros()` returns
        // 64, but the spec result is SEW; cap at `sew_bits` handles both cases.
        let ctz = elem.trailing_zeros().min(sew_bits);
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, u64::from(ctz));
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise population count over `vstart..vl`, writing SEW-wide results into `vd`.
///
/// For each active element i: `vd[i] = popcount(vs2[i])`, in range `[0, SEW]`.
///
/// When `vm=false`, masked-off elements are left undisturbed.
///
/// # Safety
/// Same register-group constraints as [`execute_vbrev`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vcpop<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(ext_state.read_vregs().get(VReg::V0), i) {
            continue;
        }
        // SAFETY: `vs2 % group_regs == 0` and `vs2 + group_regs <= 32`; `i < vl`
        let elem = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // `elem` is zero-extended from SEW bits; upper bits are already zero, so `count_ones()`
        // directly gives the population count within the SEW-wide field
        let cpop = elem.count_ones();
        // SAFETY: `vd % group_regs == 0` and `vd + group_regs <= 32`; `i < vl`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, u64::from(cpop));
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute element-wise widening shift-left-logical over `vstart..vl`, writing 2*SEW-wide
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
/// # Safety
/// - `vd` register group satisfies alignment for EMUL = 2*LMUL: `vd.to_bits() % dest_group_regs ==
///   0` and `vd.to_bits() + dest_group_regs <= 32`
/// - `vs2` register group satisfies alignment for LMUL
/// - `src` register (if `Vreg`) satisfies the same alignment as `vs2`
/// - `vl <= dest_group_regs * VLENB / double_sew_bytes`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vwsll<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    sew: Vsew,
    double_sew: Vsew,
    vm: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    // `double_sew_bits` is always a power of two (16, 32, or 64); `& (bits - 1)` is equivalent to
    // `% bits` and avoids a division
    let double_sew_bits = u64::from(double_sew.bits_width());
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
        let shift = (amount & (double_sew_bits - 1)) as u32;
        // `a` is zero-extended from SEW bits; `shift < double_sew_bits <= 64`, so this never shifts
        // by >= 64. The caller guarantees SEW <= E32, hence `double_sew_bits <= 64`.
        let result = a << shift;
        // SAFETY: `vd % dest_group_regs == 0` and `vd + dest_group_regs <= 32`; `i < vl`;
        // `write_element_u64` with `double_sew` writes exactly 2*SEW bits of `result`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, double_sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
