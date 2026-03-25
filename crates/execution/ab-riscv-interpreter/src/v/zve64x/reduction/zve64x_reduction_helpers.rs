//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::arith::zve64x_arith_helpers::{
    read_element_u64, sign_extend, write_element_u64,
};
use crate::v::zve64x::load::zve64x_load_helpers::{mask_bit, snapshot_mask};
use crate::{InterpreterState, ProgramCounter, VirtualMemory};
use ab_riscv_primitives::instructions::v::Vsew;
use ab_riscv_primitives::registers::general_purpose::Register;
use ab_riscv_primitives::registers::vector::VReg;
use core::fmt;

/// Execute a single-width integer reduction over `vstart..vl`.
///
/// The initial accumulator is read from element 0 of `vs1` (always SEW-wide, single register).
/// Active elements of `vs2` (masked by `vm` / `v0`) are folded into the accumulator using `op`.
/// The scalar result is written to element 0 of `vd` (single register, always SEW-wide).
///
/// When `vl == 0` or all elements are masked out, element 0 of `vs1` is passed through to `vd[0]`
/// unchanged, per spec §14.1.
///
/// `op` receives `(accumulator: u64, element: u64, sew: Vsew) -> u64`. Only the low `sew.bits()`
/// of the returned value are significant; all arithmetic should be performed within that width.
///
/// # Safety
/// - `vs2.bits() % group_regs == 0` and `vs2.bits() + group_regs <= 32` (verified by caller)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_reduce_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
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
    // Read scalar initial value from vs1[0]; vs1 is always a single register, SEW-wide
    // SAFETY: element 0 always fits within register vs1 (0 < VLENB / sew_bytes)
    let init =
        unsafe { read_element_u64(state.ext_state.read_vreg(), usize::from(vs1.bits()), 0, sew) };

    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };

    let vs2_base = usize::from(vs2.bits());

    // When vstart > 0 the spec says the reduction resumes from wherever it left off; since
    // reductions are not restartable in general, vstart != 0 at entry to a reduction instruction
    // is reserved. We follow the simplest compliant path: treat vstart as the lower bound of
    // active elements, initialising the accumulator to vs1[0] unconditionally.
    let mut acc = init;
    for i in vstart..vl {
        // Inactive elements are skipped; they do not contribute to the result
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `vs2_base % group_regs == 0` and `i < vl <= group_regs * elems_per_reg`,
        // so `vs2_base + i / elems_per_reg < vs2_base + group_regs <= 32`
        let elem = unsafe { read_element_u64(state.ext_state.read_vreg(), vs2_base, i, sew) };
        acc = op(acc, elem, sew);
    }

    // Write scalar result to vd[0]; vd is always a single register, SEW-wide
    // SAFETY: element 0 always fits within register vd
    unsafe {
        write_element_u64(state.ext_state.write_vreg(), vd.bits(), 0, sew, acc);
    }

    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}

/// Execute a widening integer sum reduction over `vstart..vl`.
///
/// `vs2` elements are SEW-wide; the accumulator and result (`vs1[0]` / `vd[0]`) are 2*SEW-wide.
/// The `sign_extend_src` flag controls whether each `vs2` element is sign-extended (`vwredsum`)
/// or zero-extended (`vwredsumu`) to 2*SEW before accumulation.
///
/// Per spec §14.2, the result SEW for the destination is 2*SEW, stored as a single element in
/// `vd[0]` using the widened width. `vs1[0]` is also read at 2*SEW.
///
/// # Safety
/// - `vs2.bits() % group_regs == 0` and `vs2.bits() + group_regs <= 32` (verified by caller)
/// - `2 * sew.bits() <= ELEN` (verified by caller - widening constraint)
/// - `vl <= group_regs * VLENB / sew_bytes`
/// - `vl <= VLEN`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_widening_reduce_op<Reg, ExtState, Memory, PC, IH, CustomError, F>(
    state: &mut InterpreterState<Reg, ExtState, Memory, PC, IH, CustomError>,
    vd: VReg,
    vs2: VReg,
    vs1: VReg,
    vm: bool,
    vl: u32,
    vstart: u32,
    sew: Vsew,
    op: F,
    sign_extend_src: bool,
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
    // The widened SEW for vs1/vd operands
    // Caller guarantees `2 * sew.bits() <= ELEN <= 64`, so this fits in u8
    let wide_sew = match sew {
        Vsew::E8 => Vsew::E16,
        Vsew::E16 => Vsew::E32,
        // E32 -> E64 is the max widening allowed under ELEN=64
        Vsew::E32 => Vsew::E64,
        // E64 widening would require 128-bit result; caller must reject this before entry
        Vsew::E64 => {
            // SAFETY: caller verified `2*SEW <= ELEN`; E64 widening is unreachable here
            unsafe { core::hint::unreachable_unchecked() }
        }
    };

    // Read initial accumulator from vs1[0] at 2*SEW
    // SAFETY: element 0 always fits within register vs1
    let init = unsafe {
        read_element_u64(
            state.ext_state.read_vreg(),
            usize::from(vs1.bits()),
            0,
            wide_sew,
        )
    };

    // SAFETY: `vl <= VLEN`, so `vl.div_ceil(8) <= VLENB`
    let mask_buf = unsafe { snapshot_mask(state.ext_state.read_vreg(), vm, vl) };

    let vs2_base = usize::from(vs2.bits());

    let mut acc = init;
    for i in vstart..vl {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: same bounds argument as `execute_reduce_op`
        let raw = unsafe { read_element_u64(state.ext_state.read_vreg(), vs2_base, i, sew) };
        // Widen element to 2*SEW
        let elem = if sign_extend_src {
            sign_extend(raw, sew).cast_unsigned()
        } else {
            // Zero-extension: raw is already zero-extended to u64 by read_element_u64
            raw
        };
        acc = op(acc, elem, wide_sew);
    }

    // Write result to vd[0] at 2*SEW
    // SAFETY: element 0 always fits within register vd
    unsafe {
        write_element_u64(state.ext_state.write_vreg(), vd.bits(), 0, wide_sew, acc);
    }

    state.ext_state.mark_vs_dirty();
    state.ext_state.reset_vstart();
}
