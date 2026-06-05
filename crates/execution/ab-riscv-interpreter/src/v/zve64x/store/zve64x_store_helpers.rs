//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::load::zve64x_load_helpers::{
    check_register_group_alignment, mask_bit, read_group_element, snapshot_mask,
};
use crate::v::zve64x::zve64x_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter, VirtualMemory, VirtualMemoryError};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Interpret `buf[..index_eew.bytes()]` as a little-endian unsigned integer and return it as
/// `u64`. Used to convert a packed index element into a byte offset.
///
/// # Safety
/// `index_eew.bytes() <= Eew::MAX_BYTES`, which is always true by construction.
#[inline(always)]
unsafe fn index_buf_to_u64(buf: [u8; Eew::MAX_BYTES as usize], index_eew: Eew) -> u64 {
    match index_eew {
        Eew::E8 => u64::from(buf[0]),
        Eew::E16 => u64::from(u16::from_le_bytes([buf[0], buf[1]])),
        Eew::E32 => u64::from(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]])),
        Eew::E64 => u64::from_le_bytes(buf),
    }
}

/// Write `eew`-sized data from `buf[..eew.bytes()]` to memory at `addr` (little-endian)
#[inline(always)]
fn write_mem_element(
    memory: &mut impl VirtualMemory,
    addr: u64,
    eew: Eew,
    buf: [u8; Eew::MAX_BYTES as usize],
) -> Result<(), VirtualMemoryError> {
    memory.write_slice(addr, &buf[..usize::from(eew.bytes())])
}

/// Validate a segment store's destination register group.
///
/// Like [`validate_segment_registers`] but omits the v0-overlap check, since
/// segment stores read `vs3` as a source and the source/v0 overlap restriction
/// applies only to load destinations.
#[inline(always)]
#[doc(hidden)]
pub fn validate_segment_store_registers<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vs3: VReg,
    group_regs: u8,
    nf: Nf,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    check_register_group_alignment::<Reg, _, _, _>(program_counter, vs3, group_regs)?;
    let total = u32::from(vs3.bits()) + u32::from(nf.fields_per_segment()) * u32::from(group_regs);
    if total > 32 {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Execute a unit-stride or unit-stride segment store.
///
/// Segment stride between elements is `nf * eew.bytes()`. Field `f` for element `i` is at
/// `base + i * nf * eew.bytes() + f * eew.bytes()`. When `nf == 1` this degenerates to a
/// plain unit-stride store.
///
/// # Safety
/// - `vs3.bits() % group_regs == 0`
/// - `vs3.bits() + nf * group_regs <= 32`
/// - `vl <= group_regs * VLENB / eew.bytes()` (all `vl` elements fit within the source register
///   group; this holds when `vl` is the architectural `vl` and `group_regs` is the EMUL register
///   count for the given `eew` and `vtype`)
/// - When `vm=false`: `vs3` does not overlap `v0` (i.e. `vs3.bits() != 0`)
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_unit_stride_store<Reg, ExtState, Memory, CustomError>(
    ext_state: &mut ExtState,
    memory: &mut Memory,
    vs3: VReg,
    vm: bool,
    base: u64,
    eew: Eew,
    group_regs: u8,
    nf: Nf,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let elem_bytes = eew.bytes();
    let segment_stride = u64::from(nf.fields_per_segment() * elem_bytes);
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLEN / 8 = VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }
        let elem_base = base.wrapping_add(u64::from(i) * segment_stride);
        for f in 0..nf.fields_per_segment() {
            let addr = elem_base.wrapping_add(u64::from(f) * u64::from(elem_bytes));
            let field_base_reg = vs3.bits() + f * group_regs;
            // SAFETY: need `field_base_reg + i / (VLENB / elem_bytes) < 32`.
            //
            // Let `elems_per_reg = VLENB / elem_bytes`.
            // `i < vl <= group_regs * elems_per_reg` (precondition), so
            // `i / elems_per_reg < group_regs`.
            //
            // `field_base_reg = vs3.bits() + f * group_regs`. Since `f < nf` and the
            // precondition guarantees `vs3.bits() + nf * group_regs <= 32`:
            // `field_base_reg + group_regs <= vs3.bits() + (f+1) * group_regs
            //                             <= vs3.bits() + nf * group_regs <= 32`.
            //
            // Therefore,
            // `field_base_reg + i / elems_per_reg < field_base_reg + group_regs <= 32`.
            let data = unsafe {
                read_group_element(ext_state.read_vreg(), usize::from(field_base_reg), i, eew)
            };
            // Record the current element index in `vstart` so that, on a memory fault, the failing
            // element can be identified and the operation can be restarted
            if let Err(error) = write_mem_element(memory, addr, eew, data) {
                ext_state.set_vstart(i as u16);
                return Err(ExecutionError::MemoryAccess(error));
            }
        }
    }
    ext_state.reset_vstart();
    Ok(())
}

/// Execute a strided or strided-segment store.
///
/// The address of element `i`, field `f` is:
///   `base.wrapping_add(i.wrapping_mul(stride) as u64).wrapping_add(f * eew.bytes())`
///
/// `stride` is the raw XLEN register value reinterpreted as a signed integer, matching the RVV
/// specification where the stride operand is a two's-complement signed offset.
///
/// # Safety
/// - `vs3.bits() % group_regs == 0`
/// - `vs3.bits() + nf * group_regs <= 32`
/// - `vl <= group_regs * VLENB / eew.bytes()`
/// - When `vm=false`: `vs3.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_strided_store<Reg, ExtState, Memory, CustomError>(
    ext_state: &mut ExtState,
    memory: &mut Memory,
    vs3: VReg,
    vm: bool,
    base: u64,
    stride: i64,
    eew: Eew,
    group_regs: u8,
    nf: Nf,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let elem_bytes = eew.bytes();
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }
        let elem_base = base.wrapping_add(i64::from(i).wrapping_mul(stride).cast_unsigned());
        for f in 0..nf.fields_per_segment() {
            let addr = elem_base.wrapping_add(u64::from(f) * u64::from(elem_bytes));
            let field_base_reg = vs3.bits() + f * group_regs;
            // SAFETY: same argument as `execute_unit_stride_store`; `field_base_reg +
            // i / elems_per_reg < field_base_reg + group_regs <= vs3.bits() + nf *
            // group_regs <= 32`.
            let data = unsafe {
                read_group_element(ext_state.read_vreg(), usize::from(field_base_reg), i, eew)
            };
            // Record the current element index in `vstart` so that, on a memory fault, the failing
            // element can be identified and the operation can be restarted
            if let Err(error) = write_mem_element(memory, addr, eew, data) {
                ext_state.set_vstart(i as u16);
                return Err(ExecutionError::MemoryAccess(error));
            }
        }
    }
    ext_state.reset_vstart();
    Ok(())
}

/// Execute an indexed (unordered or ordered) store or indexed-segment store.
///
/// The effective address of element `i`, field `f` is:
///   `base + index[i] + f * eew.bytes()`
/// where `index[i]` is element `i` of the index register group `vs2`, interpreted as an
/// unsigned integer of width `index_eew`.
///
/// `data_eew` is the element width of the data being stored (from `vtype.vsew`).
/// `index_eew` is the element width of the indices (from the instruction encoding).
///
/// # Safety
/// - `vs3.bits() % data_group_regs == 0`
/// - `vs3.bits() + nf * data_group_regs <= 32`
/// - `vs2` register group is aligned and fits within `[0, 32)` (caller must verify via
///   `check_register_group_alignment` before calling)
/// - `vl <= data_group_regs * VLENB / data_eew.bytes()`
/// - `vl <= index_group_regs * VLENB / index_eew.bytes()` (caller must verify)
/// - When `vm=false`: `vs3.bits() != 0`
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_indexed_store<Reg, ExtState, Memory, CustomError>(
    ext_state: &mut ExtState,
    memory: &mut Memory,
    vs3: VReg,
    vs2: VReg,
    vm: bool,
    base: u64,
    data_eew: Eew,
    index_eew: Eew,
    data_group_regs: u8,
    nf: Nf,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    CustomError: fmt::Debug,
{
    let vl = ext_state.vl();
    let vstart = ext_state.vstart();
    let data_elem_bytes = data_eew.bytes();
    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vreg(), vm, vl) };
    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }
        // SAFETY: `i < vl <= index_group_regs * VLENB / index_eew.bytes()` (precondition), so
        // `vs2.bits() + i / (VLENB / index_eew.bytes()) < vs2.bits() + index_group_regs <= 32`.
        let index_buf = unsafe {
            read_group_element(ext_state.read_vreg(), usize::from(vs2.bits()), i, index_eew)
        };
        // SAFETY: `index_eew.bytes() <= Eew::MAX_BYTES` always holds.
        let offset = unsafe { index_buf_to_u64(index_buf, index_eew) };
        let elem_base = base.wrapping_add(offset);
        for f in 0..nf.fields_per_segment() {
            let addr = elem_base.wrapping_add(u64::from(f) * u64::from(data_elem_bytes));
            let field_base_reg = vs3.bits() + f * data_group_regs;
            // SAFETY: `i < vl <= data_group_regs * VLENB / data_eew.bytes()` (precondition), so
            // `field_base_reg + i / elems_per_reg < field_base_reg + data_group_regs
            //                                    <= vs3.bits() + nf * data_group_regs <= 32`.
            let data = unsafe {
                read_group_element(
                    ext_state.read_vreg(),
                    usize::from(field_base_reg),
                    i,
                    data_eew,
                )
            };
            // Record the current element index in `vstart` so that, on a memory fault, the failing
            // element can be identified and the operation can be restarted
            if let Err(error) = write_mem_element(memory, addr, data_eew, data) {
                ext_state.set_vstart(i as u16);
                return Err(ExecutionError::MemoryAccess(error));
            }
        }
    }
    ext_state.reset_vstart();
    Ok(())
}
