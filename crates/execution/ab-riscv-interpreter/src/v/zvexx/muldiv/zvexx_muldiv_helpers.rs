//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::VectorRegistersExt;
pub use crate::v::zvexx::arith::zvexx_arith_helpers::{
    OpSrc, check_vreg_group_alignment, sew_mask, sign_extend,
};
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use crate::v::zvexx::zvexx_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, PackedAddress, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Whether a widening operation is representable for the given `SEW` and `ELEN`.
///
/// Widening instructions produce a `2*SEW` result, and an EEW greater than `ELEN` is reserved by
/// the RISC-V "V" spec §3.4.2 for *every* implementation. This is therefore not an extension-level
/// restriction like the one on the high-half multiplies in Zve64x: no vector extension makes
/// `SEW == ELEN` legal for a widening instruction.
///
/// This also underpins the safety preconditions of [`execute_widening_op()`],
/// [`execute_widening_muladd_op()`] and [`execute_widening_muladd_scalar_op()`], because
/// `ELEN <= 64` means `2*SEW <= 64` and the wide element fits in a `u64`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn widening_eew_supported(sew: Vsew, elen: Elen) -> bool {
    u32::from(sew.bits_width()) * 2 <= u32::from(elen)
}

/// Check that a narrower source register group does not *illegally* overlap the wider destination
/// group of a widening instruction.
///
/// For widening instructions `vd` occupies `dest_group_regs` registers (which is
/// [`widening_dest_register_count()`] of the source LMUL); `vs` occupies `src_group_regs`.
///
/// Per the RISC-V "V" spec §5.2, because the destination EEW (`2*SEW`) is greater than the source
/// EEW (`SEW`), the source group *may* overlap the destination group, but only when both of the
/// following hold:
/// - the source EMUL is at least 1, and
/// - the overlap is in the highest-numbered part of the destination register group, i.e. the source
///   occupies exactly the top `src_group_regs` registers of the destination group.
///
/// When the source EMUL is at least 1, `dest_group_regs == 2 * src_group_regs`, so
/// `dest_group_regs > src_group_regs` is an equivalent test for "source EMUL >= 1": a fractional
/// source EMUL (`< 1`) yields `dest_group_regs == src_group_regs == 1`, in which case no overlap is
/// ever legal. Any overlap that is not the legal "source in the highest-numbered part" form is
/// rejected as an illegal instruction.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn check_no_widening_overlap<Reg, Memory, PC>(
    program_counter: &PC,
    vd: VReg,
    vs: VReg,
    dest_group_regs: VRegGroupSize,
    src_group_regs: VRegGroupSize,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    let dest_group_regs = dest_group_regs.get();
    let src_group_regs = src_group_regs.get();
    let vd_start = vd.to_bits();
    let vd_end = vd_start + dest_group_regs;
    let vs_start = vs.to_bits();
    let vs_end = vs_start + src_group_regs;
    // Disjoint register groups are always fine
    if vs_start >= vd_end || vd_start >= vs_end {
        return Ok(());
    }
    // The groups overlap. This is legal only when the source EMUL is at least 1
    // (`dest_group_regs > src_group_regs`) and the source occupies exactly the highest-numbered
    // part of the destination group (`vs_start == vd_end - src_group_regs`).
    if dest_group_regs > src_group_regs && vs_start == vd_end - src_group_regs {
        return Ok(());
    }

    cold_path();
    Err(ExecutionError::IllegalInstruction {
        address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
    })
}

/// Execute a single-width element-wise arithmetic operation over `vstart..vl`.
///
/// `op` receives `(vs2_elem: u64, src_elem: u64, sew: Vsew)` and returns the `u64` result.
/// Only the low `sew.bytes()` of the result are written back.
///
/// # Safety
/// - `vd` and source register alignment verified by caller
/// - `vl <= group_regs * VLEN.bytes() / sew_bytes`
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_arith_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, Vsew) -> u64,
{
    let vl = env.vl();
    let vstart = env.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`
    let mask_buf = unsafe { snapshot_mask(env.read_vregs(), vm, vl) };
    for i in vstart.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let a = unsafe { env.read_vregs().read_element(vs2, i, sew) };
        let b = match src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(vs1_base) => unsafe { env.read_vregs().read_element(vs1_base, i, sew) },
            OpSrc::Scalar(val) => val,
        };
        let result = op(a, b, sew);
        // SAFETY: register bounds verified by caller
        unsafe {
            env.write_vregs().write_element(vd, i, sew, result);
        }
    }
    env.mark_vs_dirty();
    env.reset_vstart();
}

/// Execute a single-width widening operation over `vstart..vl`.
///
/// Reads SEW-wide elements from `vs2` and `src`, computes `op`, and writes a 2*SEW-wide result
/// into `vd`.
///
/// # Safety
/// - `vd` uses `dest_group_regs` registers (result of `widening_dest_register_count()`); alignment
///   and non-overlap verified by caller
/// - `vl <= src_group_regs * VLEN.bytes() / sew_bytes`
/// - `2*SEW <= ELEN` verified by caller via [`widening_eew_supported()`], so the 2*SEW result fits
///   in a `u64`; this holds for every implementation because an EEW may not exceed ELEN
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_widening_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    vs2: VReg,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, Vsew) -> u64,
{
    // SAFETY: `2 * SEW <= ELEN` is a precondition, so the double width exists
    let wide_sew = unsafe { sew.double_width().unwrap_unchecked() };
    let vl = env.vl();
    let vstart = env.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`
    let mask_buf = unsafe { snapshot_mask(env.read_vregs(), vm, vl) };
    for i in vstart.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let a = unsafe { env.read_vregs().read_element(vs2, i, sew) };
        let b = match src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(vs1_base) => unsafe { env.read_vregs().read_element(vs1_base, i, sew) },
            OpSrc::Scalar(val) => val,
        };
        let result = op(a, b, sew);
        // SAFETY: vd has dest_group_regs registers; element `i` fits within them because
        // `vl <= src_group_regs * VLEN.bytes() / sew_bytes` and dest stores at 2*SEW width so
        // `i < dest_group_regs * VLEN.bytes() / (2*sew_bytes)`; `2*SEW <= ELEN <= 64` by caller
        unsafe {
            env.write_vregs().write_element(vd, i, wide_sew, result);
        }
    }
    env.mark_vs_dirty();
    env.reset_vstart();
}

/// Execute a single-width multiply-add where the first multiplier is a vector register group.
///
/// `op` receives `(acc: u64, a: u64, b: u64, sew: Vsew)` where `acc` is the current `vd[i]`,
/// `a` is the element from `a_reg`, and `b` is the element from `src`. Returns the new `vd[i]`.
///
/// # Safety
/// - `vd`, `a_reg`, and `src` register alignment verified by caller
/// - `vl <= group_regs * VLEN.bytes() / sew_bytes`
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_muladd_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    a_reg: VReg,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    let vl = env.vl();
    let vstart = env.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`
    let mask_buf = unsafe { snapshot_mask(env.read_vregs(), vm, vl) };
    for i in vstart.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let acc = unsafe { env.read_vregs().read_element(vd, i, sew) };
        // SAFETY: register bounds verified by caller
        let a = unsafe { env.read_vregs().read_element(a_reg, i, sew) };
        let b = match src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe { env.read_vregs().read_element(b_reg, i, sew) },
            OpSrc::Scalar(val) => val,
        };
        let result = op(acc, a, b, sew);
        // SAFETY: register bounds verified by caller
        unsafe {
            env.write_vregs().write_element(vd, i, sew, result);
        }
    }
    env.mark_vs_dirty();
    env.reset_vstart();
}

/// Execute a single-width multiply-add where the first multiplier is a scalar.
///
/// Analogous to [`execute_muladd_op`] but `a` is a fixed scalar instead of a register element.
///
/// # Safety
/// Same as [`execute_muladd_op`], minus constraints on `a_reg`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_muladd_scalar_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    scalar: u64,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    let vl = env.vl();
    let vstart = env.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`
    let mask_buf = unsafe { snapshot_mask(env.read_vregs(), vm, vl) };
    for i in vstart.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: register bounds verified by caller
        let acc = unsafe { env.read_vregs().read_element(vd, i, sew) };
        let b = match src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe { env.read_vregs().read_element(b_reg, i, sew) },
            OpSrc::Scalar(val) => val,
        };
        let result = op(acc, scalar, b, sew);
        // SAFETY: register bounds verified by caller
        unsafe {
            env.write_vregs().write_element(vd, i, sew, result);
        }
    }
    env.mark_vs_dirty();
    env.reset_vstart();
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
/// - `2*SEW <= ELEN` verified by caller via [`widening_eew_supported()`], so both the accumulator
///   and the result fit in a `u64`
/// - When `vm=false`: `vd.to_bits() != 0`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_widening_muladd_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    a_reg: VReg,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    // SAFETY: `2 * SEW <= ELEN` is a precondition, so the double width exists
    let wide_sew = unsafe { sew.double_width().unwrap_unchecked() };
    let vl = env.vl();
    let vstart = env.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`
    let mask_buf = unsafe { snapshot_mask(env.read_vregs(), vm, vl) };
    for i in vstart.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Read the existing 2*SEW accumulator from vd
        // SAFETY: vd has dest_group_regs registers; element `i` fits within them (see
        // `execute_widening_op` for the bound argument); `2*SEW <= ELEN <= 64` by caller
        let acc = unsafe { env.read_vregs().read_element(vd, i, wide_sew) };
        // SAFETY: register bounds verified by caller
        let a = unsafe { env.read_vregs().read_element(a_reg, i, sew) };
        let b = match src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe { env.read_vregs().read_element(b_reg, i, sew) },
            OpSrc::Scalar(val) => val,
        };
        let result = op(acc, a, b, sew);
        // SAFETY: same as acc read above
        unsafe {
            env.write_vregs().write_element(vd, i, wide_sew, result);
        }
    }
    env.mark_vs_dirty();
    env.reset_vstart();
}

/// Execute a widening multiply-add where the first multiplier is a scalar.
///
/// Analogous to [`execute_widening_muladd_op`] but `a` is a fixed scalar.
///
/// # Safety
/// Same as [`execute_widening_muladd_op`], minus constraints on `a_reg`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub unsafe fn execute_widening_muladd_scalar_op<Reg, Env, F>(
    env: &mut Env,
    vd: VReg,
    scalar: u64,
    src: OpSrc,
    vm: bool,
    sew: Vsew,
    op: F,
) where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    F: Fn(u64, u64, u64, Vsew) -> u64,
{
    // SAFETY: `2 * SEW <= ELEN` is a precondition, so the double width exists
    let wide_sew = unsafe { sew.double_width().unwrap_unchecked() };
    let vl = env.vl();
    let vstart = env.vstart();
    // SAFETY: `vl <= VLMAX <= VLEN`
    let mask_buf = unsafe { snapshot_mask(env.read_vregs(), vm, vl) };
    for i in vstart.range_to(vl) {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: vd has dest_group_regs registers; element `i` fits within them (see
        // `execute_widening_op` for the bound argument); `2*SEW <= ELEN <= 64` by caller
        let acc = unsafe { env.read_vregs().read_element(vd, i, wide_sew) };
        let b = match src {
            // SAFETY: register bounds verified by caller
            OpSrc::Vreg(b_reg) => unsafe { env.read_vregs().read_element(b_reg, i, sew) },
            OpSrc::Scalar(val) => val,
        };
        let result = op(acc, scalar, b, sew);
        // SAFETY: same as acc read above
        unsafe {
            env.write_vregs().write_element(vd, i, wide_sew, result);
        }
    }
    env.mark_vs_dirty();
    env.reset_vstart();
}

/// Signed × signed high half.
///
/// Both operands are sign-extended to i64, multiplied as i128, and the upper SEW bits of the
/// 2*SEW product are returned (zero-extended to u64 for writeback into a SEW-wide element slot).
///
/// Valid for every SEW including 64, because the intermediate product is formed in i128. Whether
/// SEW=64 is reachable at all is an extension-level decision made by the caller (Zve64x excludes
/// it, the full "V" extension does not).
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn mulh_ss(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    let product = sa.widening_mul(sb);
    // Extract bits [2*SEW-1 : SEW] of the product
    let high = (product >> sew.bits_width()).cast_unsigned() as u64;
    high & sew_mask(sew)
}

/// Unsigned × unsigned high half.
///
/// Valid for every SEW including 64; see [`mulh_ss()`] for the extension-level caveat.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn mulhu_uu(a: u64, b: u64, sew: Vsew) -> u64 {
    let ua = a & sew_mask(sew);
    let ub = b & sew_mask(sew);
    let product = ua.widening_mul(ub);
    let high = (product >> sew.bits_width()) as u64;
    high & sew_mask(sew)
}

/// Signed × unsigned high half.
///
/// `a` (vs2) is the signed operand; `b` (vs1/rs1) is the unsigned operand.
///
/// Valid for every SEW including 64; see [`mulh_ss()`] for the extension-level caveat. Note that
/// the unsigned operand is widened to i128 rather than i64, so a full-width unsigned value at
/// SEW=64 keeps its magnitude instead of being reinterpreted as negative.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn mulhsu_su(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = i128::from(sign_extend(a, sew));
    let ub = i128::from(b & sew_mask(sew));
    let product = sa.wrapping_mul(ub);
    let high = (product >> sew.bits_width()).cast_unsigned() as u64;
    high & sew_mask(sew)
}

/// Signed divide with division-by-zero and signed-overflow semantics from the RISC-V V spec §12.11.
///
/// - Division by zero: result = all-ones (i.e., −1 as signed SEW-wide integer)
/// - Signed overflow (MIN / −1): result = MIN (i.e., `1 << (SEW-1)`)
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn sdiv(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Division by zero: return all-ones in the SEW-wide slot (= −1 signed)
    if sb == 0 {
        return sew_mask(sew);
    }
    sa.wrapping_div(sb).cast_unsigned() & sew_mask(sew)
}

/// Signed remainder with division-by-zero and signed-overflow semantics from the RISC-V V spec
/// §12.11.
///
/// - Division by zero: remainder = dividend
/// - Signed overflow (MIN % −1): remainder = 0
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn srem(a: u64, b: u64, sew: Vsew) -> u64 {
    let sa = sign_extend(a, sew);
    let sb = sign_extend(b, sew);
    // Division by zero: remainder = dividend
    if sb == 0 {
        return a & sew_mask(sew);
    }
    sa.wrapping_rem(sb).cast_unsigned() & sew_mask(sew)
}
