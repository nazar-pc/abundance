//! Opaque helpers for ZveXx extension

#[cfg(test)]
mod tests;

use crate::v::vector_config::VectorConfig;
use crate::v::vector_registers::{VLENB_USIZE, VectorRegisterFile, VectorRegistersExt};
pub use crate::v::vector_registers::{VRegGroup, VRegSegmentGroup};
use crate::{ExecutionError, VirtualMemory, VirtualMemoryError};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Effective address `base + offset`, wrapping around at the end of the `XLEN`-bit address space
/// like every memory address computation
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn effective_address<Reg>(base: u64, offset: u64) -> u64
where
    Reg: Register,
{
    Reg::Type::truncate_from_u64(base.wrapping_add(offset)).as_u64()
}

/// Number of bytes of a `len`-byte access at `address` that come before the end of the `XLEN`-bit
/// address space, the rest of the access wraps around to address zero
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn bytes_before_wrap<Reg>(address: u64, len: usize) -> usize
where
    Reg: Register,
{
    let remaining = (1u128 << Reg::XLEN).saturating_sub(u128::from(address));
    usize::try_from(remaining).map_or(len, |remaining| remaining.min(len))
}

/// Whether a `len`-byte access at `address` wraps around the end of the `XLEN`-bit address space
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn access_wraps<Reg>(address: u64, len: usize) -> bool
where
    Reg: Register,
{
    bytes_before_wrap::<Reg>(address, len) < len
}

/// Read exactly `dst.len()` bytes at `address` with a single `read_slice()` call
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn read_exact(
    memory: &impl VirtualMemory,
    address: u64,
    dst: &mut [u8],
) -> Result<(), VirtualMemoryError> {
    let Ok(len) = u32::try_from(dst.len()) else {
        cold_path();
        return Err(VirtualMemoryError::OutOfBoundsRead { address });
    };
    // Only the requested bytes are used, even if memory returned more
    let Some(src) = memory.read_slice(address, len)?.get(..dst.len()) else {
        cold_path();
        return Err(VirtualMemoryError::OutOfBoundsRead { address });
    };
    // Same as `copy_from_slice()`, but without a length check the compiler may fail to prove
    // redundant
    for (dst, src) in dst.iter_mut().zip(src) {
        *dst = *src;
    }
    Ok(())
}

/// Read `dst.len()` bytes starting at effective address `address`, wrapping around at the end of
/// the `XLEN`-bit address space
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn read_bytes<Reg, Memory>(
    memory: &Memory,
    address: u64,
    dst: &mut [u8],
) -> Result<(), VirtualMemoryError>
where
    Reg: Register,
    Memory: VirtualMemory,
{
    let before_wrap = bytes_before_wrap::<Reg>(address, dst.len());
    let Some((head, tail)) = dst.split_at_mut_checked(before_wrap) else {
        cold_path();
        return Err(VirtualMemoryError::OutOfBoundsRead { address });
    };
    read_exact(memory, address, head)?;
    if !tail.is_empty() {
        cold_path();
        read_exact(memory, 0, tail)?;
    }
    Ok(())
}

/// Return whether mask bit `i` is set in the mask byte slice.
///
/// Bits are stored LSB-first within each byte: bit `i` is at byte `i / 8`, position `i % 8`.
/// Returns `false` for any `i` outside the slice bounds.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(crate) fn mask_bit(mask: &[u8], i: u16) -> bool {
    mask.get(usize::from(i / u8::BITS as u16))
        .is_some_and(|b| (b >> (i % u8::BITS as u16)) & 1 != 0)
}

/// Copy mask register `v0` into a stack buffer and return it. The copy releases the shared borrow
/// on the register file so the caller can immediately take an exclusive borrow for writes.
///
/// The whole register is copied regardless of `vl`: a fixed-size copy needs no bounds check, and
/// callers only read mask bits of elements below `vl`.
///
/// When `vm=true` (unmasked), the buffer is filled with `0xff` so that every mask bit reads as `1`.
/// This means callers can unconditionally call [`mask_bit()`] on the returned buffer without
/// branching on `vm`. Current callers short-circuit with `!vm &&` before calling [`mask_bit()`] as
/// a micro-optimization on the common unmasked path, but correctness does not depend on that guard:
/// if it were removed, the `0xff` fill ensures [`mask_bit()`] would return `true` for every
/// element, preserving the unmasked semantics.
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub(in super::super) fn snapshot_mask<const VLEN: Vlen>(
    vregs: &VectorRegisterFile<VLEN>,
    vm: bool,
) -> [u8; VLENB_USIZE::<VLEN>] {
    if vm {
        // All-ones: every element active
        [0xffu8; _]
    } else {
        *vregs.get(VReg::V0)
    }
}

/// Read `eew`-sized data from memory at `addr` into a `[u8; Eew::MAX_BYTES]` buffer
/// (little-endian)
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn read_mem_element<Reg>(
    memory: &impl VirtualMemory,
    addr: u64,
    eew: Eew,
) -> Result<[u8; const { usize::from(Eew::MAX_BYTES) }], VirtualMemoryError>
where
    Reg: Register,
{
    let mut out = [0; _];
    let element = out
        .get_mut(..usize::from(eew.bytes_width()))
        .expect("Element width never exceeds `Eew::MAX_BYTES`; qed");
    if let Err(err) = read_bytes::<Reg, _>(memory, addr, element) {
        cold_path();
        return Err(err);
    }
    Ok(out)
}

/// Execute a unit-stride or unit-stride segment load (including fault-only-first variants).
///
/// Segment stride between elements is `nf * eew.bytes()`. Field `f` for element `i` is at
/// `base + i * nf * eew.bytes() + f * eew.bytes()`. When `nf == 1` this degenerates to a
/// plain unit-stride load.
///
/// When `fault_only_first` is set to the configuration the instruction executes with: a memory
/// error at element `i > 0` truncates `vl` to `i` and returns `Ok`. An error at element `0` always
/// propagates.
///
/// Elements `vstart..vl` of `vd` are loaded, where `vl` is the architectural one for regular loads.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_unit_stride_load<Reg, Env, Memory>(
    env: &mut Env,
    memory: &Memory,
    vd: VRegSegmentGroup<{ Env::VLEN }>,
    vm: bool,
    base: u64,
    fault_only_first: Option<VectorConfig<{ Env::ELEN }, { Env::VLEN }>>,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    Memory: VirtualMemory,
{
    let vl = vd.first().vl();
    let vstart = env.vstart();
    let eew = vd.first().eew();
    let elem_bytes = eew.bytes_width();
    let nf = vd.nf();

    // Unmasked non-segment load is a plain copy of a contiguous memory range into the contiguous
    // element range of the register group, as long as the whole range is readable. If it is not,
    // the element-wise path below is what determines the faulting element and everything before
    // it, exactly as if the copy was never attempted (nothing was written).
    if vm
        && nf.fields_per_segment() == 1
        && let Some(count) = u32::from(vl.get()).checked_sub(u32::from(u16::from(vstart)))
        && count > 0
    {
        let first = u16::from(vstart);
        let addr = effective_address::<Reg>(base, u64::from(first) * u64::from(elem_bytes));
        // At most 8 registers worth of bytes
        let len = count * u32::from(elem_bytes);
        // A range that wraps around the end of the address space is not contiguous in memory
        if let Ok(len_usize) = usize::try_from(len)
            && !access_wraps::<Reg>(addr, len_usize)
            && let Ok(bytes) = memory.read_slice(addr, len)
            && let Some(dst) = env
                .write_vregs()
                .elements_bytes_mut(vd.first(), first, count)
            && bytes.len() >= dst.len()
        {
            // Same as `copy_from_slice()`, but without a length check the compiler may fail to
            // prove redundant
            for (dst, src) in dst.iter_mut().zip(bytes) {
                *dst = *src;
            }
            env.mark_vs_dirty();
            env.reset_vstart();
            return Ok(());
        }
        cold_path();
    }

    let segment_stride = u64::from(nf.fields_per_segment()) * u64::from(elem_bytes);

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for element in vd.elements(u16::from(vstart)) {
        let i = element.index();
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }

        let elem_base = effective_address::<Reg>(base, u64::from(i) * segment_stride);

        // Read all nf fields into a stack buffer before writing any of them.
        // This ensures a fault on field f>0 leaves the destination registers untouched for the
        // faulting element, so only elements with index new_vl are ever written (fault-only-first
        // semantics).
        //
        // Sized by `Nf::MAX * Eew::MAX_BYTES`: the V spec allows at most 8 fields (nf in 1..=8)
        // each is at most 8 bytes (E64), giving 64 bytes.
        let mut field_buf = [0u64; const { usize::from(Nf::MAX.fields_per_segment()) }];

        // `nf <= Nf::MAX`, which is exactly the length of `field_buf`, so every field has a slot
        for (f, field) in (0..nf.fields_per_segment()).zip(&mut field_buf) {
            let addr = effective_address::<Reg>(elem_base, u64::from(f * elem_bytes));
            match read_mem_element::<Reg>(memory, addr, eew) {
                Ok(data) => {
                    *field = u64::from_le_bytes(data);
                }
                Err(mem_err) => {
                    cold_path();
                    if let Some(config) = fault_only_first
                        && i > 0
                    {
                        env.set_vector_config(Some(config.with_vl_at_most(Vl::from(i))));
                        env.mark_vs_dirty();
                        env.reset_vstart();
                        return Ok(());
                    }
                    if i > u16::from(vstart) {
                        // Elements [vstart, i) were committed; VS is now dirty.
                        env.mark_vs_dirty();
                        // vstart records the faulting element for restartability.
                        env.set_vstart(Vstart::from(i));
                    }
                    return Err(ExecutionError::from(mem_err));
                }
            }
        }

        // All nf fields for element i were read successfully; commit to the register file.
        element.write_fields(env.write_vregs(), &field_buf);
    }

    env.mark_vs_dirty();
    env.reset_vstart();
    Ok(())
}

/// Execute a strided or strided segment load.
///
/// `addr[i] = base + i * stride` where `stride` is a signed XLEN-wide value. Field `f` of
/// element `i` is at `addr[i] + f * eew.bytes()`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_strided_load<Reg, Env, Memory>(
    env: &mut Env,
    memory: &Memory,
    vd: VRegSegmentGroup<{ Env::VLEN }>,
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
    let eew = vd.first().eew();
    let elem_bytes = eew.bytes_width();

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for element in vd.elements(u16::from(vstart)) {
        let i = element.index();
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }

        let elem_base =
            effective_address::<Reg>(base, i64::from(i).wrapping_mul(stride).cast_unsigned());

        for field in element.fields() {
            let f = field.index();
            let addr = effective_address::<Reg>(elem_base, u64::from(f * elem_bytes));
            let data = match read_mem_element::<Reg>(memory, addr, eew) {
                Ok(data) => data,
                Err(mem_err) => {
                    cold_path();
                    if f > 0 || i > u16::from(vstart) {
                        env.mark_vs_dirty();
                        env.set_vstart(Vstart::from(i));
                    }
                    return Err(ExecutionError::from(mem_err));
                }
            };
            field.write(env.write_vregs(), u64::from_le_bytes(data));
        }
    }

    env.mark_vs_dirty();
    env.reset_vstart();
    Ok(())
}

/// Execute an indexed (unordered or ordered) or indexed segment load.
///
/// For element `i`, reads the index element `i` of `vs2` to obtain a zero-extended byte offset,
/// then loads `nf` data fields from `base + offset + f * data_eew.bytes()`. Unordered vs ordered is
/// functionally identical in a software interpreter.
///
/// `vs2` must have the same `vl` as `vd`, which holds for groups created from the same
/// configuration, nothing is loaded otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_indexed_load<Reg, Env, Memory>(
    env: &mut Env,
    memory: &Memory,
    vd: VRegSegmentGroup<{ Env::VLEN }>,
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
    let Some(indexed) = vd.with_index(vs2) else {
        cold_path();
        return Ok(());
    };
    let vstart = env.vstart();
    let data_eew = vd.first().eew();
    let data_elem_bytes = data_eew.bytes_width();

    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for indexed in indexed.elements(u16::from(vstart)) {
        let element = indexed.element();
        let i = element.index();
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }

        let offset = indexed.read_index(env.read_vregs());
        let elem_addr = effective_address::<Reg>(base, offset);

        for field in element.fields() {
            let f = field.index();
            let addr =
                effective_address::<Reg>(elem_addr, u64::from(f) * u64::from(data_elem_bytes));
            let data = match read_mem_element::<Reg>(memory, addr, data_eew) {
                Ok(data) => data,
                Err(mem_err) => {
                    cold_path();
                    if f > 0 || i > u16::from(vstart) {
                        env.mark_vs_dirty();
                        env.set_vstart(Vstart::from(i));
                    }
                    return Err(ExecutionError::from(mem_err));
                }
            };
            field.write(env.write_vregs(), u64::from_le_bytes(data));
        }
    }

    env.mark_vs_dirty();
    env.reset_vstart();
    Ok(())
}
