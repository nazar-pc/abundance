//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VectorRegisterFile, VectorRegistersExt};
pub use crate::v::zvexx::arith::zvexx_arith_helpers::check_vreg_group_alignment;
use crate::v::zvexx::arith::zvexx_arith_helpers::{read_element_u64, write_element_u64};
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Check that register groups `[a, a+count)` and `[b, b+count)` do not overlap.
///
/// Both groups must have the same size `count`. For groups of different sizes use
/// [`check_no_overlap_asymmetric`].
#[inline(always)]
#[doc(hidden)]
pub fn check_no_overlap<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    a: VReg,
    b: VReg,
    count: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let a_start = u16::from(a.to_bits());
    let b_start = u16::from(b.to_bits());
    let count = u16::from(count);
    // Intervals [a_start, a_start+count) and [b_start, b_start+count) overlap iff
    // each starts before the other ends. Arithmetic is widened to u16 to avoid u8 overflow
    // (e.g., b_start=30 + count=8 = 38, which overflows u8).
    if a_start < b_start + count && b_start < a_start + count {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check that register group `[a, a+a_count)` does not overlap `[b, b+b_count)`.
///
/// Unlike [`check_no_overlap`], the two groups are allowed to have different sizes.
/// Used for `vrgatherei16.vv` where vd/vs2 use LMUL-derived `group_regs` and vs1
/// uses EEW=16-derived `index_group_regs`.
#[inline(always)]
#[doc(hidden)]
pub fn check_no_overlap_asymmetric<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    a: VReg,
    a_count: u8,
    b: VReg,
    b_count: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let a_start = u16::from(a.to_bits());
    let b_start = u16::from(b.to_bits());
    let a_count = u16::from(a_count);
    let b_count = u16::from(b_count);
    // Intervals [a_start, a_start+a_count) and [b_start, b_start+b_count) overlap iff
    // each starts before the other ends.
    if a_start < b_start + b_count && b_start < a_start + a_count {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Read element 0 of register `base_reg` as `u64`, zero-extended.
///
/// # Safety
/// `sew.bytes() <= VLENB`
#[inline(always)]
pub unsafe fn read_element_0_u64<const VLENB: usize>(
    vregs: &VectorRegisterFile<VLENB>,
    base_reg: VReg,
    sew: Vsew,
) -> u64 {
    let sew_bytes = usize::from(sew.bytes_width());
    let reg = vregs.get(base_reg);
    let mut buf = [0u8; 8];
    // SAFETY: `sew_bytes <= VLENB` for all legal vtype; `sew_bytes <= 8`
    unsafe {
        buf.get_unchecked_mut(..sew_bytes)
            .copy_from_slice(reg.get_unchecked(..sew_bytes));
    }
    u64::from_le_bytes(buf)
}

/// Write element 0 of register `base_reg` from the low `sew_bytes` of `value`.
///
/// # Safety
/// `sew.bytes() <= VLENB`
#[inline(always)]
pub unsafe fn write_element_0_u64<const VLENB: usize>(
    vregs: &mut VectorRegisterFile<VLENB>,
    base_reg: VReg,
    sew: Vsew,
    value: u64,
) {
    let sew_bytes = usize::from(sew.bytes_width());
    let buf = value.to_le_bytes();
    let reg = vregs.get_mut(base_reg);
    // SAFETY: `sew_bytes <= VLENB`; `sew_bytes <= 8`
    unsafe {
        reg.get_unchecked_mut(..sew_bytes)
            .copy_from_slice(buf.get_unchecked(..sew_bytes));
    }
}

/// Sign-extend the low `sew.bits_width()` of `val` to the register type width.
///
/// The arithmetic is performed entirely in 64-bit signed integer space: we shift the SEW-wide
/// value left to place its sign bit at bit 63, then arithmetic-right-shift back to propagate it.
/// The resulting `u64` is then narrowed to `Reg::Type` (32 or 64 bits) by combining via
/// `From<u32>` - the only integer conversion in the `Register::Type` trait bounds.
///
/// For RV32 (`Reg::XLEN == 32`) the low 32 bits are already the correct sign-extended result
/// because the arithmetic shift propagates the sign across all 64 bits and then we discard the
/// upper half.
///
/// For RV64 (`Reg::XLEN == 64`) we must preserve all 64 bits. Since `Reg::Type: From<u32>` and
/// `Reg::Type: Shl<u8>`, we reconstruct the 64-bit value by OR-ing two 32-bit halves shifted
/// into position.
#[inline(always)]
pub fn sign_extend_to_reg<Reg>(val: u64, sew: Vsew) -> Reg::Type
where
    Reg: Register,
{
    let sew_bits = u32::from(sew.bits_width());
    // `shift` is in [0, 64). When sew_bits == 64, shift == 0 and the value is unchanged.
    let shift = u64::BITS - sew_bits;
    // Cast to i64 so the right-shift is arithmetic (sign-extending).
    let sign_extended = (val.cast_signed() << shift) >> shift;
    let raw = sign_extended.cast_unsigned();
    if Reg::XLEN == u64::BITS as u8 {
        // RV64: preserve all 64 bits by splitting into two u32 halves.
        let lo = Reg::Type::from(raw as u32);
        let hi = Reg::Type::from((raw >> u32::BITS) as u32);
        lo | (hi << 32u8)
    } else {
        // RV32: the low 32 bits are the correctly truncated result.
        Reg::Type::from(raw as u32)
    }
}

/// Execute a vslideup operation.
///
/// Elements `vstart..min(offset, vl)` in vd are unchanged.
/// Elements `max(vstart, offset)..vl` where mask is active get vs2[i - offset].
///
/// # Safety
/// - `vd` and `vs2` are validly aligned and non-overlapping (verified by caller).
/// - `vl <= group_regs * VLENB / sew_bytes`.
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_slideup<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    offset: u64,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    // Per spec §16.3.1: elements 0..offset are never written (vd keeps its value).
    // The active range starts at max(vstart, offset).
    let start = u32::from(vstart).max(offset.min(u64::from(u32::MAX)) as u32);
    for i in start..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let src_idx = u64::from(i) - offset;
        // SAFETY: src_idx < vl <= group_regs * elems_per_reg, so source element is in range
        let val = unsafe { read_element_u64(ext_state.read_vregs(), vs2, src_idx as u32, sew) };
        // SAFETY: i < vl <= group_regs * elems_per_reg, so dest element is in range
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a vslidedown operation.
///
/// Element `vd[i] = vs2[i + offset]` if `i + offset < vlmax`, else `0`.
///
/// # Safety
/// - `vd` and `vs2` are validly aligned (verified by caller); overlap is permitted.
/// - `vl <= vlmax`.
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_slidedown<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    vlmax: u32,
    offset: u64,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Use checked_add to guard against offset being so large that i + offset overflows u64.
        // Any value that wraps past u64::MAX is trivially >= vlmax, so the spec requires vd[i]=0.
        let val = if let Some(src_idx) = u64::from(i).checked_add(offset)
            && src_idx < u64::from(vlmax)
        {
            // SAFETY: src_idx < vlmax <= group_regs * elems_per_reg, so element is in range
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, src_idx as u32, sew) }
        } else {
            0
        };
        // SAFETY: i < vl <= vlmax <= group_regs * elems_per_reg
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a vslide1up operation.
///
/// Element 0 of vd gets `scalar` (when active and vl > 0).
/// Element `i` for `1 <= i < vl` gets `vs2[i - 1]`.
/// vd must not overlap vs2.
///
/// # Safety
/// - `vd` and `vs2` are validly aligned and non-overlapping (verified by caller).
/// - `vl <= group_regs * VLENB / sew_bytes`.
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_slide1up<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    scalar: u64,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let val = if i == 0 {
            scalar
        } else {
            // SAFETY: i - 1 < vl <= group_regs * elems_per_reg
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, i - 1, sew) }
        };
        // SAFETY: i < vl <= group_regs * elems_per_reg
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a vslide1down operation.
///
/// Element `vd[i] = vs2[i + 1]` for `i < vl - 1`; element `vd[vl - 1]` gets `scalar`.
///
/// Overlap between `vd` and `vs2` is permitted by the spec. When they share the same register
/// group base (exact overlap), ascending iteration is still correct: each write goes to byte range
/// `[i*sew, (i+1)*sew)` while the subsequent read comes from `[(i+1)*sew, (i+2)*sew)`. These
/// ranges are adjacent and non-overlapping, so writing element `i` never corrupts the source bytes
/// of element `i+1`.
///
/// # Safety
/// - `vd` and `vs2` are validly aligned (verified by caller); overlap is permitted.
/// - `vl <= group_regs * VLENB / sew_bytes`.
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_slide1down<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    scalar: u64,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let val = if i + 1 < vl {
            // SAFETY: i + 1 < vl <= group_regs * elems_per_reg
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, i + 1, sew) }
        } else {
            scalar
        };
        // SAFETY: i < vl <= group_regs * elems_per_reg
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute vrgather.vv: `vd[i] = (vs1[i] < vlmax) ? vs2[vs1[i]] : 0`.
///
/// # Safety
/// - `vd`, `vs2`, and `vs1` are validly aligned and mutually non-overlapping (verified by caller).
/// - `vl <= vlmax`.
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_rgather_vv<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    sew: Vsew,
    vlmax: u32,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: i < vl <= group_regs * elems_per_reg for vs1
        let index = unsafe { read_element_u64(ext_state.read_vregs(), vs1, i, sew) };
        let val = if index < u64::from(vlmax) {
            // SAFETY: index < vlmax <= group_regs * elems_per_reg for vs2
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, index as u32, sew) }
        } else {
            0u64
        };
        // SAFETY: i < vl <= group_regs * elems_per_reg for vd
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute vrgather.vx / vrgather.vi: all active elements get `vs2[index]` or `0`.
///
/// # Safety
/// - `vd` and `vs2` are validly aligned and non-overlapping (verified by caller).
/// - `vl <= vlmax`.
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_rgather_scalar<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    vlmax: u32,
    index: u64,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    // Pre-compute the gathered value; it's the same for all elements.
    let val = if index < u64::from(vlmax) {
        // SAFETY: index < vlmax <= group_regs * elems_per_reg for vs2
        unsafe { read_element_u64(ext_state.read_vregs(), vs2, index as u32, sew) }
    } else {
        0u64
    };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: i < vl <= group_regs * elems_per_reg for vd
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute vrgatherei16.vv: `vd[i] = (vs1_16[i] < vlmax) ? vs2[vs1_16[i]] : 0`.
///
/// `vs1` always uses EEW=16 regardless of SEW. `vl` must not exceed the index register group
/// capacity, i.e. `vl <= index_group_regs * VLENB / 2` (VLENB/2 = elems per register at EEW=16).
///
/// # Safety
/// - `vd`, `vs2`, and `vs1` are validly aligned and mutually non-overlapping (verified by caller).
/// - `vl <= vlmax` (for the data register group) AND `vl <= index_group_regs * VLENB / 2` (for the
///   index register group).
/// - When `vm=false`: `vd.to_bits() != 0`.
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_rgatherei16<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    sew: Vsew,
    vlmax: u32,
    index_group_regs: u8,
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
    // Maximum number of EEW=16 elements the index register group can hold.
    // Each register holds VLENB / 2 elements at EEW=16.
    let index_capacity = u32::from(index_group_regs) * (ExtState::VLENB / 2);
    // `vl` must not exceed either the data VLMAX or the index register group capacity.
    // Both bounds are guaranteed by the caller; this debug assertion catches misuse early.
    debug_assert!(
        vl <= vlmax && vl <= index_capacity,
        "vl={vl} exceeds vlmax={vlmax} or index_capacity={index_capacity}"
    );
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Read 16-bit index from vs1; EEW=16 always.
        // SAFETY: i < vl <= index_capacity = index_group_regs * (VLENB/2), so element i
        // fits within the index register group.
        let index = unsafe { read_element_u64(ext_state.read_vregs(), vs1, i, Vsew::E16) };
        let val = if index < u64::from(vlmax) {
            // SAFETY: index < vlmax <= group_regs * elems_per_reg for vs2
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, index as u32, sew) }
        } else {
            0u64
        };
        // SAFETY: i < vl <= group_regs * elems_per_reg for vd
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute vmerge.vvm / vmv.v.v.
///
/// When `vm=true` (vmv.v.v): all active elements `vstart..vl` get `vs1[i]`; vs2 unused.
/// When `vm=false` (vmerge.vvm): active elements where `v0[i]=1` get `vs1[i]`,
/// inactive elements get `vs2[i]`.
///
/// # Safety
/// - `vd` and `vs1` are validly aligned (verified by caller).
/// - When `vm=false`: `vs2` is validly aligned and `vd` does not overlap v0 (verified by caller).
/// - `vl <= group_regs * VLENB / sew_bytes`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_merge_vv<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`.
    // For vmv.v.v (vm=true) the mask is all-ones so snapshot_mask is still valid.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };
    for i in u32::from(vstart)..vl {
        let mask_set = mask_bit(&mask_buf, i);
        let val = if mask_set {
            // SAFETY: i < vl <= group_regs * elems_per_reg for vs1
            unsafe { read_element_u64(ext_state.read_vregs(), vs1, i, sew) }
        } else {
            // mask_set=false only reachable when vm=false (vmerge path).
            // SAFETY: i < vl <= group_regs * elems_per_reg for vs2
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) }
        };
        // SAFETY: i < vl <= group_regs * elems_per_reg for vd
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute vmerge.vxm / vmerge.vim / vmv.v.x / vmv.v.i.
///
/// When `vm=true`: all active elements `vstart..vl` get `scalar`; vs2 unused.
/// When `vm=false`: active elements where `v0[i]=1` get `scalar`,
/// inactive elements get `vs2[i]`.
///
/// # Safety
/// - `vd` is validly aligned (verified by caller).
/// - When `vm=false`: `vs2` is validly aligned and `vd` does not overlap v0 (verified by caller).
/// - `vl <= group_regs * VLENB / sew_bytes`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_merge_scalar<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    sew: Vsew,
    scalar: u64,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        let val = if mask_bit(&mask_buf, i) {
            scalar
        } else {
            // SAFETY: i < vl <= group_regs * elems_per_reg for vs2
            unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) }
        };
        // SAFETY: i < vl <= group_regs * elems_per_reg for vd
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, val);
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute vcompress.vm: pack active elements of vs2 (under vs1 mask) sequentially into vd.
///
/// `vs1` is treated as an explicit mask register (single register, not LMUL-grouped).
/// The output write index increments only for elements where `vs1[i]` is set.
/// vd must not overlap vs1 or vs2.
///
/// # Safety
/// - `vd`, `vs2` are validly aligned and non-overlapping (verified by caller).
/// - `vs1` does not overlap `vd` (verified by caller).
/// - `vl <= VLMAX`.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_compress<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vl: u32,
    sew: Vsew,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let mask_bytes = vl.div_ceil(u8::BITS) as usize;
    let vreg = ext_state.read_vregs();
    let mut vs1_buf = [0u8; { ExtState::VLENB as usize }];
    // SAFETY: mask_bytes <= VLENB since vl <= VLEN; vs1_base < 32
    unsafe {
        vs1_buf
            .get_unchecked_mut(..mask_bytes)
            .copy_from_slice(vreg.get(vs1).get_unchecked(..mask_bytes));
    }
    let mut out_idx = 0u32;
    for i in 0..vl {
        if !mask_bit(&vs1_buf, i) {
            continue;
        }
        // SAFETY: i < vl <= group_regs * elems_per_reg
        let val = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };
        // SAFETY: out_idx <= popcount(vs1[0..vl)) <= vl
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, out_idx, sew, val);
        }
        out_idx += 1;
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Copy `count` whole vector registers from `src_base` to `dst_base`.
///
/// No masking, no vtype dependency. Uses snapshot semantics: all source registers are read into
/// a stack buffer before any destination registers are written, giving correct memmove-style
/// behaviour for all overlap patterns (including partial overlap such as src=V0, dst=V1, count=2).
///
/// The stack allocation is at most 8 × VLENB bytes (`count <= 8` for vmv1r–vmv8r).
///
/// # Safety
/// - `dst_base + count <= 32` and `src_base + count <= 32` (verified by caller via alignment
///   checks).
/// - `dst_base % count == 0` and `src_base % count == 0` (verified by caller).
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_whole_reg_move<const VLENB: usize>(
    vregs: &mut VectorRegisterFile<VLENB>,
    dst_base: VReg,
    src_base: VReg,
    count: u8,
) {
    let count = usize::from(count);
    debug_assert!(count <= 8, "count must be <= 8 for vmvNr");
    // Snapshot all source registers before writing any destination registers.
    // This is correct for all overlap patterns without direction-dependent logic.
    let mut tmp = [[0u8; VLENB]; 8];
    for (k, item) in tmp.iter_mut().enumerate().take(count) {
        // SAFETY: Guaranteed by function contract
        let src = unsafe { VReg::from_bits(src_base.to_bits() + k as u8).unwrap_unchecked() };
        *item = *vregs.get(src);
    }
    for (k, item) in tmp.iter().enumerate().take(count) {
        // SAFETY: Guaranteed by function contract
        let dst = unsafe { VReg::from_bits(dst_base.to_bits() + k as u8).unwrap_unchecked() };
        *vregs.get_mut(dst) = *item;
    }
}
