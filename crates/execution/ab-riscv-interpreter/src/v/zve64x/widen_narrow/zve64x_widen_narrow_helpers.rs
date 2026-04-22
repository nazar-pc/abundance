//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zve64x::arith::zve64x_arith_helpers::{OpSrc, check_vreg_group_alignment};
use crate::v::zve64x::zve64x_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter};
use ab_riscv_primitives::instructions::v::Vsew;
use ab_riscv_primitives::prelude::*;
use core::fmt;

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
    let vd_idx = vd.bits();
    if !vd_idx.is_multiple_of(wide_group_regs) || vd_idx + wide_group_regs > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that an extension source `vs2` is aligned to `src_group_regs`, fits in `[0,32)`, and
/// does not overlap `vd` (which occupies `group_regs` registers).
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
    let vs2_idx = vs2.bits();
    if !vs2_idx.is_multiple_of(src_group_regs) || vs2_idx + src_group_regs > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    // vd and vs2 must not overlap
    if ranges_overlap(vd.bits(), group_regs, vs2_idx, src_group_regs) {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that a widening destination `vd` is aligned to `wide_group_regs`, does not overlap the
/// `group_regs` registers starting at `vs_a` or `vs_b`, and fits within `[0, 32)`.
///
/// `wide_group_regs` is the pre-computed register count for the wide EMUL (2*LMUL), obtained via
/// `Vlmul::index_register_count(wide_eew, sew)`. `group_regs` is the narrow LMUL register count.
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
    let vd_idx = vd.bits();
    if !vd_idx.is_multiple_of(wide_group_regs) || vd_idx + wide_group_regs > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    let va_idx = vs_a.bits();
    if ranges_overlap(vd_idx, wide_group_regs, va_idx, group_regs) {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    if let Some(vs_b) = vs_b_opt {
        let vb_idx = vs_b.bits();
        if ranges_overlap(vd_idx, wide_group_regs, vb_idx, group_regs) {
            return Err(ExecutionError::IllegalInstruction {
                address: program_counter.old_pc(INSTRUCTION_SIZE),
            });
        }
    }
    Ok(())
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
    let vs_idx = vs.bits();
    if !vs_idx.is_multiple_of(wide_group_regs) || vs_idx + wide_group_regs > 32 {
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
    let vd_idx = vd.bits();
    if !vd_idx.is_multiple_of(group_regs) || vd_idx + group_regs > 32 {
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
    vreg: &[[u8; VLENB]; 32],
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
                .copy_from_slice(vreg[usize::from(VReg::V0.bits())].get_unchecked(..mask_bytes));
        }
    }
    buf
}

/// Read the low `sew_bytes` of element `elem_i` from register group `base_reg`, zero-extended to
/// `u64`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / sew_bytes) < 32`
#[inline(always)]
unsafe fn read_element_u64<const VLENB: usize>(
    vreg: &[[u8; VLENB]; 32],
    base_reg: usize,
    elem_i: u32,
    sew_bytes: usize,
) -> u64 {
    let elems_per_reg = VLENB / sew_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * sew_bytes;
    // SAFETY: `base_reg + reg_off < 32` by caller's precondition
    let reg = unsafe { vreg.get_unchecked(base_reg + reg_off) };
    // SAFETY: `byte_off + sew_bytes <= VLENB`
    let src = unsafe { reg.get_unchecked(byte_off..byte_off + sew_bytes) };
    let mut buf = [0u8; 8];
    // SAFETY: `sew_bytes <= 8`
    unsafe { buf.get_unchecked_mut(..sew_bytes) }.copy_from_slice(src);
    u64::from_le_bytes(buf)
}

/// Write the low `sew_bytes` of `value` into element `elem_i` in register group `base_reg`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / sew_bytes) < 32`
#[inline(always)]
unsafe fn write_element_u64<const VLENB: usize>(
    vreg: &mut [[u8; VLENB]; 32],
    base_reg: u8,
    elem_i: u32,
    sew_bytes: usize,
    value: u64,
) {
    let elems_per_reg = VLENB / sew_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * sew_bytes;
    let buf = value.to_le_bytes();
    // SAFETY: `base_reg + reg_off < 32` by caller's precondition
    let reg = unsafe { vreg.get_unchecked_mut(usize::from(base_reg) + reg_off) };
    // SAFETY: `byte_off + sew_bytes <= VLENB`
    let dst = unsafe { reg.get_unchecked_mut(byte_off..byte_off + sew_bytes) };
    // SAFETY: `sew_bytes <= 8`
    dst.copy_from_slice(unsafe { buf.get_unchecked(..sew_bytes) });
}

/// Sign-extend the low `bits` of `val` to `i64`.
#[inline(always)]
#[doc(hidden)]
pub fn sign_extend_bits(val: u64, bits: u32) -> i64 {
    let shift = u64::BITS - bits;
    (val.cast_signed() << shift) >> shift
}

/// Execute a widening integer add/subtract.
///
/// Each source element is SEW-wide; the destination element is 2×SEW-wide.
/// `zero_extend_a` and `zero_extend_b` select unsigned vs signed widening for each source
/// (unsigned = zero-extend, signed = sign-extend).
///
/// `op` receives `(wide_a: u64, wide_b: u64) -> u64`.
///
/// # Safety
/// - `vd` aligned to `2*group_regs`, fits in `[0,32)`, does not overlap `vs2` or `src` (verified by
///   caller)
/// - `vs2` aligned to `group_regs`, fits in `[0,32)` (verified by caller)
/// - `src` register (when `WidenSrc::Vreg`) aligned to `group_regs`, fits in `[0,32)` (verified by
///   caller)
/// - `vl <= group_regs * VLENB / sew_bytes` (all elements fit)
/// - SEW < 64 (wide_sew_bytes <= 8)
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widen_op<Reg, ExtState, CustomError, F>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    zero_extend_a: bool,
    zero_extend_b: bool,
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
    let sew_bytes = usize::from(sew.bytes());
    // 2×SEW in bytes; SEW < 64 is enforced by caller, so this is at most 8
    let wide_sew_bytes = sew_bytes * 2;
    let sew_bits = u32::from(sew.bits());

    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    let vs2_base = vs2.bits();

    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2` aligned to `group_regs`; `i < vl <= group_regs * (VLENB / sew_bytes)`
        let raw_a =
            unsafe { read_element_u64(ext_state.read_vreg(), usize::from(vs2_base), i, sew_bytes) };
        let wide_a = if zero_extend_a {
            raw_a
        } else {
            sign_extend_bits(raw_a, sew_bits).cast_unsigned()
        };
        let wide_b = match &src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same argument as vs2
                let raw_b = unsafe {
                    read_element_u64(ext_state.read_vreg(), usize::from(*vs1_base), i, sew_bytes)
                };
                if zero_extend_b {
                    raw_b
                } else {
                    sign_extend_bits(raw_b, sew_bits).cast_unsigned()
                }
            }
            OpSrc::Scalar(val) => *val,
        };
        let result = op(wide_a, wide_b);
        // SAFETY: `vd` aligned to `2*group_regs`; `i < vl <= group_regs * (VLENB / sew_bytes)`
        // so `i < 2*group_regs * (VLENB / wide_sew_bytes)` - element fits in the wide group
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd_base, i, wide_sew_bytes, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a widening add/subtract where `vs2` is already 2×SEW wide.
///
/// `vs2` is read at `wide_sew_bytes`; `src` (narrow) is read at `sew_bytes` and widened.
/// `zero_extend_b` selects unsigned vs signed widening for the narrow source operand.
///
/// # Safety
/// - `vd` aligned to `2*group_regs`, fits in `[0,32)`, does not overlap `vs2` or `src`
/// - `vs2` aligned to `2*group_regs`, fits in `[0,32)` (wide source)
/// - `src` register (when `WidenSrc::Vreg`) aligned to `group_regs`, fits in `[0,32)`
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - SEW < 64
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widen_w_op<Reg, ExtState, CustomError, F>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    zero_extend_b: bool,
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
    let sew_bytes = usize::from(sew.bytes());
    let wide_sew_bytes = sew_bytes * 2;
    let sew_bits = u32::from(sew.bits());

    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    let vs2_base = vs2.bits();

    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // vs2 is already 2×SEW; read at wide width
        // SAFETY: `vs2` aligned to `2*group_regs`; element `i` fits within it
        let wide_a = unsafe {
            read_element_u64(
                ext_state.read_vreg(),
                usize::from(vs2_base),
                i,
                wide_sew_bytes,
            )
        };
        let wide_b = match &src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: `vs1` is aligned to `group_regs` and fits within `[0, 32)`,
                // verified by caller; `i < vl <= group_regs * (VLENB / sew_bytes)`,
                // so `vs1_base + i / elems_per_reg < vs1_base + group_regs <= 32`
                let raw_b = unsafe {
                    read_element_u64(ext_state.read_vreg(), usize::from(*vs1_base), i, sew_bytes)
                };
                if zero_extend_b {
                    raw_b
                } else {
                    sign_extend_bits(raw_b, sew_bits).cast_unsigned()
                }
            }
            OpSrc::Scalar(val) => *val,
        };
        let result = op(wide_a, wide_b);
        // SAFETY: same as `execute_widen_op` for vd
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd_base, i, wide_sew_bytes, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a narrowing right-shift.
///
/// `vs2` is 2×SEW wide; the shift amount comes from `src` (SEW-wide or scalar).
/// The shift amount is masked to `log2(2*SEW)` bits per spec §12.6.
/// `arithmetic` selects sign-extending (true) vs zero-extending (false) before shifting.
///
/// # Safety
/// - `vd` aligned to `group_regs`, fits in `[0,32)`
/// - `vs2` aligned to `wide_group_regs`, fits in `[0,32)`; aliasing with the low half of `vs2` is
///   permitted per spec §11.7 - reads complete before writes to any overlapping element since the
///   destination SEW is half the source SEW
/// - `src` register (when `OpSrc::Vreg`) aligned to `group_regs`, fits in `[0,32)`
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - SEW < 64
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_narrow_shift<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    arithmetic: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let sew_bytes = usize::from(sew.bytes());
    let wide_sew_bytes = sew_bytes * 2;
    // Shift amount mask: log2(2*SEW) bits = log2(SEW) + 1 bits
    // 2*SEW in bits
    let wide_sew_bits = u32::from(sew.bits()) * 2;
    let shamt_mask = u64::from(wide_sew_bits - 1);

    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    let vs2_base = vs2.bits();

    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2` is the wide source group
        let wide_val = unsafe {
            read_element_u64(
                ext_state.read_vreg(),
                usize::from(vs2_base),
                i,
                wide_sew_bytes,
            )
        };
        let shamt = match &src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: `vs1` is aligned to `group_regs` and fits within `[0, 32)`,
                // verified by caller; `i < vl <= group_regs * (VLENB / sew_bytes)`,
                // so `vs1_base + i / elems_per_reg < vs1_base + group_regs <= 32`
                let raw = unsafe {
                    read_element_u64(ext_state.read_vreg(), usize::from(*vs1_base), i, sew_bytes)
                };
                raw & shamt_mask
            }
            // Scalar shift amount: only the low log2(2*SEW) bits are used per spec
            OpSrc::Scalar(val) => val & shamt_mask,
        };
        let result_wide = if arithmetic {
            // Sign-extend to i64 first, then shift arithmetically as i64 to
            // preserve sign bits, then cast back. Shifting u64 after cast_unsigned()
            // would be a logical shift and lose sign bits.
            (sign_extend_bits(wide_val, wide_sew_bits) >> shamt).cast_unsigned()
        } else {
            wide_val >> shamt
        };
        // Truncate to SEW bits
        let result = result_wide & ((1u64 << sew.bits()) - 1);
        // SAFETY: `vd` is the narrow destination group
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd_base, i, sew_bytes, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute an integer extension (vzext/vsext).
///
/// Source element width is `sew_bytes / factor`; destination is `sew_bytes`.
/// `sign_extend` selects sign- vs zero-extension.
///
/// The source EMUL = LMUL / factor; the source register group is `max(1, group_regs / factor)`
/// registers.
///
/// # Safety
/// - `vd` aligned to `group_regs`, fits in `[0,32)`
/// - `vs2` aligned to `src_group_regs`, fits in `[0,32)`, does not overlap `vd`
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `sew_bytes / factor >= 1` (SEW >= factor*8)
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_extension<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    factor: u8,
    sign: bool,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let sew_bytes = usize::from(sew.bytes());
    let src_sew_bytes = sew_bytes / usize::from(factor);
    let src_sew_bits = (u32::from(sew.bits())) / u32::from(factor);

    // SAFETY: `vl <= VLEN`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    let vs2_base = vs2.bits();

    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: vs2 group covers `vl` narrow elements
        let raw = unsafe {
            read_element_u64(
                ext_state.read_vreg(),
                usize::from(vs2_base),
                i,
                src_sew_bytes,
            )
        };
        let result = if sign {
            sign_extend_bits(raw, src_sew_bits).cast_unsigned()
        } else {
            raw
        };
        // SAFETY: vd group covers `vl` wide elements
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd_base, i, sew_bytes, result);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
