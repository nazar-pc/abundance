//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VectorRegisterFile, VectorRegistersExt};
use crate::v::zvexx::zvexx_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter, VirtualMemory, VirtualMemoryError};
use ab_riscv_primitives::prelude::*;
use core::cmp::Ordering;
use core::fmt;
use core::hint::cold_path;

/// Return whether mask bit `i` is set in the mask byte slice.
///
/// Bits are stored LSB-first within each byte: bit `i` is at byte `i / 8`, position `i % 8`.
/// Returns `false` for any `i` outside the slice bounds.
#[inline(always)]
pub(crate) fn mask_bit(mask: &[u8], i: u32) -> bool {
    mask.get((i / u8::BITS) as usize)
        .is_some_and(|b| (b >> (i % u8::BITS)) & 1 != 0)
}

/// Copy the mask bytes needed to cover `vl` elements from `v0` into a stack buffer and return
/// it. The copy releases the shared borrow on the register file so the caller can immediately
/// take an exclusive borrow for writes.
///
/// When `vm=true` (unmasked), the buffer is filled with `0xff` so that every mask bit reads as `1`.
/// This means callers can unconditionally call [`mask_bit()`] on the returned buffer without
/// branching on `vm`. Current callers short-circuit with `!vm &&` before calling [`mask_bit()`] as
/// a micro-optimization on the common unmasked path, but correctness does not depend on that guard:
/// if it were removed, the `0xff` fill ensures [`mask_bit()`] would return `true` for every
/// element, preserving the unmasked semantics.
///
/// # Safety
/// `vl.div_ceil(8)` must be `<= VLENB`. This holds when `vl <= VLEN`, which is always true
/// when `vl` is the current architectural `vl` (bounded by `VLMAX <= VLEN`).
#[inline(always)]
pub(in super::super) unsafe fn snapshot_mask<const VLENB: usize>(
    vregs: &VectorRegisterFile<VLENB>,
    vm: bool,
    vl: u32,
) -> [u8; VLENB] {
    let mut buf = [0u8; VLENB];
    if vm {
        // All-ones: every element active
        buf = [0xffu8; VLENB];
    } else {
        let mask_bytes = vl.div_ceil(u8::BITS) as usize;
        // SAFETY: `mask_bytes <= VLENB` by the caller's precondition
        unsafe {
            buf.get_unchecked_mut(..mask_bytes)
                .copy_from_slice(vregs.get(VReg::V0).get_unchecked(..mask_bytes));
        }
    }
    buf
}

/// Return whether register groups `[a, a+a_regs)` and `[b, b+b_regs)` overlap.
#[inline(always)]
#[doc(hidden)]
pub fn groups_overlap(a: VReg, a_regs: u8, b: VReg, b_regs: u8) -> bool {
    let (a, b) = (a.to_bits(), b.to_bits());
    a < b + b_regs && b < a + a_regs
}

/// Return whether a *non-segment* indexed load's data destination group
/// `[vd, vd + data_regs)` may legally overlap its index source group `[vs2, vs2 + index_regs)`.
///
/// The data EEW equals `sew` (indexed loads take their data width from `vtype.vsew()`) with
/// `EMUL = LMUL`, whereas the index group has EEW `index_eew` and `EMUL = (index_eew / sew) *
/// LMUL`. Because the two groups can have different EEW, the general vector register overlap
/// constraint applies: a destination group may overlap a source group only when one of the
/// following holds:
///
/// - the EEWs are equal (the groups coincide); or
/// - the destination EEW is smaller and the overlap is in the lowest-numbered part of the source
///   group, i.e. the destination starts at the source's base register (`vd == vs2`); or
/// - the destination EEW is larger, the source EMUL is at least one register, and the overlap is in
///   the highest-numbered part of the destination group, i.e. both groups end at the same register
///   (`vd + data_regs == vs2 + index_regs`).
///
/// Groups that do not overlap at all are always permitted. Any other overlap is reserved.
///
/// Unlike indexed *segment* loads (which forbid any `vd`/`vs2` overlap to remain restartable),
/// these relaxed rules are what allow encodings such as `vluxei32.v v16, (s2), v16` when the data
/// and index EEW match.
#[inline(always)]
#[doc(hidden)]
pub fn indexed_load_overlap_allowed(
    vd: VReg,
    data_regs: u8,
    vs2: VReg,
    index_regs: u8,
    index_eew: Eew,
    sew: Vsew,
    vlmul: Vlmul,
) -> bool {
    if !groups_overlap(vd, data_regs, vs2, index_regs) {
        return true;
    }

    match sew.bytes_width().cmp(&index_eew.bytes_width()) {
        // Equal EEW: the two groups coincide, overlap is permitted.
        Ordering::Equal => true,
        // Smaller data EEW: overlap must be in the lowest-numbered part of the index group, which
        // (given both groups are alignment-checked) means the data group starts at the index base.
        Ordering::Less => vd == vs2,
        // Larger data EEW: overlap must be in the highest-numbered part of the data group, and the
        // index EMUL must be at least one full register. `index_regs` alone cannot distinguish a
        // whole-register EMUL from a fractional one clamped to a single register, so the EMUL is
        // recomputed here as `(index_eew / sew) * LMUL >= 1`.
        Ordering::Greater => {
            let (lmul_num, lmul_den) = vlmul.as_fraction();
            let index_emul_at_least_one = u16::from(index_eew.bits_width()) * u16::from(lmul_num)
                >= u16::from(sew.bits_width()) * u16::from(lmul_den);
            let (vd, vs2) = (vd.to_bits(), vs2.to_bits());
            index_emul_at_least_one && vd + data_regs == vs2 + index_regs
        }
    }
}

/// Check that `vd` is aligned to `group_regs` and that the group fits within `[0, 32)`.
///
/// Per spec, the base register of every register group must be a multiple of the group size.
#[inline(always)]
#[doc(hidden)]
pub fn check_register_group_alignment<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vd: VReg,
    group_regs: u8,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let vd = vd.to_bits();
    if !vd.is_multiple_of(group_regs) || vd + group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Validate segment register layout: all `nf` field groups fit within `[0, 32)`, the base
/// register is group-aligned, and the first field group does not include `v0` when masked.
///
/// Field `f` occupies registers `[vd + f * group_regs, vd + f * group_regs + group_regs)`.
/// On `Ok`, `vd.to_bits() + nf * group_regs <= 32` is guaranteed.
#[inline(always)]
#[doc(hidden)]
pub fn validate_segment_registers<Reg, Memory, PC, CustomError>(
    program_counter: &PC,
    vd: VReg,
    vm: bool,
    group_regs: u8,
    nf: Nf,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
{
    let group_regs = u32::from(group_regs);
    let nf = u32::from(nf.fields_per_segment());
    let vd_idx = u32::from(vd.to_bits());
    if vd_idx % group_regs != 0 || vd_idx + nf * group_regs > 32 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    // When masked, no field group may contain v0 (index 0). Since groups are laid out
    // contiguously from vd and vd is group-aligned, only the first field (f=0) could contain
    // v0, which happens exactly when vd == 0.
    if !vm && vd_idx == 0 {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }
    Ok(())
}

/// Read element `elem_i` from register group `[base_reg, base_reg + group_regs)` into a
/// `[u8; Eew::MAX_BYTES]` buffer.
///
/// The in-register position of element `elem_i` is:
///   - register `base_reg + elem_i / (VLENB / eew.bytes())`
///   - byte offset `(elem_i % (VLENB / eew.bytes())) * eew.bytes()`
///
/// The result is placed in `buf[..eew.bytes()]`; the remaining bytes are zero.
///
/// # Safety
/// `base_reg + elem_i / (VLENB / eew.bytes())` must be less than 32, i.e. `elem_i` must be
/// a valid element index within the register group.
#[inline(always)]
pub(in super::super) unsafe fn read_group_element<const VLENB: usize>(
    vregs: &VectorRegisterFile<VLENB>,
    base_reg: VReg,
    // TODO: `elem_i` here and in other places shouldn't be `u32`
    elem_i: u32,
    eew: Eew,
) -> [u8; Eew::MAX_BYTES as usize] {
    let elem_bytes = usize::from(eew.bytes_width());
    let elems_per_reg = VLENB / elem_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * elem_bytes;
    // SAFETY: `base_reg + reg_off < 32` by the caller's precondition
    let reg = unsafe {
        vregs.get(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap_unchecked())
    };
    // SAFETY: `byte_off + elem_bytes <= VLENB`: the maximum `byte_off` is
    // `(elems_per_reg - 1) * elem_bytes = VLENB - elem_bytes`, so
    // `byte_off + elem_bytes <= VLENB - elem_bytes + elem_bytes = VLENB`.
    // `elem_bytes <= Eew::MAX_BYTES`: all `Eew` variants are at most E64.
    let src = unsafe { reg.get_unchecked(byte_off..byte_off + elem_bytes) };
    let mut buf = [0; _];
    // SAFETY: `elem_bytes <= Eew::MAX_BYTES` as established above, so `..elem_bytes` is in bounds
    // for `buf`
    unsafe { buf.get_unchecked_mut(..elem_bytes) }.copy_from_slice(src);
    buf
}

/// Write `eew`-sized data from `buf[..eew.bytes()]` into element `elem_i` of register group
/// `[base_reg, base_reg + group_regs)`.
///
/// The in-register position follows the same layout as [`read_group_element`].
///
/// # Safety
/// `base_reg + elem_i / (VLENB / eew.bytes())` must be less than 32, i.e. `elem_i` must be
/// a valid element index within the register group.
#[inline(always)]
unsafe fn write_group_element<const VLENB: usize>(
    vregs: &mut VectorRegisterFile<VLENB>,
    base_reg: VReg,
    elem_i: u32,
    eew: Eew,
    buf: [u8; Eew::MAX_BYTES as usize],
) {
    let elem_bytes = usize::from(eew.bytes_width());
    let elems_per_reg = VLENB / elem_bytes;
    let reg_off = elem_i as usize / elems_per_reg;
    let byte_off = (elem_i as usize % elems_per_reg) * elem_bytes;
    // SAFETY: `base_reg + reg_off < 32` by the caller's precondition
    let reg = unsafe {
        vregs.get_mut(VReg::from_bits(base_reg.to_bits() + reg_off as u8).unwrap_unchecked())
    };
    // SAFETY: `byte_off + elem_bytes <= VLENB` and `elem_bytes <= Eew::MAX_BYTES`: same argument as
    // in `read_group_element`
    let dst = unsafe { reg.get_unchecked_mut(byte_off..byte_off + elem_bytes) };
    // SAFETY: `elem_bytes <= Eew::MAX_BYTES` as established above, so `..elem_bytes` is in bounds
    // for `buf`
    dst.copy_from_slice(unsafe { buf.get_unchecked(..elem_bytes) });
}

/// Read `eew`-sized data from memory at `addr` into a `[u8; Eew::MAX_BYTES]` buffer
/// (little-endian)
#[inline(always)]
fn read_mem_element(
    memory: &impl VirtualMemory,
    addr: u64,
    eew: Eew,
) -> Result<[u8; Eew::MAX_BYTES as usize], VirtualMemoryError> {
    let source = match memory.read_slice(addr, u32::from(eew.bytes_width())) {
        Ok(source) => source,
        Err(err) => {
            cold_path();
            return Err(err);
        }
    };
    let mut out = [0; _];
    out[..usize::from(eew.bytes_width())].copy_from_slice(source);
    Ok(out)
}

/// Execute a unit-stride or unit-stride segment load (including fault-only-first variants).
///
/// Segment stride between elements is `nf * eew.bytes()`. Field `f` for element `i` is at
/// `base + i * nf * eew.bytes() + f * eew.bytes()`. When `nf == 1` this degenerates to a
/// plain unit-stride load.
///
/// When `fault_only_first` is set: a memory error at element `i > 0` truncates `vl` to `i`
/// and returns `Ok`. An error at element `0` always propagates.
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0`
/// - `vd.to_bits() + nf * group_regs <= 32`
/// - `vl <= group_regs * VLENB / eew.bytes()` (all `vl` elements fit within the destination
///   register group; this holds when `vl` is the architectural `vl` and `group_regs` is the EMUL
///   register count for the given `eew` and `vtype`)
/// - When `vm=false`: `vd` does not overlap `v0` (i.e. `vd.to_bits() != 0`)
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_unit_stride_load<
    const FAULT_ONLY_FIRST: bool,
    Reg,
    ExtState,
    Memory,
    CustomError,
>(
    ext_state: &mut ExtState,
    memory: &Memory,
    vd: VReg,
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
    let elem_bytes = eew.bytes_width();
    let segment_stride = u64::from(nf.fields_per_segment()) * u64::from(elem_bytes);

    // SAFETY: `vl <= VLMAX <= VLEN`, so `vl.div_ceil(8) <= VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }

        let elem_base = base.wrapping_add(u64::from(i) * segment_stride);

        // Read all nf fields into a stack buffer before writing any of them.
        // This ensures a fault on field f>0 leaves the destination registers untouched for the
        // faulting element, so only elements with index new_vl are ever written (fault-only-first
        // semantics).
        //
        // Sized by `Nf::MAX * Eew::MAX_BYTES`: the V spec allows at most 8 fields (nf in 1..=8)
        // each is at most 8 bytes (E64), giving 64 bytes.
        let mut field_buf =
            [[0u8; usize::from(Eew::MAX_BYTES)]; usize::from(Nf::MAX.fields_per_segment())];

        for f in 0..nf.fields_per_segment() {
            let addr = elem_base.wrapping_add(u64::from(f * elem_bytes));
            match read_mem_element(memory, addr, eew) {
                Ok(data) => {
                    // SAFETY: `f < nf` and the precondition on this function requires
                    // `nf <= Nf::MAX` (the V spec encodes nf in 3 bits giving 1..=Nf::MAX, and the
                    // decoder enforces this before constructing the instruction). Therefore, `f as
                    // usize < nf as usize <= Nf::MAX`, which is exactly the length of `field_buf`.
                    unsafe {
                        *field_buf.get_unchecked_mut(f as usize) = data;
                    }
                }
                Err(mem_err) => {
                    cold_path();
                    if FAULT_ONLY_FIRST && i > 0 {
                        ext_state.set_vl(i);
                        ext_state.mark_vs_dirty();
                        ext_state.reset_vstart();
                        return Ok(());
                    }
                    if i > u32::from(vstart) {
                        // Elements [vstart, i) were committed; VS is now dirty.
                        ext_state.mark_vs_dirty();
                        // vstart records the faulting element for restartability.
                        ext_state.set_vstart(i as u16);
                    }
                    return Err(ExecutionError::MemoryAccess(mem_err));
                }
            }
        }

        // All nf fields for element i were read successfully; commit to the register file.
        for f in 0..nf.fields_per_segment() {
            // SAFETY: Guaranteed by function contract
            let field_base_reg =
                unsafe { VReg::from_bits(vd.to_bits() + f * group_regs).unwrap_unchecked() };
            // SAFETY: need `field_base_reg + i / (VLENB / elem_bytes) < 32`.
            //
            // Let `elems_per_reg = VLENB / elem_bytes`.
            // `i < vl <= group_regs * elems_per_reg` (precondition), so
            // `i / elems_per_reg < group_regs`.
            //
            // `field_base_reg = vd.to_bits() + f * group_regs`. Since `f < nf` and the
            // precondition guarantees `vd.to_bits() + nf * group_regs <= 32`:
            // `field_base_reg + group_regs <= vd.to_bits() + (f+1) * group_regs
            //                             <= vd.to_bits() + nf * group_regs <= 32`.
            //
            // Therefore, `field_base_reg + i / elems_per_reg
            //            < field_base_reg + group_regs <= 32`.
            //
            // For `field_buf`: `f < nf <= Nf::MAX` (the same argument as in the read loop
            // above), so `f as usize < Nf::MAX = field_buf.len()`.
            unsafe {
                write_group_element(
                    ext_state.write_vregs(),
                    field_base_reg,
                    i,
                    eew,
                    *field_buf.get_unchecked(f as usize),
                );
            }
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
    Ok(())
}

/// Execute a strided or strided segment load.
///
/// `addr[i] = base + i * stride` where `stride` is a signed XLEN-wide value. Field `f` of
/// element `i` is at `addr[i] + f * eew.bytes()`.
///
/// # Safety
/// - `vd.to_bits() % group_regs == 0`
/// - `vd.to_bits() + nf * group_regs <= 32`
/// - `vl <= group_regs * VLENB / eew.bytes()`
/// - When `vm=false`: `vd` does not overlap `v0` (i.e. `vd.to_bits() != 0`)
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_strided_load<Reg, ExtState, Memory, CustomError>(
    ext_state: &mut ExtState,
    memory: &Memory,
    vd: VReg,
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
    let elem_bytes = eew.bytes_width();

    // SAFETY: `vl <= VLMAX <= VLEN` (precondition), so `vl.div_ceil(8) <= VLEN / 8 = VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }

        let elem_base = base.wrapping_add(i64::from(i).wrapping_mul(stride).cast_unsigned());

        for f in 0..nf.fields_per_segment() {
            let addr = elem_base.wrapping_add(u64::from(f * elem_bytes));
            let data = match read_mem_element(memory, addr, eew) {
                Ok(data) => data,
                Err(mem_err) => {
                    cold_path();
                    if f > 0 || i > u32::from(vstart) {
                        ext_state.mark_vs_dirty();
                        ext_state.set_vstart(i as u16);
                    }
                    return Err(ExecutionError::MemoryAccess(mem_err));
                }
            };
            // SAFETY: Guaranteed by function contract
            let field_base_reg =
                unsafe { VReg::from_bits(vd.to_bits() + f * group_regs).unwrap_unchecked() };
            // SAFETY: need `field_base_reg + i / (VLENB / elem_bytes) < 32`.
            //
            // Let `elems_per_reg = VLENB / elem_bytes`.
            // `i < vl <= group_regs * elems_per_reg` (precondition), so
            // `i / elems_per_reg < group_regs`.
            //
            // `field_base_reg = vd.to_bits() + f * group_regs`. Since `f < nf` and
            // `vd.to_bits() + nf * group_regs <= 32` (precondition):
            // `field_base_reg + group_regs <= vd.to_bits() + (f+1) * group_regs
            //                             <= vd.to_bits() + nf * group_regs <= 32`.
            //
            // Therefore, `field_base_reg + i / elems_per_reg < field_base_reg + group_regs <= 32`.
            unsafe {
                write_group_element(ext_state.write_vregs(), field_base_reg, i, eew, data);
            }
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
    Ok(())
}

/// Execute an indexed (unordered or ordered) or indexed segment load.
///
/// For element `i`, reads `index_eew`-sized bytes from register group `vs2` at element `i`
/// to obtain a zero-extended byte offset, then loads `nf` data fields from
/// `base + offset + f * data_eew.bytes()`. Unordered vs ordered is functionally identical in
/// a software interpreter.
///
/// # Safety
/// - `vd.to_bits() % data_group_regs == 0`
/// - `vd.to_bits() + nf * data_group_regs <= 32`
/// - `vs2.to_bits() + (vl - 1) / (VLENB / index_eew.bytes()) < 32` (all `vl` index elements fit
///   within the register file; satisfied when `vs2` is alignment-checked against `EMUL_index` and
///   `vl` is the architectural `vl` bounded by `VLMAX`)
/// - `vl <= data_group_regs * VLENB / data_eew.bytes()` (all `vl` elements fit in a data group)
/// - When `vm=false`: `vd` does not overlap `v0` (i.e. `vd.to_bits() != 0`)
#[inline(always)]
#[expect(clippy::too_many_arguments, reason = "Internal API")]
#[doc(hidden)]
pub unsafe fn execute_indexed_load<Reg, ExtState, Memory, CustomError>(
    ext_state: &mut ExtState,
    memory: &Memory,
    vd: VReg,
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
    let index_base_reg = vs2;

    // SAFETY: `vl <= VLMAX <= VLEN` (precondition), so `vl.div_ceil(8) <= VLEN / 8 = VLENB`.
    let mask_buf = unsafe { snapshot_mask(ext_state.read_vregs(), vm, vl) };

    for i in u32::from(vstart)..vl {
        if !vm && !mask_bit(&mask_buf, i) {
            continue;
        }

        // SAFETY: need `index_base_reg + i / (VLENB / index_eew.bytes()) < 32`.
        //
        // The caller verified `vs2` is aligned to `EMUL_index` registers and that
        // `vs2.to_bits() + EMUL_index <= 32`. `EMUL_index` is defined so that
        // `EMUL_index * (VLENB / index_eew.bytes()) = VLMAX`. Since `i < vl <= VLMAX`,
        // `i / (VLENB / index_eew.bytes()) < EMUL_index`, and therefore
        // `index_base_reg + i / (VLENB / index_eew.bytes()) < index_base_reg + EMUL_index <= 32`.
        let index_buf =
            unsafe { read_group_element(ext_state.read_vregs(), index_base_reg, i, index_eew) };
        let offset = u64::from_le_bytes(index_buf);
        let elem_addr = base.wrapping_add(offset);

        let data_elem_bytes = data_eew.bytes_width();
        for f in 0..nf.fields_per_segment() {
            let addr = elem_addr.wrapping_add(u64::from(f) * u64::from(data_elem_bytes));
            let data = match read_mem_element(memory, addr, data_eew) {
                Ok(data) => data,
                Err(mem_err) => {
                    cold_path();
                    if f > 0 || i > u32::from(vstart) {
                        ext_state.mark_vs_dirty();
                        ext_state.set_vstart(i as u16);
                    }
                    return Err(ExecutionError::MemoryAccess(mem_err));
                }
            };
            // SAFETY: Guaranteed by function contract
            let field_base_reg =
                unsafe { VReg::from_bits(vd.to_bits() + f * data_group_regs).unwrap_unchecked() };
            // SAFETY: need `field_base_reg + i / (VLENB / data_eew.bytes()) < 32`.
            //
            // Let `data_elems_per_reg = VLENB / data_eew.bytes()`.
            // `i < vl <= data_group_regs * data_elems_per_reg` (precondition), so
            // `i / data_elems_per_reg < data_group_regs`.
            //
            // `field_base_reg = vd.to_bits() + f * data_group_regs`. Since `f < nf` and
            // `vd.to_bits() + nf * data_group_regs <= 32` (precondition):
            // `field_base_reg + data_group_regs <= vd.to_bits() + (f+1) * data_group_regs
            //                                  <= vd.to_bits() + nf * data_group_regs <= 32`.
            //
            // Therefore,
            // `field_base_reg + i / data_elems_per_reg < field_base_reg + data_group_regs <= 32`.
            unsafe {
                write_group_element(ext_state.write_vregs(), field_base_reg, i, data_eew, data);
            }
        }
    }

    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();
    Ok(())
}
