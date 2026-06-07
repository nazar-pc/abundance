//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::{VectorRegisterFile, VectorRegistersExt};
use crate::v::zve64x::load::zve64x_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zve64x::zve64x_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Check that `vreg` (`vd`/`vs`) is aligned to `group_regs` and fits within `[0, 32)`
#[inline(always)]
#[doc(hidden)]
pub fn check_vreg_group_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vreg: VReg,
    group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vreg_idx = vreg.to_bits();
    if !vreg_idx.is_multiple_of(group_regs) || vreg_idx + group_regs > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Check mask-destination / source overlap constraint for compare instructions.
///
/// Per RVV §11.8: a mask destination register may overlap a source register group only when
/// the source group occupies a single register (LMUL ≤ 1, i.e. `group_regs == 1`). Otherwise
/// the encoding is reserved and raises an illegal instruction.
#[inline(always)]
#[doc(hidden)]
pub fn check_mask_dest_no_overlap<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vd: VReg,
    src_base: VReg,
    group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    if group_regs > 1 {
        let vd_idx = vd.to_bits();
        let src = src_base.to_bits();
        if vd_idx >= src && vd_idx < src + group_regs {
            return Err(ExecutionError::IllegalInstruction {
                address: program_counter.old_pc(INSTRUCTION_SIZE),
            });
        }
    }
    Ok(())
}

/// Read a SEW-wide element from register group `[base_reg, base_reg + group_regs)` as `u64`.
///
/// Element `elem_i` occupies bytes at:
///   - register `base_reg + elem_i / elems_per_reg`
///   - byte offset `(elem_i % elems_per_reg) * sew_bytes`
///
/// The value is zero-extended to `u64`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / sew_bytes) < 32` must hold.
#[inline(always)]
pub(in super::super) unsafe fn read_element_u64<const VLENB: usize>(
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
    let reg = vregs
        .get(unsafe { VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap_unchecked() });
    // SAFETY: `byte_off + sew_bytes <= VLENB` because `byte_off` is at most
    // `(elems_per_reg - 1) * sew_bytes = VLENB - sew_bytes`
    let src = unsafe { reg.get_unchecked(byte_off..byte_off + sew_bytes) };
    let mut buf = [0u8; 8];
    // SAFETY: `sew_bytes <= 8` for all `Vsew` variants
    unsafe { buf.get_unchecked_mut(..sew_bytes) }.copy_from_slice(src);
    u64::from_le_bytes(buf)
}

/// Write a SEW-wide element (low `sew_bytes` of `value`) into register group
/// `[base_reg, base_reg + group_regs)` at element index `elem_i`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / sew_bytes) < 32` must hold.
#[inline(always)]
pub(in super::super) unsafe fn write_element_u64<const VLENB: usize>(
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
    let reg = vregs
        .get_mut(unsafe { VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap_unchecked() });
    // SAFETY: `byte_off + sew_bytes <= VLENB` - same argument as `read_element_u64`.
    // `sew_bytes <= 8` for all `Vsew` variants.
    let dst = unsafe { reg.get_unchecked_mut(byte_off..byte_off + sew_bytes) };
    // SAFETY: `sew_bytes <= 8` for all `Vsew` variants
    dst.copy_from_slice(unsafe { buf.get_unchecked(..sew_bytes) });
}

/// Write one mask bit (the comparison result for element `elem_i`) into register `vd`.
///
/// Bits are stored LSB-first: element `i` lives at byte `i / 8`, bit `i % 8`.
/// Only the target bit is modified; all other bits are undisturbed (tail-undisturbed semantics
/// required for mask destinations per spec §5.3).
///
/// # Safety
/// `elem_i / 8 < VLENB` must hold, i.e. `elem_i < VLEN`. This is guaranteed when
/// `elem_i < vl <= VLMAX <= VLEN`.
#[inline(always)]
pub(in super::super) unsafe fn write_mask_bit<const VLENB: usize>(
    vregs: &mut VectorRegisterFile<VLENB>,
    vd: VReg,
    elem_i: u32,
    result: bool,
) {
    let byte_idx = (elem_i / u8::BITS) as usize;
    let bit_idx = elem_i % u8::BITS;
    // SAFETY: `byte_idx < VLENB` by the caller's precondition
    let byte = unsafe { vregs.get_mut(vd).get_unchecked_mut(byte_idx) };
    if result {
        *byte |= 1 << bit_idx;
    } else {
        *byte &= !(1 << bit_idx);
    }
}

/// Operand source
#[derive(Debug)]
#[doc(hidden)]
pub enum OpSrc {
    /// Vector-vector: source register index
    Vreg(VReg),
    /// Vector-scalar: scalar value (sign- or zero-extended to u64)
    Scalar(u64),
}

/// Execute a single-width element-wise arithmetic operation over `vstart..vl`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew)` and returns the `u64` result (only the
/// low `sew.bits_width()` are written back).
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0` and `vd.to_bits() + group_regs <= 32` (verified by caller)
/// - `src` register (when `OpSrc::Vreg`) satisfies the same alignment (verified by caller)
/// - `vl <= group_regs * VLENB / sew_bytes` (all `vl` elements fit within the register group)
/// - When `vm=false`: `vd.to_bits() != 0` (vd does not overlap v0)
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_arith_op<Reg, ExtState, CustomError, F>(
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
    F: Fn(u64, u64, Vsew) -> u64,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }

        // SAFETY: `vs2 % group_regs == 0` and `i < vl <= group_regs * elems_per_reg`, so
        // `vs2 + i / elems_per_reg < vs2 + group_regs <= 32`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };

        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same argument as vs2
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };

        let result = op(a, b, sew);

        // SAFETY: `vd % group_regs == 0` and `i < vl <= group_regs * elems_per_reg`, so
        // `vd + i / elems_per_reg < vd + group_regs <= 32`
        unsafe {
            write_element_u64(ext_state.write_vregs(), vd, i, sew, result);
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Execute a single-width element-wise integer compare over `vstart..vl`, writing one result
/// bit per element into the mask register `vd`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew) -> bool`.
///
/// Mask destination tail bits (indices `>= vl`) are always left undisturbed per spec §5.3,
/// regardless of `vta`. Only bits in `vstart..vl` are written.
///
/// # Safety
/// - `vs2.to_bits() % group_regs == 0` and `vs2.to_bits() + group_regs <= 32` (verified by caller)
/// - `src` register (when `OpSrc::Vreg`) satisfies the same alignment (verified by caller)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN` (so every element index fits within the mask register)
#[inline(always)]
#[doc(hidden)]
pub unsafe fn execute_compare_op<Reg, ExtState, CustomError, F>(
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
    F: Fn(u64, u64, Vsew) -> bool,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        // When masked, inactive elements in the destination mask register are left undisturbed
        // (spec §12.8: "mask register results follow mask-undisturbed policy")
        if !mask_bit(&mask_buf, i) {
            continue;
        }

        // SAFETY: same argument as in `execute_arith_op`
        let a = unsafe { read_element_u64(ext_state.read_vregs(), vs2, i, sew) };

        let b = match src {
            OpSrc::Vreg(vs1_base) => {
                // SAFETY: same argument as vs2
                unsafe { read_element_u64(ext_state.read_vregs(), vs1_base, i, sew) }
            }
            OpSrc::Scalar(val) => val,
        };

        let result = op(a, b, sew);

        // SAFETY: `i < vl <= VLMAX <= VLEN`, so `i / 8 < VLEN / 8 = VLENB`
        unsafe {
            write_mask_bit(ext_state.write_vregs(), vd, i, result);
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
}

/// Sign-extend the low `sew.bits_width()` of `val` to a full `i64`
#[inline(always)]
#[doc(hidden)]
pub fn sign_extend(val: u64, sew: Vsew) -> i64 {
    let shift = u64::BITS - u32::from(sew.bits_width());
    (val.cast_signed() << shift) >> shift
}

/// Mask off the upper bits of a `u64` to leave only the low `sew.bits_width()`.
///
/// Used for unsigned arithmetic and comparisons where only the SEW-wide portion is significant. For
/// SEW = 64 this is a no-op (all bits are significant).
#[inline(always)]
#[doc(hidden)]
pub fn sew_mask(sew: Vsew) -> u64 {
    if u32::from(sew.bits_width()) == u64::BITS {
        u64::MAX
    } else {
        (1u64 << sew.bits_width()) - 1
    }
}
