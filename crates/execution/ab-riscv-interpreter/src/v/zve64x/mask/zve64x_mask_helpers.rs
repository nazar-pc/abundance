//! Opaque helpers for Zve64x extension

use crate::RegisterFile;
use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::arith::zve64x_arith_helpers::{write_element_u64, write_mask_bit};
use crate::v::zve64x::load::zve64x_load_helpers::{mask_bit, snapshot_mask};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Execute a mask-register logical operation (§16.1).
///
/// Computes the result for the body elements `[vstart, vl)` only. Prestart bits `[0, vstart)`
/// are left undisturbed, and tail bits `[vl, VLEN)` follow the tail-agnostic policy, realised
/// here as undisturbed (a permitted agnostic implementation and the one the reference model
/// produces). `op` receives `(vs2_bit: bool, vs1_bit: bool) -> bool`.
///
/// # Safety
/// `vd`, `vs2`, and `vs1` are valid register indices (guaranteed by `VReg`).
/// `vl <= VLEN`, so `(vl - 1) / 8 < VLENB`; `vstart <= vl` by the architectural invariant.
/// The operation snapshots both sources before writing, so `vd` may safely overlap either source.
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_mask_logical_op<Reg, ExtState, CustomError, F>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    op: F,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
    F: Fn(bool, bool) -> bool,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    // Snapshot both sources before writing to handle vd overlapping vs2 or vs1
    // SAFETY: `vs2.bits() < 32`; `VReg` values are always in `0..32`
    let vs2_snap = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    // SAFETY: `vs1.bits() < 32`; `VReg` values are always in `0..32`
    let vs1_snap = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs1.bits())) };
    // Body elements [vstart, vl): compute the logical operation bit-by-bit. Prestart bits
    // [0, vstart) and tail bits [vl, VLEN) are left undisturbed.
    for i in u32::from(vstart)..vl {
        let a = mask_bit(&vs2_snap, i);
        let b = mask_bit(&vs1_snap, i);
        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vreg(), vd, i, op(a, b));
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute `vcpop.m`: count set bits in vs2 for active elements `0..vl`, write result to `rd`.
///
/// Per spec §16.2: `rd` receives the number of mask bits set in `vs2`, considering only elements
/// `vstart..vl` that are active under the mask. For elements `< vstart`, they are not counted.
///
/// # Safety
/// - `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
/// - `vstart <= vl`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vcpop<Reg, Regs, ExtState, CustomError>(
    regs: &mut Regs,
    ext_state: &mut ExtState,
    rd: Reg,
    vs2: VReg,
    vm: bool,
) where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    // SAFETY: `vs2` is a valid VReg index (< 32)
    let vs2_reg = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    let mut count = 0u32;
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        if mask_bit(&vs2_reg, i) {
            count += 1;
        }
    }
    regs.write(rd, Reg::Type::from(count));
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute `vfirst.m`: find the index of the first set bit in vs2 for active elements `0..vl`,
/// write result (or -1 if none) to `rd`.
///
/// Per spec §16.3: `rd` receives the element index of the lowest-numbered active set bit, or
/// `-1` (all-ones) if no active element of vs2 is set.
///
/// # Safety
/// - `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
/// - `vstart <= vl`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vfirst<Reg, Regs, ExtState, CustomError>(
    regs: &mut Regs,
    ext_state: &mut ExtState,
    rd: Reg,
    vs2: VReg,
    vm: bool,
) where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    // SAFETY: `vs2` is a valid VReg index (< 32)
    let vs2_reg = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    // -1 encoded as all-ones for the register width; `Into<u64>` on XLEN-wide type then back
    let not_found = u64::MAX;
    let mut result = not_found;
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        if mask_bit(&vs2_reg, i) {
            result = u64::from(i);
            break;
        }
    }
    // Write -1 (all-ones for XLEN bits) or the found index.
    // The spec requires -1 as a signed XLEN-wide value, meaning all bits set.
    // `!Reg::Type::from(0)` produces all-ones for both u32 (RV32) and u64 (RV64)
    // without depending on `From<u64>` (which is not in the `Register` trait bounds).
    // For the found index, element indices fit in u32 since vl <= VLEN <= 2^32.
    let reg_value = if result == not_found {
        !Reg::Type::from(0u8)
    } else {
        Reg::Type::from(result as u32)
    };
    regs.write(rd, reg_value);
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute `vmsbf.m`: set all mask bits before (not including) the first set bit of vs2.
///
/// Per spec §16.4: for each element `i` in `vstart..vl`, if no prior active set bit exists in
/// vs2, the destination bit is set; once the first set bit in vs2 is encountered, all subsequent
/// destination bits are cleared.
///
/// Inactive elements (masked off) are left undisturbed. Tail elements are undisturbed.
///
/// # Safety
/// - `vd` does not overlap `vs2` (checked by caller)
/// - `vm=false` implies `vd != v0` (checked by caller)
/// - `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vmsbf<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    vl: u32,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    // SAFETY: `vs2.bits() < 32`; `VReg` values are always in `0..32`
    let vs2_snap = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    let mut found_first = false;
    for i in 0..vl {
        // Inactive elements: undisturbed
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let vs2_bit = mask_bit(&vs2_snap, i);
        // vmsbf: set bits strictly *before* the first set bit; clear from first set bit onward
        let result = !found_first && !vs2_bit;
        if vs2_bit {
            found_first = true;
        }
        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vreg(), vd, i, result);
        }
    }
    ext_state.mark_vs_dirty();
    // vstart is already zero, doesn't need to be reset
}

/// Execute `vmsof.m`: set only the first set bit position of vs2, clear all others.
///
/// Per spec §16.5: the destination bit is set only at the lowest-numbered active element where
/// vs2 has a set bit. All other active destination bits are cleared.
///
/// # Safety
/// Same as [`execute_vmsbf`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vmsof<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    vl: u32,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    // SAFETY: `vs2.bits() < 32`; `VReg` values are always in `0..32`
    let vs2_snap = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    let mut found_first = false;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let vs2_bit = mask_bit(&vs2_snap, i);
        // vmsof: set only the first set bit position; clear all others (including after first)
        let result = !found_first && vs2_bit;
        if vs2_bit && !found_first {
            found_first = true;
        }
        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vreg(), vd, i, result);
        }
    }
    ext_state.mark_vs_dirty();
    // vstart is already zero, doesn't need to be reset
}

/// Execute `vmsif.m`: set all mask bits up to and including the first set bit of vs2.
///
/// Per spec §16.6: for each active element, the destination bit is set if no prior active set bit
/// in vs2 has been seen yet *or* the current element itself is set; it is cleared once a set bit
/// has been seen and the current element is past it.
///
/// # Safety
/// Same as [`execute_vmsbf`].
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vmsif<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
    vl: u32,
) where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    CustomError: fmt::Debug,
{
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    // SAFETY: `vs2.bits() < 32`; `VReg` values are always in `0..32`
    let vs2_snap = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    let mut found_first = false;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let vs2_bit = mask_bit(&vs2_snap, i);
        // vmsif: set bits up to *and including* the first set bit; clear elements past it
        let result = !found_first;
        if vs2_bit {
            found_first = true;
        }
        // SAFETY: `i < vl <= VLEN`, so `i / 8 < VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vreg(), vd, i, result);
        }
    }
    ext_state.mark_vs_dirty();
    // vstart is already zero, doesn't need to be reset
}

/// Execute `viota.m`: for each active element `i`, write the popcount of set bits in vs2 at
/// positions `0..i` (strictly before `i`) as a SEW-wide integer into `vd[i]`.
///
/// Per spec §16.8: this instruction honors the source mask; inactive mask elements of vs2 are
/// treated as zero for the prefix sum. Inactive destination elements follow the mask-agnostic
/// policy (here implemented as undisturbed, which is a permitted realisation).
///
/// If SEW is too narrow to hold the prefix count, the value wraps (truncates to SEW) via
/// [`write_element_u64()`]; the spec does not raise an exception for this case.
///
/// The caller must reject `vstart != 0` before invocation (spec §16.8 mandatory trap).
///
/// # Safety
/// - `vd` does not overlap `vs2` (checked by caller)
/// - `vm=false` implies `vd != v0` (checked by caller)
/// - `vd.bits() % group_regs == 0` and `vd.bits() + group_regs <= 32` (checked by caller)
/// - `vl <= VLMAX`; `vl <= VLEN`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_viota<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
    vs2: VReg,
    vm: bool,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    // SAFETY: `vs2.bits() < 32`; `VReg` values are always in `0..32`
    let vs2_snap = *unsafe { ext_state.read_vreg().get_unchecked(usize::from(vs2.bits())) };
    // Per spec §16.8: inactive vs2 elements are treated as zero for the prefix sum.
    // The prefix count advances only when the execution mask is active AND the
    // corresponding vs2 bit is set.
    let mut prefix_count = 0u64;
    for i in 0..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vd + i / elems_per_reg < 32` by caller's alignment + vl preconditions
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd, i, sew, prefix_count);
        }
        if mask_bit(&vs2_snap, i) {
            prefix_count += 1;
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute `vid.v`: write the element index `i` as a SEW-wide integer into `vd[i]` for each
/// active element in `vstart..vl`.
///
/// Per spec §16.9: inactive elements are left undisturbed (mask-undisturbed policy).
///
/// # Safety
/// - `vm=false` implies `vd != v0` (checked by caller)
/// - `vd.bits() % group_regs == 0` and `vd.bits() + group_regs <= 32` (checked by caller)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_vid<Reg, ExtState, CustomError>(
    ext_state: &mut ExtState,
    vd: VReg,
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
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vd + i / elems_per_reg < 32` by caller's alignment + vl preconditions
        unsafe {
            write_element_u64(ext_state.write_vreg(), vd, i, sew, u64::from(i));
        }
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}
