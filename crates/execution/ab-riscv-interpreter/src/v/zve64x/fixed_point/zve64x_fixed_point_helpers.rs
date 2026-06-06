//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zve64x::arith::zve64x_arith_helpers::{
    OpSrc, check_vreg_group_alignment, sew_mask,
};
use crate::v::zve64x::arith::zve64x_arith_helpers::{
    read_element_u64, sign_extend, write_element_u64,
};
use crate::v::zve64x::load::zve64x_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zve64x::zve64x_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Compute the rounding increment for a right shift of `val` by `shift` bits.
///
/// When `shift == 0` there are no fractional bits so the increment is always zero.
/// `current_result_lsb` is the LSB of the truncated result, required for `Rne` and `Rod`.
#[inline(always)]
fn round_increment(val: u64, shift: u32, mode: Vxrm, current_result_lsb: u64) -> u64 {
    if shift == 0 {
        return 0;
    }
    // `d_minus1_bit`: the most-significant discarded bit (bit position `shift - 1`)
    let d_minus1_bit = (val >> (shift - 1)) & 1;
    // `sticky`: OR of all bits below position `shift - 1`
    let sticky = if shift >= 2 {
        // Any of bits [shift-2 : 0] set?
        (val & ((1u64 << (shift - 1)).wrapping_sub(1))) != 0
    } else {
        false
    };
    match mode {
        // Round nearest up: increment = v[d-1]
        Vxrm::Rnu => d_minus1_bit,
        // Round nearest even: increment = v[d-1] & (sticky | result_lsb)
        Vxrm::Rne => d_minus1_bit & u64::from(sticky || current_result_lsb != 0),
        // Round down / truncate: never increment
        Vxrm::Rdn => 0,
        // Round to odd: set result LSB if any discarded bit was non-zero
        Vxrm::Rod => u64::from(current_result_lsb == 0 && (d_minus1_bit != 0 || sticky)),
    }
}

/// Perform a rounded right shift of `val` by `shift` bits (logical / unsigned).
///
/// Returns `(val >> shift) + round_increment`.
#[inline(always)]
#[doc(hidden)]
pub fn rounded_srl(val: u64, shift: u32, mode: Vxrm) -> u64 {
    let truncated = val >> shift;
    let r = round_increment(val, shift, mode, truncated & 1);
    truncated.wrapping_add(r)
}

/// Perform a rounded arithmetic right shift of `val` (sign-extended to SEW) by `shift` bits.
///
/// Returns the SEW-wide signed result as `u64` (sign bits above SEW are meaningful).
#[inline(always)]
#[doc(hidden)]
pub fn rounded_sra(val: u64, shift: u32, mode: Vxrm, sew: Vsew) -> u64 {
    let signed = sign_extend(val, sew);
    // Treat the raw bits for rounding purposes: rounding uses the unsigned representation of the
    // SEW-wide value (only bits below `shift` matter, so masking is not needed here since the
    // discarded bits are the same regardless of sign extension).
    let truncated_signed = signed >> shift;
    let r = round_increment(val, shift, mode, truncated_signed.cast_unsigned() & 1);
    truncated_signed.cast_unsigned().wrapping_add(r)
}

/// Saturating unsigned add: `vs2 + src`, clamped to `[0, 2^SEW - 1]`.
///
/// Sets `vxsat` to `true` on overflow.
#[inline(always)]
#[doc(hidden)]
pub fn sat_addu(a: u64, b: u64, sew: Vsew, vxsat: &mut bool) -> u64 {
    let mask = sew_mask(sew);
    let a_w = a & mask;
    let b_w = b & mask;
    let result = a_w.wrapping_add(b_w);
    if result & mask < a_w {
        // Overflow: wrapped around
        *vxsat = true;
        mask
    } else {
        result & mask
    }
}

/// Saturating signed add: `vs2 + src`, clamped to `[-(2^(SEW-1)), 2^(SEW-1) - 1]`.
///
/// Sets `vxsat` to `true` on overflow.
#[inline(always)]
#[doc(hidden)]
pub fn sat_add(a: u64, b: u64, sew: Vsew, vxsat: &mut bool) -> u64 {
    let sa = i128::from(sign_extend(a, sew));
    let sb = i128::from(sign_extend(b, sew));
    let result = sa.wrapping_add(sb);
    let min_val = i128::MIN >> (i128::BITS - u32::from(sew.bits_width()));
    let max_val = i128::MAX >> (i128::BITS - u32::from(sew.bits_width()));
    if result < min_val {
        *vxsat = true;
        (min_val as i64).cast_unsigned() & sew_mask(sew)
    } else if result > max_val {
        *vxsat = true;
        (max_val as i64).cast_unsigned() & sew_mask(sew)
    } else {
        (result as i64).cast_unsigned() & sew_mask(sew)
    }
}

/// Saturating unsigned subtract: `vs2 - src`, clamped to `[0, 2^SEW - 1]`.
///
/// Sets `vxsat` to `true` on overflow (underflow to negative).
#[inline(always)]
#[doc(hidden)]
pub fn sat_subu(a: u64, b: u64, sew: Vsew, vxsat: &mut bool) -> u64 {
    let mask = sew_mask(sew);
    let a_w = a & mask;
    let b_w = b & mask;
    if a_w < b_w {
        *vxsat = true;
        0
    } else {
        (a_w - b_w) & mask
    }
}

/// Saturating signed subtract: `vs2 - src`, clamped to `[-(2^(SEW-1)), 2^(SEW-1) - 1]`.
///
/// Sets `vxsat` to `true` on overflow.
#[inline(always)]
#[doc(hidden)]
pub fn sat_sub(a: u64, b: u64, sew: Vsew, vxsat: &mut bool) -> u64 {
    let sa = i128::from(sign_extend(a, sew));
    let sb = i128::from(sign_extend(b, sew));
    let result = sa.wrapping_sub(sb);
    let min_val = i128::MIN >> (i128::BITS - u32::from(sew.bits_width()));
    let max_val = i128::MAX >> (i128::BITS - u32::from(sew.bits_width()));
    if result < min_val {
        *vxsat = true;
        (min_val as i64).cast_unsigned() & sew_mask(sew)
    } else if result > max_val {
        *vxsat = true;
        (max_val as i64).cast_unsigned() & sew_mask(sew)
    } else {
        (result as i64).cast_unsigned() & sew_mask(sew)
    }
}

/// Averaging unsigned add: `(vs2 + src) >> 1` with rounding per `vxrm`.
///
/// Uses a 1-bit wider intermediate to avoid overflow; no saturation, no `vxsat`.
#[inline(always)]
#[doc(hidden)]
pub fn avg_addu(a: u64, b: u64, sew: Vsew, mode: Vxrm) -> u64 {
    let mask = sew_mask(sew);
    let a_w = a & mask;
    let b_w = b & mask;
    // Compute full sum in one extra bit by using u128 or by widening trick.
    // Since SEW <= 64 and both operands are SEW-bit values, the sum fits in SEW+1 bits.
    // Use wrapping_add: the carry out of bit SEW-1 is the extra bit.
    let sum = a_w.wrapping_add(b_w);
    // Carry: set if unsigned sum overflowed SEW bits
    let carry = u64::from(sum & mask < a_w);
    // Full (SEW+1)-bit value: `carry` is at bit position SEW, `sum & mask` are low SEW bits.
    // We need `(carry:sum) >> 1` with rounding.
    // Bit 0 of `sum & mask` is the rounding bit for the truncated division.
    let r = round_increment(sum & mask, 1, mode, (sum >> 1u8) & 1);
    // Shift the (SEW+1)-bit quantity right by 1: result = (carry << (SEW-1)) | ((sum & mask) >> 1)
    let shifted = (carry << (u32::from(sew.bits_width()) - 1)) | ((sum & mask) >> 1u8);
    (shifted.wrapping_add(r)) & mask
}

/// Averaging signed add: `(vs2 + src) >> 1` with rounding per `vxrm`.
///
/// No saturation, no `vxsat`.
#[inline(always)]
#[doc(hidden)]
pub fn avg_add(a: u64, b: u64, sew: Vsew, mode: Vxrm) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Full sum as i128 to avoid overflow
    let sum = i128::from(sa).wrapping_add(i128::from(sb));
    // The low bit is the fractional bit for rounding
    let r = match mode {
        Vxrm::Rnu => (sum & 1).cast_unsigned() as u64,
        Vxrm::Rne => {
            // round-to-nearest-even: increment if fractional bit set AND (result LSB or sticky)
            // For a single bit shift there are no lower sticky bits, so only check result LSB
            let result_lsb = ((sum >> 1u8) & 1).cast_unsigned() as u64;
            ((sum & 1).cast_unsigned() as u64) & result_lsb
        }
        Vxrm::Rdn => 0,
        Vxrm::Rod => {
            // Set result LSB if it would be 0 and the fractional bit is nonzero
            let result_lsb = (sum >> 1u8) & 1;
            u64::from(result_lsb == 0 && (sum & 1) != 0)
        }
    };
    let result = (sum >> 1u8) + i128::from(r);
    (result as i64).cast_unsigned() & sew_mask(sew)
}

/// Averaging unsigned subtract: `(vs2 - src) >> 1` with rounding per `vxrm`.
///
/// No saturation, no `vxsat`.
#[inline(always)]
#[doc(hidden)]
pub fn avg_subu(a: u64, b: u64, sew: Vsew, mode: Vxrm) -> u64 {
    let mask = sew_mask(sew);
    let a_w = a & mask;
    let b_w = b & mask;
    // Compute difference with borrow using wrapping sub; borrow extends to SEW+1 bit.
    let diff = a_w.wrapping_sub(b_w);
    // Borrow: set if a < b (unsigned)
    let borrow = u64::from(a_w < b_w);
    // Full (SEW+1)-bit two's-complement difference:
    // If borrow: the SEW-bit `diff` is correct (it wrapped), and the sign extension bit is 1.
    // Rounding: bit 0 of diff is the fractional bit.
    let r = round_increment(diff & mask, 1, mode, (diff >> 1u8) & 1);
    // Arithmetic right shift by 1 of the (SEW+1)-bit signed value.
    // For unsigned averaging subtract: result = ((SEW+1)-bit diff) / 2 with rounding.
    // The (SEW+1)-bit value is: borrow is the sign bit. If borrow set, value is negative.
    // Result = (borrow << SEW | diff) >> 1 (arithmetic) + r
    // Arithmetic shift: sign bit (`borrow`) propagates.
    let sign_fill = borrow.wrapping_neg(); // all ones if borrow set, zero otherwise
    let shifted = (sign_fill << (u32::from(sew.bits_width()) - 1)) | ((diff & mask) >> 1u8);
    (shifted.wrapping_add(r)) & mask
}

/// Averaging signed subtract: `(vs2 - src) >> 1` with rounding per `vxrm`.
///
/// No saturation, no `vxsat`.
#[inline(always)]
#[doc(hidden)]
pub fn avg_sub(a: u64, b: u64, sew: Vsew, mode: Vxrm) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    let diff = i128::from(sa).wrapping_sub(i128::from(sb));
    let r = match mode {
        Vxrm::Rnu => (diff & 1).cast_unsigned() as u64,
        Vxrm::Rne => {
            let result_lsb = ((diff >> 1u8) & 1).cast_unsigned() as u64;
            ((diff & 1).cast_unsigned() as u64) & result_lsb
        }
        Vxrm::Rdn => 0,
        Vxrm::Rod => {
            let result_lsb = (diff >> 1u8) & 1;
            u64::from(result_lsb == 0 && (diff & 1) != 0)
        }
    };
    let result = (diff >> 1u8) + i128::from(r);
    (result as i64).cast_unsigned() & sew_mask(sew)
}

/// Fractional multiply with rounding and saturation: `vsmul`.
///
/// Computes `(a * b * 2 + rounding) >> SEW`, saturating at the signed maximum when the
/// product of two minimum signed values overflows (`INT_MIN * INT_MIN`).
///
/// Per spec §12.4: `vd[i] = clip(roundoff_signed(vs2[i] * vs1[i] * 2, SEW))`.
/// Sets `vxsat` on overflow.
#[inline(always)]
#[doc(hidden)]
pub fn smul(a: u64, b: u64, sew: Vsew, mode: Vxrm, vxsat: &mut bool) -> u64 {
    // SEW-wide signed min and max in i64 (valid for all SEW <= 64)
    let min_sew = i64::MIN >> (i64::BITS - u32::from(sew.bits_width()));
    let max_sew = i64::MAX >> (i64::BITS - u32::from(sew.bits_width()));
    let sa = i128::from(sign_extend(a, sew));
    let sb = i128::from(sign_extend(b, sew));
    // The only case where `product * 2` overflows a 2*SEW signed result is INT_MIN * INT_MIN.
    // Detect this before any multiply: for SEW=64 INT64_MIN^2 = 2^126 and <<1 would overflow i128.
    if sa == i128::from(min_sew) && sb == i128::from(min_sew) {
        *vxsat = true;
        return max_sew.cast_unsigned() & sew_mask(sew);
    }
    // Full 2*SEW-bit product; no overflow possible because at least one operand != INT_MIN,
    // so |product| < INT_MIN^2 and the value fits in i128 for SEW <= 64.
    let product = sa * sb;
    // Left shift by 1 for the Q-format fractional interpretation; safe because
    // |product| < INT_MIN^2, so after <<1 the result still fits in i128 for SEW <= 64.
    let doubled = product << 1u8;
    // Extract the low SEW bits (the discarded portion) for rounding.
    // Cast to u128 first to avoid sign-extension contaminating the mask.
    let shift = u32::from(sew.bits_width());
    let low_bits = (doubled.cast_unsigned() & u128::from(sew_mask(sew))) as u64;
    // Arithmetic right shift by SEW gives the truncated signed result in SEW-wide range.
    let truncated = doubled >> shift;
    let r = round_increment(
        low_bits,
        shift.min(64),
        mode,
        (truncated.cast_unsigned() as u64) & 1,
    );
    // `truncated` fits in i64 after the SEW-bit shift (it is a SEW-wide signed value).
    let result = (truncated as i64).wrapping_add(r.cast_signed());
    // Clamp to SEW-wide signed range (only reachable if rounding pushed the value over)
    if result < min_sew {
        *vxsat = true;
        min_sew.cast_unsigned() & sew_mask(sew)
    } else if result > max_sew {
        *vxsat = true;
        max_sew.cast_unsigned() & sew_mask(sew)
    } else {
        result.cast_unsigned() & sew_mask(sew)
    }
}

/// Narrowing unsigned clip: read a 2*SEW element from `vs2`, shift right by `shamt` with
/// rounding, saturate to unsigned SEW range, set `vxsat` on clamp.
///
/// `vs2_elem` is the 2*SEW-bit element (zero-extended to u64 for SEW <= 32;
/// for SEW = 64 the doubled width would be 128 bits, but Zve64x only supports SEW up to 64 and
/// the narrowing destination is at most 64 bits wide, so 2*SEW = 128 - however the spec requires
/// `ELEN >= 2*SEW` for narrowing instructions. Since `ELEN = 64` in Zve64x, narrowing is only
/// valid for SEW <= 32 (`2*SEW <= 64`).  The caller must enforce this constraint by checking
/// `vsew` before invoking narrowing operations.
///
/// `vs2_elem` is passed as `u64`; for SEW = 32 it holds a 64-bit (2*SEW) value.
#[inline(always)]
#[doc(hidden)]
pub fn nclipu(vs2_elem: u64, shamt: u32, sew: Vsew, mode: Vxrm, vxsat: &mut bool) -> u64 {
    // Shift right with rounding
    let shifted = rounded_srl(vs2_elem, shamt, mode);
    // Saturate to destination SEW unsigned range [0, 2^SEW - 1]
    let max_dst = sew_mask(sew);
    if shifted > max_dst {
        *vxsat = true;
        max_dst
    } else {
        shifted & max_dst
    }
}

/// Narrowing signed clip: read a 2*SEW signed element from `vs2`, shift right arithmetically
/// with rounding, saturate to signed SEW range.
///
/// Same SEW constraint as [`nclipu`].
#[inline(always)]
#[doc(hidden)]
pub fn nclip(vs2_elem: u64, shamt: u32, sew: Vsew, mode: Vxrm, vxsat: &mut bool) -> u64 {
    // Sign-extend vs2_elem to full i64 treating it as a 2*SEW-bit signed value.
    // For SEW=8 the source is 16-bit, for SEW=16 it is 32-bit, for SEW=32 it is 64-bit.
    let double_sew_bits = sew.bits_width() * 2;
    let shift_amt = i64::BITS - u32::from(double_sew_bits);
    let signed_wide = (vs2_elem.cast_signed() << shift_amt) >> shift_amt;
    // Arithmetic right shift with rounding
    // For rounding we need the raw low bits of the wide value before shifting
    let low_bits = signed_wide.cast_unsigned()
        & if double_sew_bits == 64 {
            u64::MAX
        } else {
            (1u64 << double_sew_bits) - 1
        };
    let truncated = signed_wide >> shamt;
    let r = round_increment(low_bits, shamt, mode, (truncated.cast_unsigned()) & 1);
    let rounded = truncated.wrapping_add(r.cast_signed());
    // Saturate to signed SEW range
    let min_dst = i64::MIN >> (i64::BITS - u32::from(sew.bits_width()));
    let max_dst = i64::MAX >> (i64::BITS - u32::from(sew.bits_width()));
    if rounded < min_dst {
        *vxsat = true;
        min_dst.cast_unsigned() & sew_mask(sew)
    } else if rounded > max_dst {
        *vxsat = true;
        max_dst.cast_unsigned() & sew_mask(sew)
    } else {
        rounded.cast_unsigned() & sew_mask(sew)
    }
}

/// Read a 2*SEW-wide element as `u64` from the double-width source register group of a narrowing
/// instruction.
///
/// For narrowing instructions `vs2` holds elements of width `2*SEW`. The register group size is
/// `2 * group_regs`. Element `i` of width `2*SEW` is located in the same way as a SEW-wide
/// element of width `2*SEW` (i.e., treating `2*SEW` as the element width). For `SEW = 32` this
/// reads 64-bit elements; for `SEW <= 16` it reads narrower elements but zero-extends to `u64`.
///
/// # Safety
/// - `2*SEW <= 64` (Zve64x constraint: only valid for SEW <= 32; caller must verify)
/// - `base_reg + elem_i / (VLENB / (2*sew_bytes)) < 32`
#[inline(always)]
pub unsafe fn read_wide_element_u64<const VLENB: usize>(
    vreg: &[[u8; VLENB]; 32],
    base_reg: VReg,
    elem_i: u32,
    sew: Vsew,
) -> u64 {
    let double_sew_bytes = usize::from(sew.bytes_width()) * 2;
    let elems_per_reg = VLENB / double_sew_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * double_sew_bytes;
    // SAFETY: caller guarantees bounds
    let reg = unsafe { vreg.get_unchecked(usize::from(base_reg.to_bits()) + reg_off) };
    // SAFETY: `byte_off + double_sew_bytes <= VLENB`
    let src = unsafe { reg.get_unchecked(byte_off..byte_off + double_sew_bytes) };
    let mut buf = [0u8; 8];
    // SAFETY: `double_sew_bytes <= 8` (SEW <= 32 for Zve64x narrowing)
    unsafe { buf.get_unchecked_mut(..double_sew_bytes) }.copy_from_slice(src);
    u64::from_le_bytes(buf)
}

/// Execute a single-width fixed-point arithmetic operation that may set `vxsat`.
///
/// `op` receives `(vs2_elem, src_elem, sew, vxrm)` and returns `(result, saturated)`.
/// The helper ORs any saturation flag into `vxsat` after the loop.
///
/// # Safety
/// Same preconditions as `execute_arith_op` in the arithmetic helpers.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_fixed_point_op<Reg, ExtState, CustomError, F>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
    // op: (vs2_elem, src_elem, sew, vxrm) -> result
    F: Fn(u64, u64, Vsew, Vxrm, &mut bool) -> u64,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let vxrm = ext_state.vxrm();
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    let mut any_sat = false;
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: alignment and bounds checked by caller
        let a = unsafe { read_element_u64(ext_state.read_vreg(), vs2, i, sew) };
        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same argument as vs2
                unsafe { read_element_u64(ext_state.read_vreg(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };
        let result = op(a, b, sew, vxrm, &mut any_sat);
        // SAFETY: alignment and bounds checked by caller
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd, i, sew, result);
        }
    }
    if any_sat {
        // vxsat is sticky: OR in the new saturation flag
        ext_state.set_vxsat(true);
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a narrowing fixed-point clip operation.
///
/// `vs2` holds a double-width register group (2x `group_regs` registers). `vd` holds the
/// single-width destination. `src` provides the shift amount (Vreg or Scalar).
///
/// For Zve64x narrowing instructions, `SEW` must be at most 32 because `2*SEW` must fit in 64
/// bits. The caller must verify this constraint before invoking this function.
///
/// # Safety
/// - `sew.bits() <= 32` (Zve64x ELEN = 64 constraint for narrowing)
/// - `vs2.bits() % (2 * group_regs) == 0` and `vs2.bits() + 2 * group_regs <= 32`
/// - `vd.bits() % group_regs == 0` and `vd.bits() + group_regs <= 32`
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_narrowing_clip_op<Reg, ExtState, CustomError, F>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
    // op: (vs2_wide_elem, shamt, sew, vxrm, vxsat) -> result
    F: Fn(u64, u32, Vsew, Vxrm, &mut bool) -> u64,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let vxrm = ext_state.vxrm();
    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    let mut any_sat = false;
    // Mask shift amount to log2(2*SEW) bits per spec §12.11
    let shamt_mask = u64::from(sew.bits_width() * 2 - 1);
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Read 2*SEW-wide source element
        // SAFETY: `vs2` double-width alignment checked by caller
        let wide_a = unsafe { read_wide_element_u64(ext_state.read_vreg(), vs2, i, sew) };
        let shamt = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: vs1 SEW-wide alignment checked by caller
                let raw = unsafe { read_element_u64(ext_state.read_vreg(), vs1_base, i, sew) };
                (raw & shamt_mask) as u32
            }
            OpSrc::Scalar(val) => (val & shamt_mask) as u32,
        };
        let result = op(wide_a, shamt, sew, vxrm, &mut any_sat);
        // SAFETY: `vd` alignment checked by caller
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd, i, sew, result);
        }
    }
    if any_sat {
        ext_state.set_vxsat(true);
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Verify that the destination SEW is valid for narrowing (must be at most 32 in Zve64x).
///
/// Returns `Err(IllegalInstruction)` when `sew.bits() > 32`.
#[inline(always)]
#[doc(hidden)]
pub fn check_narrowing_sew<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    sew: Vsew,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    if sew.bits_width() > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that the double-width source `vs2` of a narrowing instruction is aligned to its register
/// group and fits in `[0, 32)`.
///
/// The source operand has `EEW = 2*SEW`, so its `EMUL = 2*LMUL`. Per v-spec §5.2 the group must be
/// aligned to `EMUL` registers, and `EMUL` outside the legal range `[1/8, 8]` (e.g. `LMUL=8`, which
/// would need `EMUL=16`) is reserved. Unlike `2 * register_count()`, this correctly yields a single
/// register with no alignment constraint for fractional `LMUL` (where `2*LMUL <= 1`).
///
/// `sew` is the destination (narrow) SEW; it must be at most 32 (see [`check_narrowing_sew()`]).
#[inline(always)]
#[doc(hidden)]
pub fn check_vs2_narrowing_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vs2: VReg,
    vlmul: Vlmul,
    sew: Vsew,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    // Source EEW is double the destination SEW. SEW=64 is rejected earlier by
    // `check_narrowing_sew`.
    let wide_eew = match sew {
        Vsew::E8 => Eew::E16,
        Vsew::E16 => Eew::E32,
        Vsew::E32 => Eew::E64,
        Vsew::E64 => {
            return Err(ExecutionError::IllegalInstruction {
                address: program_counter.old_pc(INSTRUCTION_SIZE),
            });
        }
    };
    // `EMUL = 2*LMUL`; `None` when reserved (e.g. LMUL=8 -> EMUL=16).
    let wide_group =
        vlmul
            .data_register_count(wide_eew, sew)
            .ok_or(ExecutionError::IllegalInstruction {
                address: program_counter.old_pc(INSTRUCTION_SIZE),
            })?;
    let vs2_idx = vs2.to_bits();
    if !vs2_idx.is_multiple_of(wide_group) || vs2_idx + wide_group > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}
