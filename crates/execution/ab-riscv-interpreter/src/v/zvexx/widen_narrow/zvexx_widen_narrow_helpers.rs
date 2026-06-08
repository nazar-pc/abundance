//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VectorRegisterFile, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{OpSrc, check_vreg_group_alignment};
use crate::v::zvexx::zvexx_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter};
use ab_riscv_primitives::instructions::v::Vsew;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::hint::cold_path;

/// Check that a widening destination `vd` is aligned to `wide_group_regs` and fits within
/// `[0,32)`, without any source overlap check
#[inline(always)]
#[doc(hidden)]
pub fn check_vd_widen_no_src_check<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vd: VReg,
    wide_group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vd_idx = vd.to_bits();
    if !vd_idx.is_multiple_of(wide_group_regs) || vd_idx + wide_group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that an extension source `vs2` is aligned to `src_group_regs`, fits in `[0,32)`, and only
/// overlaps `vd` (which occupies `group_regs` registers) in a manner permitted by the spec.
///
/// Per the vector spec §5.2, the destination EEW (SEW) of an extension is greater than the source
/// EEW (SEW/factor), so the destination may overlap the source only when the source EMUL is at
/// least 1 and the overlap is in the highest-numbered part of the destination register group (e.g.
/// `vzext.vf4 v0, v6` with LMUL=8, where the narrow source `{v6,v7}` aliases the high registers of
/// the wide `{v0..v7}` destination). Any other overlap is illegal.
#[inline(always)]
#[doc(hidden)]
pub fn check_vs_ext_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vs2: VReg,
    src_group_regs: u8,
    vd: VReg,
    group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vs2_idx = vs2.to_bits();
    if !vs2_idx.is_multiple_of(src_group_regs) || vs2_idx + src_group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    // The wide destination (group_regs) may overlap the narrow source (src_group_regs) only in the
    // highest-numbered part of the destination group, and only when the source EMUL >= 1.
    if widen_src_overlap_illegal(vd.to_bits(), group_regs, vs2_idx, src_group_regs) {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that a widening destination `vd` is aligned to `wide_group_regs`, fits within `[0, 32)`,
/// and only overlaps the `group_regs`-register narrow source(s) starting at `vs_a`/`vs_b` in a
/// manner permitted by the spec.
///
/// `wide_group_regs` is the pre-computed register count for the wide EMUL (2*LMUL), obtained via
/// `Vlmul::index_register_count(wide_eew, sew)`. `group_regs` is the narrow LMUL register count.
///
/// Per the vector spec §5.2, a destination whose EEW (2*SEW) is greater than a source's EEW (SEW)
/// may overlap that source only when the source EMUL is at least 1 and the overlap is in the
/// highest-numbered part of the destination register group (e.g. `vwsubu.wv v2, v14, v3` with
/// LMUL=1, where the narrow `v3` aliases the high register of the wide `{v2, v3}` destination).
/// Any other overlap is illegal.
#[inline(always)]
#[doc(hidden)]
pub fn check_vd_widen_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vd: VReg,
    vs_a: VReg,
    vs_b_opt: Option<VReg>,
    group_regs: u8,
    wide_group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vd_idx = vd.to_bits();
    if !vd_idx.is_multiple_of(wide_group_regs) || vd_idx + wide_group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    if widen_src_overlap_illegal(vd_idx, wide_group_regs, vs_a.to_bits(), group_regs) {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    if let Some(vs_b) = vs_b_opt
        && widen_src_overlap_illegal(vd_idx, wide_group_regs, vs_b.to_bits(), group_regs)
    {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Returns `true` when a narrow source group of `group_regs` registers starting at `vs_idx`
/// overlaps the wide destination group (`wide_group_regs` registers starting at `vd_idx`) in a way
/// that is *not* permitted by the spec.
///
/// Overlap is only legal when the source EMUL is at least 1 - which, on widening, is exactly when
/// the destination register count strictly exceeds the narrow source count (for fractional LMUL
/// both counts collapse to 1) - and the source occupies the highest-numbered registers of the
/// destination group.
#[inline(always)]
fn widen_src_overlap_illegal(vd_idx: u8, wide_group_regs: u8, vs_idx: u8, group_regs: u8) -> bool {
    if !ranges_overlap(vd_idx, wide_group_regs, vs_idx, group_regs) {
        return false;
    }
    let high_part_overlap =
        wide_group_regs > group_regs && vs_idx == vd_idx + wide_group_regs - group_regs;
    !high_part_overlap
}

/// Check that a widening source `vs2` that is already 2×SEW wide is aligned to `wide_group_regs`
/// and fits within `[0, 32)`.
#[inline(always)]
#[doc(hidden)]
pub fn check_vs_wide_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vs: VReg,
    wide_group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vs_idx = vs.to_bits();
    if !vs_idx.is_multiple_of(wide_group_regs) || vs_idx + wide_group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that a narrowing destination `vd` is aligned to `group_regs` and fits
/// within `[0, 32)`.
///
/// No overlap check against `vs2` is performed here because narrowing instructions
/// permit `vd` to alias the low half of the wide `vs2` register group per spec §11.7.
#[inline(always)]
#[doc(hidden)]
pub fn check_vd_narrow_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vd: VReg,
    group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vd_idx = vd.to_bits();
    if !vd_idx.is_multiple_of(group_regs) || vd_idx + group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Returns `true` when `[a_start, a_start+a_len)` overlaps `[b_start, b_start+b_len)`.
#[inline(always)]
fn ranges_overlap(a_start: u8, a_len: u8, b_start: u8, b_len: u8) -> bool {
    a_start < b_start + b_len && b_start < a_start + a_len
}

/// Return whether mask bit `i` is set in the mask byte slice (LSB-first within each byte).
#[inline(always)]
fn mask_bit(mask: &[u8], i: u32) -> bool {
    mask.get((i / u8::BITS) as usize)
        .is_some_and(|b| (b >> (i % u8::BITS)) & 1 != 0)
}

/// Snapshot the mask register into a stack buffer.
///
/// When `vm=true` (unmasked), all bytes are `0xff`.
///
/// # Safety
/// `vl.div_ceil(8) <= VLENB` must hold. This is guaranteed when `vl <= VLEN`.
#[inline(always)]
unsafe fn snapshot_mask<const VLENB: usize>(
    vregs: &VectorRegisterFile<VLENB>,
    vm: bool,
    vl: u32,
) -> [u8; VLENB] {
    let mut buf = [0u8; VLENB];
    if vm {
        buf = [0xffu8; VLENB];
    } else {
        let mask_bytes = vl.div_ceil(u8::BITS) as usize;
        // SAFETY: `mask_bytes <= VLENB` by precondition
        unsafe {
            buf.get_unchecked_mut(..mask_bytes)
                .copy_from_slice(vregs.get(VReg::V0).get_unchecked(..mask_bytes));
        }
    }
    buf
}

/// Read the low `sew.bytes_width()` of the element `elem_i` from the register group `base_reg`,
/// zero-extended to `u64`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / sew.bytes_width()) < 32`
#[inline(always)]
unsafe fn read_element_u64<const VLENB: usize>(
    vregs: &VectorRegisterFile<VLENB>,
    base_reg: VReg,
    elem_i: u32,
    sew: Vsew,
) -> u64 {
    let sew_bytes = usize::from(sew.bytes_width());
    let elems_per_reg = VLENB / sew_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * sew_bytes;
    // SAFETY: `base_reg + reg_off < 32` by caller's precondition
    let reg = unsafe {
        vregs.get(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap_unchecked())
    };
    // SAFETY: `byte_off + sew_bytes <= VLENB`
    let src = unsafe { reg.get_unchecked(byte_off..byte_off + sew_bytes) };
    let mut buf = [0u8; 8];
    // SAFETY: `sew_bytes <= 8`
    unsafe { buf.get_unchecked_mut(..sew_bytes) }.copy_from_slice(src);
    u64::from_le_bytes(buf)
}

/// Write the low `sew.bytes_width()` of `value` into element `elem_i` in register group `base_reg`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / sew.bytes_width()) < 32`
#[inline(always)]
unsafe fn write_element_u64<const VLENB: usize>(
    vregs: &mut VectorRegisterFile<VLENB>,
    base_reg: VReg,
    elem_i: u32,
    sew: Vsew,
    value: u64,
) {
    let sew_bytes = usize::from(sew.bytes_width());
    let elems_per_reg = VLENB / sew_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * sew_bytes;
    let buf = value.to_le_bytes();
    // SAFETY: `base_reg + reg_off < 32` by caller's precondition
    let reg = unsafe {
        vregs.get_mut(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap_unchecked())
    };
    // SAFETY: `byte_off + sew_bytes <= VLENB`
    let dst = unsafe { reg.get_unchecked_mut(byte_off..byte_off + sew_bytes) };
    // SAFETY: `sew_bytes <= 8`
    dst.copy_from_slice(unsafe { buf.get_unchecked(..sew_bytes) });
}

/// Sign-extend the low `bits` of `val` to `i64`.
#[inline(always)]
#[doc(hidden)]
pub fn sign_extend_bits(val: u64, bits: u8) -> i64 {
    let shift = u64::BITS - u32::from(bits);
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
fn scalar_unsigned_for_sew(val: u64, sew_bits: u8) -> u64 {
    val & (u64::MAX >> (u64::BITS - u32::from(sew_bits)))
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
fn scalar_signed_for_sew(val: u64, sew_bits: u8) -> u64 {
    sign_extend_bits(val, sew_bits).cast_unsigned()
}

/// Execute a widening integer add/subtract.
///
/// Each source element is SEW-wide; the destination element is 2×SEW-wide.
/// `ZERO_EXTEND_AB` selects unsigned or signed widening for sources (unsigned = zero-extend,
/// signed = sign-extend).
///
/// `op` receives `(wide_a: u64, wide_b: u64) -> u64`.
///
/// # Safety
/// - `vd` aligned to `2*group_regs`, fits in `[0,32)`, does not overlap `vs2` or `src` (verified by
///   caller)
/// - `vs2` aligned to `group_regs`, fits in `[0,32)` (verified by caller)
/// - `src` register (when `WidenSrc::Vreg`) aligned to `group_regs`, fits in `[0,32)` (verified by
///   caller)
/// - `vl <= group_regs * VLENB / sew.bytes_width()` (all elements fit)
/// - SEW < 64
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_widen_op<const ZERO_EXTEND_AB: bool, Reg, ExtState, CustomError, F>(
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
    F: Fn(u64, u64) -> u64,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let wide_sew = sew
        .double_width()
        .expect("SEW < 64 is enforced by caller, hence this is always valid; qed");

    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2` aligned to `group_regs`;
        // `i < vl <= group_regs * (VLENB / sew.bytes_width())`
        let raw_a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        let wide_a = if ZERO_EXTEND_AB {
            raw_a
        } else {
            sign_extend_bits(raw_a, sew.bits_width()).cast_unsigned()
        };
        let wide_b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same argument as vs2
                let raw_b = unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) };
                if ZERO_EXTEND_AB {
                    raw_b
                } else {
                    sign_extend_bits(raw_b, sew.bits_width()).cast_unsigned()
                }
            }
            OpSrc::Scalar(val) => {
                if ZERO_EXTEND_AB {
                    scalar_unsigned_for_sew(val, sew.bits_width())
                } else {
                    scalar_signed_for_sew(val, sew.bits_width())
                }
            }
        };
        let result = op(wide_a, wide_b);
        // SAFETY: `vd` aligned to `2*group_regs`;
        // `i < vl <= group_regs * (VLENB / sew.bytes_width())` so
        // `i < 2*group_regs * (VLENB / wide_sew.bytes_width())` - element fits in the wide group
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, wide_sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a widening add/subtract where `vs2` is already 2×SEW wide.
///
/// `vs2` is read at `wide_sew.bytes_width()`; `src` (narrow) is read at `sew.bytes_width()` and
/// widened. `ZERO_EXTEND_B` selects unsigned vs signed widening for the narrow source operand.
///
/// # Safety
/// - `vd` aligned to `2*group_regs`, fits in `[0,32)`, does not overlap `vs2` or `src`
/// - `vs2` aligned to `2*group_regs`, fits in `[0,32)` (wide source)
/// - `src` register (when `WidenSrc::Vreg`) aligned to `group_regs`, fits in `[0,32)`
/// - `vl <= group_regs * VLENB / sew.bytes_width()`
/// - SEW < 64
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_widen_w_op<const ZERO_EXTEND_B: bool, Reg, ExtState, CustomError, F>(
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
    F: Fn(u64, u64) -> u64,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let wide_sew = sew
        .double_width()
        .expect("SEW < 64 is enforced by caller, hence this is always valid; qed");

    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // vs2 is already 2×SEW; read at wide width
        // SAFETY: `vs2` aligned to `2*group_regs`; element `i` fits within it
        let wide_a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, wide_sew) };
        let wide_b = match src {
            OpSrc::Vreg(vs1) => {
                // SAFETY: `vs1` is aligned to `group_regs` and fits within `[0, 32)`,
                // verified by caller; `i < vl <= group_regs * (VLENB / sew.bytes_width())`,
                // so `vs1_base + i / elems_per_reg < vs1_base + group_regs <= 32`
                let raw_b = unsafe { read_element_u64(ext_state.read_vregs(), vs1, i, sew) };
                if ZERO_EXTEND_B {
                    raw_b
                } else {
                    sign_extend_bits(raw_b, sew.bits_width()).cast_unsigned()
                }
            }
            OpSrc::Scalar(val) => {
                if ZERO_EXTEND_B {
                    scalar_unsigned_for_sew(val, sew.bits_width())
                } else {
                    scalar_signed_for_sew(val, sew.bits_width())
                }
            }
        };
        let result = op(wide_a, wide_b);
        // SAFETY: same as `execute_widen_op` for vd
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, wide_sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a narrowing right-shift.
///
/// `vs2` is 2×SEW wide; the shift amount comes from `src` (SEW-wide or scalar).
/// The shift amount is masked to `log2(2*SEW)` bits per spec §12.6.
/// `ARITHMETIC` selects sign-extending (true) vs zero-extending (false) before shifting.
///
/// # Safety
/// - `vd` aligned to `group_regs`, fits in `[0,32)`
/// - `vs2` aligned to `wide_group_regs`, fits in `[0,32)`; aliasing with the low half of `vs2` is
///   permitted per spec §11.7 - reads complete before writes to any overlapping element since the
///   destination SEW is half the source SEW
/// - `src` register (when `OpSrc::Vreg`) aligned to `group_regs`, fits in `[0,32)`
/// - `vl <= group_regs * VLENB / sew.bytes_width()`
/// - SEW < 64
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_narrow_shift<const ARITHMETIC: bool, Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
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
    let wide_sew = sew
        .double_width()
        .expect("SEW < 64 is enforced by caller, hence this is always valid; qed");
    // Shift amount mask: log2(2*SEW) bits = log2(SEW) + 1 bits
    let shamt_mask = u64::from(wide_sew.bits_width() - 1);

    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2` is the wide source group
        let wide_val = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, wide_sew) };
        let shamt = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: `vs1` is aligned to `group_regs` and fits within `[0, 32)`,
                // verified by caller; `i < vl <= group_regs * (VLENB / sew.bytes_width())`,
                // so `vs1_base + i / elems_per_reg < vs1_base + group_regs <= 32`
                let raw = unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) };
                raw & shamt_mask
            }
            // Scalar shift amount: only the low log2(2*SEW) bits are used per spec
            OpSrc::Scalar(val) => val & shamt_mask,
        };
        let result_wide = if ARITHMETIC {
            // Sign-extend to i64 first, then shift arithmetically as i64 to
            // preserve sign bits, then cast back. Shifting u64 after cast_unsigned()
            // would be a logical shift and lose sign bits.
            (sign_extend_bits(wide_val, wide_sew.bits_width()) >> shamt).cast_unsigned()
        } else {
            wide_val >> shamt
        };
        // Truncate to SEW bits
        let result = result_wide & ((1u64 << sew.bits_width()) - 1);
        // SAFETY: `vd` is the narrow destination group
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute an integer extension (vzext/vsext).
///
/// Source element width is `sew.divide_by_factor(factor).bytes_width()`; destination is
/// `sew.bytes_width()`. `SIGN` selects sign- or zero-extension.
///
/// The source EMUL = LMUL / factor; the source register group is `max(1, group_regs / factor)`
/// registers.
///
/// # Safety
/// - `vd` aligned to `group_regs`, fits in `[0,32)`
/// - `vs2` aligned to `src_group_regs`, fits in `[0,32)`, does not overlap `vd`
/// - `vl <= group_regs * VLENB / sew.bytes_width()`
/// - `sew.divide_by_factor(factor).is_some()`
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_extension<const SIGN: bool, Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    factor: VsewFactor,
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
    let src_sew = sew
        .divide_by_factor(factor)
        .expect("SEW >= factor*8 and valid according to function contract; qed");

    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: vs2 group covers `vl` narrow elements
        let raw = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, src_sew) };
        let result = if SIGN {
            sign_extend_bits(raw, src_sew.bits_width()).cast_unsigned()
        } else {
            raw
        };
        // SAFETY: vd group covers `vl` wide elements
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
