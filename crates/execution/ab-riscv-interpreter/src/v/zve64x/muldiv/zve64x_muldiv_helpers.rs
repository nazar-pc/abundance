//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zve64x::arith::zve64x_arith_helpers::{
    OpSrc, check_vreg_group_alignment, sew_mask, sign_extend,
};
use crate::v::zve64x::arith::zve64x_arith_helpers::{read_element_u64, write_element_u64};
use crate::v::zve64x::fixed_point::zve64x_fixed_point_helpers::read_wide_element_u64;
use crate::v::zve64x::load::zve64x_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zve64x::zve64x_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, InterpreterState, ProgramCounter, VirtualMemory};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Compute the destination register count for a widening operation (`EMUL = 2 × LMUL`).
///
/// Returns `None` when the resulting EMUL falls outside the legal range `[1/8, 8]`, i.e. when
/// `LMUL` is already `M8` (EMUL would be 16) or the caller asks for a multiplication factor that
/// pushes the fraction past the legal lower bound.
///
/// The register count returned is `max(1, EMUL)`: fractional EMUL values (1/2, 1/4) still occupy
/// exactly one physical register.
#[inline(always)]
#[doc(hidden)]
pub fn widening_dest_register_count(vlmul: Vlmul) -> Option<u8> {
    let (lmul_num, lmul_den) = vlmul.as_fraction();
    // EMUL = 2 × LMUL = (2 * lmul_num) / lmul_den
    let emul_num = 2u8.checked_mul(lmul_num)?;
    let emul_den = lmul_den;
    // Reduce the fraction by GCD (both are powers of two so min works as GCD)
    let g = emul_num.min(emul_den);
    let (n, d) = (emul_num / g, emul_den / g);
    // Legal EMUL fractions: 1/8, 1/4, 1/2, 1, 2, 4, 8
    let legal = matches!(
        (n, d),
        (1, 8) | (1, 4) | (1, 2) | (1, 1) | (2, 1) | (4, 1) | (8, 1)
    );
    if !legal {
        return None;
    }
    // Register count: max(1, n/d) = n when d==1, else 1
    Some(if d > 1 { 1 } else { n })
}

/// Check that a narrower source register group does not overlap the wider destination group.
///
/// For widening instructions `vd` occupies `dest_group_regs` registers (which is
/// [`widening_dest_register_count()`] of the source LMUL); `vs` occupies `src_group_regs`.
/// The spec prohibits any overlap between them.
#[inline(always)]
#[doc(hidden)]
pub fn check_no_widening_overlap<Reg, ExtState, Memory, PC, IH, CustomError>(
    state: &InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs: VReg,
    dest_group_regs: u8,
    src_group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    [(); Reg::N]:,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vd_start = vd.bits();
    let vd_end = vd_start + dest_group_regs;
    let vs_start = vs.bits();
    let vs_end = vs_start + src_group_regs;
    // Overlap when the intervals intersect
    if vs_start < vd_end && vd_start < vs_end {
        return Err(ExecutionError::IllegalInstruction {
            address: state.instruction_fetcher.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Write a 2*SEW-wide element into the widened destination register group at element index
/// `elem_i`.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / (2*sew_bytes)) < 32` must hold.
#[inline(always)]
unsafe fn write_wide_element_u64<const VLENB: usize>(
    vreg: &mut [[u8; VLENB]; 32],
    base_reg: u8,
    elem_i: u32,
    sew: Vsew,
    value: u64,
) {
    let wide_bytes = usize::from(sew.bytes()) * 2;
    let elems_per_reg = VLENB / wide_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * wide_bytes;
    let buf = value.to_le_bytes();
    // SAFETY: `base_reg + reg_off < 32` by caller's precondition
    let reg = unsafe { vreg.get_unchecked_mut(usize::from(base_reg) + reg_off) };
    // SAFETY: `byte_off + wide_bytes <= VLENB`; `wide_bytes <= 8` for SEW < 64
    let dst = unsafe { reg.get_unchecked_mut(byte_off..byte_off + wide_bytes) };
    // SAFETY: `wide_bytes <= 8` because SEW < 64 is enforced before widening ops are called
    dst.copy_from_slice(unsafe { buf.get_unchecked(..wide_bytes) });
}

/// Execute a single-width element-wise arithmetic operation over `vstart..vl`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew)` and returns the `u64` result.
/// Only the low `sew.bytes()` of the result are written back.
///
/// # Safety
/// - `vd` and source register alignment verified by caller
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_arith_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    let vs2_base = vs2.bits();
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let a =
            unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(vs2_base), i, sew) };
        let b = match &src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(vs1_base) => unsafe {
                read_element_u64(state.ext_state.read_vreg(), usize::from(*vs1_base), i, sew)
            },
            OpSrc::Scalar(val) => *val,
        };
        let result = op(a, b, sew);
        // SAFETY: register bounds verified by caller
        unsafe {
            write_element_u64(state.ext_state.write_vreg(), vd_base, i, sew, result);
        }
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a single-width widening operation over `vstart..vl`.
///
/// Reads SEW-wide elements from `vs2` and `src`, computes `op`, and writes a 2*SEW-wide result
/// into `vd`.
///
/// # Safety
/// - `vd` uses `dest_group_regs` registers (result of `widening_dest_register_count()`); alignment
///   and non-overlap verified by caller
/// - `vl <= src_group_regs * VLENB / sew_bytes`
/// - SEW < 64 verified by caller (so 2*SEW <= 64 and fits in u64)
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widening_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    let vs2_base = vs2.bits();
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let a =
            unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(vs2_base), i, sew) };
        let b = match &src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(vs1_base) => unsafe {
                read_element_u64(state.ext_state.read_vreg(), usize::from(*vs1_base), i, sew)
            },
            OpSrc::Scalar(val) => *val,
        };
        let result = op(a, b, sew);
        // SAFETY: vd has dest_group_regs registers; element `i` fits within them because
        // `vl <= src_group_regs * VLENB / sew_bytes` and dest stores at 2*SEW width so
        // `i < dest_group_regs * VLENB / (2*sew_bytes)`
        unsafe {
            write_wide_element_u64(state.ext_state.write_vreg(), vd_base, i, sew, result);
        }
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a single-width multiply-add where the first multiplier is a vector register group.
///
/// `op` receives `(acc: u64, a: u64, b: u64, sew: Vsew)` where `acc` is the current `vd[i]`,
/// `a` is the element from `a_reg`, and `b` is the element from `src`. Returns the new `vd[i]`.
///
/// # Safety
/// - `vd`, `a_reg`, and `src` register alignment verified by caller
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_muladd_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    a_reg: u8,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let acc =
            unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(vd_base), i, sew) };
        // SAFETY: register bounds verified by caller
        let a =
            unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(a_reg), i, sew) };
        let b = match &src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe {
                read_element_u64(state.ext_state.read_vreg(), usize::from(*b_reg), i, sew)
            },
            OpSrc::Scalar(val) => *val,
        };
        let result = op(acc, a, b, sew);
        // SAFETY: register bounds verified by caller
        unsafe {
            write_element_u64(state.ext_state.write_vreg(), vd_base, i, sew, result);
        }
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a single-width multiply-add where the first multiplier is a scalar.
///
/// Analogous to [`execute_muladd_op`] but `a` is a fixed scalar instead of a register element.
///
/// # Safety
/// Same as [`execute_muladd_op`], minus constraints on `a_reg`.
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_muladd_scalar_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    scalar: u64,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let acc =
            unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(vd_base), i, sew) };
        let b = match &src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe {
                read_element_u64(state.ext_state.read_vreg(), usize::from(*b_reg), i, sew)
            },
            OpSrc::Scalar(val) => *val,
        };
        let result = op(acc, scalar, b, sew);
        // SAFETY: register bounds verified by caller
        unsafe {
            write_element_u64(state.ext_state.write_vreg(), vd_base, i, sew, result);
        }
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a widening multiply-add where the first multiplier is a vector register group.
///
/// Reads SEW-wide `acc` from the widened `vd` group, SEW-wide `a` from `a_reg`, and SEW-wide
/// `b` from `src`. Writes a 2*SEW-wide result back into `vd`.
///
/// `op` receives `(acc: u64, a: u64, b: u64, sew: Vsew)`.
///
/// # Safety
/// - `vd` uses `dest_group_regs` registers (result of `widening_dest_register_count()`); alignment
///   and non-overlap verified by caller
/// - SEW < 64 verified by caller
/// - When `vm=false`: `vd.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widening_muladd_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    a_reg: u8,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Read the existing 2*SEW accumulator from vd
        // SAFETY: vd has dest_group_regs registers; element `i` fits within them (see
        // `execute_widening_op` for the bound argument)
        let acc = unsafe {
            read_wide_element_u64(state.ext_state.read_vreg(), usize::from(vd_base), i, sew)
        };
        // SAFETY: register bounds verified by caller
        let a =
            unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(a_reg), i, sew) };
        let b = match &src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe {
                read_element_u64(state.ext_state.read_vreg(), usize::from(*b_reg), i, sew)
            },
            OpSrc::Scalar(val) => *val,
        };
        let result = op(acc, a, b, sew);
        // SAFETY: same as acc read above
        unsafe {
            write_wide_element_u64(state.ext_state.write_vreg(), vd_base, i, sew, result);
        }
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a widening multiply-add where the first multiplier is a scalar.
///
/// Analogous to [`execute_widening_muladd_op`] but `a` is a fixed scalar.
///
/// # Safety
/// Same as [`execute_widening_muladd_op`], minus constraints on `a_reg`.
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widening_muladd_scalar_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    scalar: u64,
    src: OpSrc,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    [(); Reg::N]:,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };
    let vd_base = vd.bits();
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: vd has dest_group_regs registers; element `i` fits within them (see
        // `execute_widening_op` for the bound argument)
        let acc = unsafe {
            read_wide_element_u64(state.ext_state.read_vreg(), usize::from(vd_base), i, sew)
        };
        let b = match &src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe {
                read_element_u64(state.ext_state.read_vreg(), usize::from(*b_reg), i, sew)
            },
            OpSrc::Scalar(val) => *val,
        };
        let result = op(acc, scalar, b, sew);
        // SAFETY: same as acc read above
        unsafe {
            write_wide_element_u64(state.ext_state.write_vreg(), vd_base, i, sew, result);
        }
    }
    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Signed × signed high half.
///
/// Both operands are sign-extended to i64, multiplied as i128, and the upper SEW bits of the
/// 2*SEW product are returned (zero-extended to u64 for writeback into a SEW-wide element slot).
#[inline(always)]
#[doc(hidden)]
pub fn mulh_ss(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = i128::from(sign_extend(a, sew));
    let sb = i128::from(sign_extend(b, sew));
    let product = sa.wrapping_mul(sb);
    // Extract bits [2*SEW-1 : SEW] of the product
    let high = (product >> u32::from(sew.bits())).cast_unsigned() as u64;
    high & sew_mask(sew)
}

/// Unsigned × unsigned high half
#[inline(always)]
#[doc(hidden)]
pub fn mulhu_uu(a: u64, b: u64, sew: Vsew) -> u64 {
    let ua = u128::from(a & sew_mask(sew));
    let ub = u128::from(b & sew_mask(sew));
    let product = ua.wrapping_mul(ub);
    let high = (product >> u32::from(sew.bits())) as u64;
    high & sew_mask(sew)
}

/// Signed × unsigned high half.
///
/// `a` (vs2) is the signed operand; `b` (vs1/rs1) is the unsigned operand.
#[inline(always)]
#[doc(hidden)]
pub fn mulhsu_su(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = i128::from(sign_extend(a, sew));
    let ub = u128::from(b & sew_mask(sew));
    // Compute signed × unsigned as i128 to preserve sign
    let product = sa.wrapping_mul(ub.cast_signed());
    let high = (product >> u32::from(sew.bits())).cast_unsigned() as u64;
    high & sew_mask(sew)
}

/// Signed divide with division-by-zero and signed-overflow semantics from the RISC-V V spec §12.11.
///
/// - Division by zero: result = all-ones (i.e., −1 as signed SEW-wide integer)
/// - Signed overflow (MIN / −1): result = MIN (i.e., `1 << (SEW-1)`)
#[inline(always)]
#[doc(hidden)]
pub fn sdiv(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Division by zero: return all-ones in the SEW-wide slot (= −1 signed)
    if sb == 0 {
        return sew_mask(sew);
    }
    // Signed overflow: MIN / -1 returns MIN
    let sew_min = i64::MIN >> (u64::BITS - u32::from(sew.bits()));
    if sa == sew_min && sb == -1 {
        return sew_min.cast_unsigned() & sew_mask(sew);
    }
    (sa / sb).cast_unsigned() & sew_mask(sew)
}

/// Signed remainder with division-by-zero and signed-overflow semantics from the RISC-V V spec
/// §12.11.
///
/// - Division by zero: remainder = dividend
/// - Signed overflow (MIN % −1): remainder = 0
#[inline(always)]
#[doc(hidden)]
pub fn srem(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Division by zero: remainder = dividend
    if sb == 0 {
        return a & sew_mask(sew);
    }
    // Signed overflow: MIN % -1 = 0
    let sew_min = i64::MIN >> (u64::BITS - u32::from(sew.bits()));
    if sa == sew_min && sb == -1 {
        return 0;
    }
    (sa % sb).cast_unsigned() & sew_mask(sew)
}
