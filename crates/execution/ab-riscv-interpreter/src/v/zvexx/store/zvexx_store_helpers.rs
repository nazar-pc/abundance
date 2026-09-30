//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VRegGroup, VRegSegmentGroup, VectorRegistersExt};
use crate::v::zvexx::load::zvexx_load_helpers::{
    access_wraps, bytes_before_wrap, effective_address, mask_bit, snapshot_mask,
};
use crate::{ExecutionError, VirtualMemory, VirtualMemoryError};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Write `eew`-sized data from `buf[..eew.bytes()]` to memory at `addr` (little-endian)
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn write_mem_element<Reg>(
    memory: &mut impl VirtualMemory,
    addr: u64,
    eew: Eew,
    buf: [u8; const { usize::from(Eew::MAX_BYTES) }],
) -> Result<(), VirtualMemoryError>
where
    Reg: Register,
{
    write_bytes::<Reg, _>(
        memory,
        addr,
        buf.get(..usize::from(eew.bytes_width()))
            .expect("Element width never exceeds `Eew::MAX_BYTES`; qed"),
    )
}

/// Write `src` starting at effective address `address`, wrapping around at the end of the
/// `XLEN`-bit address space
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn write_bytes<Reg, Memory>(
    memory: &mut Memory,
    address: u64,
    src: &[u8],
) -> Result<(), VirtualMemoryError>
where
    Reg: Register,
    Memory: VirtualMemory,
{
    let before_wrap = bytes_before_wrap::<Reg>(address, src.len());
    let Some((head, tail)) = src.split_at_checked(before_wrap) else {
        cold_path();
        return Err(VirtualMemoryError::OutOfBoundsWrite { address });
    };
    memory.write_slice(address, head)?;
    if !tail.is_empty() {
        cold_path();
        memory.write_slice(0, tail)?;
    }
    Ok(())
}

/// Execute a unit-stride or unit-stride segment store.
///
/// Segment stride between elements is `nf * eew.bytes()`. Field `f` for element `i` is at
/// `base + i * nf * eew.bytes() + f * eew.bytes()`. When `nf == 1` this degenerates to a
/// plain unit-stride store.
///
/// Elements `vstart..vl` of `vs3` are stored, where `vl` is the architectural one for regular
/// stores.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_unit_stride_store<Reg, Env, Memory>(
    env: &mut Env,
    memory: &mut Memory,
    vs3: VRegSegmentGroup<{ Env::VLEN }>,
    vm: bool,
    base: u64,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    Memory: VirtualMemory,
{
    let vl = vs3.first().vl();
    let vstart = env.vstart();
    let eew = vs3.first().eew();
    let elem_bytes = eew.bytes_width();
    let nf = vs3.nf();

    // Unmasked non-segment store is a plain copy of the contiguous element range of the register
    // group into a contiguous memory range, as long as the whole range is writable. If it is not,
    // the element-wise path below is what determines the faulting element and writes everything
    // before it, which is the same outcome a partially written copy would have.
    if vm
        && nf.fields_per_segment() == 1
        && let Some(count) = u32::from(vl.get()).checked_sub(u32::from(u16::from(vstart)))
        && count > 0
    {
        let first = u16::from(vstart);
        let addr = effective_address::<Reg>(base, u64::from(first) * u64::from(elem_bytes));
        // A range that wraps around the end of the address space is not contiguous in memory
        if let Some(data) = env.read_vregs().elements_bytes(vs3.first(), first, count)
            && !access_wraps::<Reg>(addr, data.len())
            && memory.write_slice(addr, data).is_ok()
        {
            env.reset_vstart();
            return Ok(());
        }
        cold_path();
    }

    let segment_stride = u64::from(nf.fields_per_segment() * elem_bytes);
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for element in vs3.elements(u16::from(vstart)) {
        let i = element.index();
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }
        let elem_base = effective_address::<Reg>(base, u64::from(i) * segment_stride);
        let fields = element.read_fields(env.read_vregs());
        for (f, field) in (0..nf.fields_per_segment()).zip(fields) {
            let addr = effective_address::<Reg>(elem_base, u64::from(f * elem_bytes));
            let data = field.to_le_bytes();
            // Record the current element index in `vstart` so that, on a memory fault, the failing
            // element can be identified and the operation can be restarted
            if let Err(error) = write_mem_element::<Reg>(memory, addr, eew, data) {
                cold_path();
                env.set_vstart(Vstart::from(i));
                return Err(ExecutionError::from(error));
            }
        }
    }
    env.reset_vstart();
    Ok(())
}

/// Execute a strided or strided-segment store.
///
///   `base + i * stride + f * eew.bytes()`, wrapping around modulo `2^XLEN`
///   `effective_address::<Reg>(base, i.wrapping_mul(stride) as u64).wrapping_add(f * eew.bytes())`
///
/// `stride` is the raw XLEN register value reinterpreted as a signed integer, matching the RVV
/// specification where the stride operand is a two's-complement signed offset.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_strided_store<Reg, Env, Memory>(
    env: &mut Env,
    memory: &mut Memory,
    vs3: VRegSegmentGroup<{ Env::VLEN }>,
    vm: bool,
    base: u64,
    stride: i64,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    Memory: VirtualMemory,
{
    let vstart = env.vstart();
    let eew = vs3.first().eew();
    let elem_bytes = eew.bytes_width();
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for element in vs3.elements(u16::from(vstart)) {
        let i = element.index();
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }
        let elem_base =
            effective_address::<Reg>(base, i64::from(i).wrapping_mul(stride).cast_unsigned());
        let fields = element.read_fields(env.read_vregs());
        for (f, field) in (0..vs3.nf().fields_per_segment()).zip(fields) {
            let addr = effective_address::<Reg>(elem_base, u64::from(f * elem_bytes));
            let data = field.to_le_bytes();
            // Record the current element index in `vstart` so that, on a memory fault, the failing
            // element can be identified and the operation can be restarted
            if let Err(error) = write_mem_element::<Reg>(memory, addr, eew, data) {
                cold_path();
                env.set_vstart(Vstart::from(i));
                return Err(ExecutionError::from(error));
            }
        }
    }
    env.reset_vstart();
    Ok(())
}

/// Execute an indexed (unordered or ordered) store or indexed-segment store.
///
/// The effective address of element `i`, field `f` is:
///   `base + index[i] + f * eew.bytes()`
/// where `index[i]` is element `i` of the index register group `vs2`, interpreted as an
/// unsigned integer of its element width.
///
/// `vs2` must have the same `vl` as `vs3`, which holds for groups created from the same
/// configuration, nothing is stored otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_indexed_store<Reg, Env, Memory>(
    env: &mut Env,
    memory: &mut Memory,
    vs3: VRegSegmentGroup<{ Env::VLEN }>,
    vs2: VRegGroup<{ Env::VLEN }>,
    vm: bool,
    base: u64,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    Memory: VirtualMemory,
{
    let Some(indexed) = vs3.with_index(vs2) else {
        cold_path();
        return Ok(());
    };
    let vstart = env.vstart();
    let data_eew = vs3.first().eew();
    let data_elem_bytes = data_eew.bytes_width();
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for indexed in indexed.elements(u16::from(vstart)) {
        let element = indexed.element();
        let i = element.index();
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }
        let offset = indexed.read_index(env.read_vregs());
        let elem_base = effective_address::<Reg>(base, offset);
        let fields = element.read_fields(env.read_vregs());
        for (f, field) in (0..vs3.nf().fields_per_segment()).zip(fields) {
            let addr =
                effective_address::<Reg>(elem_base, u64::from(f) * u64::from(data_elem_bytes));
            let data = field.to_le_bytes();
            // Record the current element index in `vstart` so that, on a memory fault, the failing
            // element can be identified and the operation can be restarted
            if let Err(error) = write_mem_element::<Reg>(memory, addr, data_eew, data) {
                cold_path();
                env.set_vstart(Vstart::from(i));
                return Err(ExecutionError::from(error));
            }
        }
    }
    env.reset_vstart();
    Ok(())
}
