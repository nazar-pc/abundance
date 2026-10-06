//! Opaque helpers for ZveXx extension

use crate::v::vector_registers::{VRegGroup, VectorRegisterFile, VectorRegistersExt};
use crate::v::zvexx::load::zvexx_load_helpers::{mask_bit, snapshot_mask};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

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
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
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
/// Elements `0..min(offset, vl)` in vd are unchanged.
/// Elements `offset..vl` where mask is active get vs2[i - offset].
///
/// Both register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_slideup<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
    offset: u64,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    // Unmasked slide is a copy of a contiguous element range between two contiguous register
    // groups, elements `offset..vl` of `vd` get elements `0..vl - offset` of `vs2`
    if vm {
        if let Some(count) = u64::from(u32::from(vl.get())).checked_sub(offset)
            && let Ok(dst_first) = u16::try_from(offset)
            && let Ok(count) = u32::try_from(count)
            && !env
                .write_vregs()
                .copy_elements(vd, dst_first, vs2, 0, count)
        {
            // Can't happen for groups of the same instruction, see
            // `VectorRegisterFile::copy_elements()`
            cold_path();
        }
        env.mark_vs_dirty();
        return;
    }
    // Per spec §16.3.1: elements `0..offset` are never written (vd keeps its value)
    let Ok(offset) = u16::try_from(offset) else {
        env.mark_vs_dirty();
        return;
    };

    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        let Some(src_i) = i.checked_sub(offset) else {
            continue;
        };
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let val = env
            .read_vregs()
            .read(vs2, src_i)
            .expect("`src_i <= i < vl`, which is the same for `vs2`, checked above; qed");
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a vslidedown operation.
///
/// Element `vd[i] = vs2[i + offset]` if `i + offset < vlmax`, else `0`.
///
/// Both register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_slidedown<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
    offset: u64,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };

    // Unmasked slide is a copy of a contiguous element range between two contiguous register
    // groups (or within one), followed by zeroing of the elements whose source is beyond `vlmax`
    if vm {
        let vl_len = u32::from(vl.get());
        // Elements `i` with `i + offset < vlmax` have a source, the rest are zeroed
        let copied = u64::from(u32::from(vs2.vlmax()))
            .saturating_sub(offset)
            .min(u64::from(vl_len));
        let copied = u32::try_from(copied).unwrap_or(vl_len);
        let vregs = env.write_vregs();
        if copied > 0
            && let Ok(src_first) = u16::try_from(offset)
            && !vregs.copy_elements(vd, 0, vs2, src_first, copied)
        {
            // Can't happen for groups of the same instruction, see
            // `VectorRegisterFile::copy_elements()`
            cold_path();
        }
        if let Ok(first_zeroed) = u16::try_from(copied)
            && let Some(bytes) = vregs.elements_bytes_mut(vd, first_zeroed, vl_len - copied)
        {
            bytes.fill(0);
        }
        env.mark_vs_dirty();
        return;
    }

    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // Any source index that doesn't fit is beyond `vlmax`, where the spec requires `vd[i] = 0`
        let val = u64::from(i)
            .checked_add(offset)
            .and_then(|src_idx| u16::try_from(src_idx).ok())
            .and_then(|src_idx| env.read_vregs().read_up_to_vlmax(vs2, src_idx))
            .unwrap_or_default();
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute a vslide1up operation.
///
/// Element 0 of vd gets `scalar` (when active and vl > 0).
/// Element `i` for `1 <= i < vl` gets `vs2[i - 1]`.
/// vd must not overlap vs2.
///
/// Both register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_slide1up<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
    scalar: u64,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let val = match i.checked_sub(1) {
            Some(src_idx) => env
                .read_vregs()
                .read(vs2, src_idx)
                .expect("`i - 1 < vl`, which is the same for `vs2`, checked above; qed"),
            None => scalar,
        };
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
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
/// Both register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_slide1down<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
    scalar: u64,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        // The last element has no source in `vs2`, which `read()` reports as `None`
        let val = i
            .checked_add(1)
            .and_then(|src_idx| env.read_vregs().read(vs2, src_idx))
            .unwrap_or(scalar);
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute vrgather.vv: `vd[i] = (vs1[i] < vlmax) ? vs2[vs1[i]] : 0`.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_rgather_vv<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vs1: VRegGroup<Env::Hart>,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs1) = vs1.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        let index = env
            .read_vregs()
            .read(vs1, i)
            .expect("`vs1` has the same `vl` as `vd`, checked above; qed");
        let val = gather_element(env.read_vregs(), vs2, index);
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute vrgather.vx / vrgather.vi: all active elements get `vs2[index]` or `0`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_rgather_scalar<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
    index: u64,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    // Pre-compute the gathered value; it's the same for all elements.
    let val = gather_element(env.read_vregs(), vs2, index);
    for i in vd.vl().indices() {
        if !mask_bit(&mask_buf, i) {
            continue;
        }
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute vrgatherei16.vv: `vd[i] = (vs1_16[i] < vlmax) ? vs2[vs1_16[i]] : 0`.
///
/// `vs1` always uses EEW=16 regardless of SEW.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_rgatherei16<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vs1: VRegGroup<Env::Hart>,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    execute_rgather_vv::<Reg, Env>(env, vd, vs2, vs1, vm);
}

/// Element `index` of `vs2` if it is below `VLMAX`, `0` otherwise, which is what `vrgather`
/// produces
#[inline(always)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
fn gather_element<Hart>(vregs: &VectorRegisterFile<Hart>, vs2: VRegGroup<Hart>, index: u64) -> u64
where
    Hart: VectorHartConfig,
{
    u16::try_from(index)
        .ok()
        .and_then(|index| vregs.read_up_to_vlmax(vs2, index))
        .unwrap_or_default()
}

/// Execute vmerge.vvm / vmv.v.v.
///
/// When `vm=true` (vmv.v.v): all active elements `0..vl` get `vs1[i]`; vs2 unused.
/// When `vm=false` (vmerge.vvm): active elements where `v0[i]=1` get `vs1[i]`,
/// inactive elements get `vs2[i]`.
///
/// All register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_merge_vv<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vs1: VRegGroup<Env::Hart>,
    vm: bool,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let (Some(vs2), Some(vs1)) = (vs2.with_same_vl(vl), vs1.with_same_vl(vl)) else {
        cold_path();
        return;
    };
    // For vmv.v.v (vm=true) the mask is all-ones so snapshot_mask is still valid.
    let mask_buf = snapshot_mask(env.read_vregs(), vm);
    for i in vl.indices() {
        // mask_set=false only reachable when vm=false (vmerge path)
        let source = if mask_bit(&mask_buf, i) { vs1 } else { vs2 };
        let val = env
            .read_vregs()
            .read(source, i)
            .expect("`vs1` and `vs2` have the same `vl` as `vd`, checked above; qed");
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute vmerge.vxm / vmerge.vim / vmv.v.x / vmv.v.i.
///
/// When `vm=true`: all active elements `0..vl` get `scalar`; vs2 unused.
/// When `vm=false`: active elements where `v0[i]=1` get `scalar`,
/// inactive elements get `vs2[i]`.
///
/// Both register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_merge_scalar<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vm: bool,
    scalar: u64,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    let mask_buf = snapshot_mask(env.read_vregs(), vm);

    for i in vl.indices() {
        let val = if mask_bit(&mask_buf, i) {
            scalar
        } else {
            env.read_vregs()
                .read(vs2, i)
                .expect("`vs2` has the same `vl` as `vd`, checked above; qed")
        };
        env.write_vregs()
            .write(vd, i, val)
            .expect("`i < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Execute vcompress.vm: pack active elements of vs2 (under vs1 mask) sequentially into vd.
///
/// `vs1` is treated as an explicit mask register (single register, not LMUL-grouped).
/// The output write index increments only for elements where `vs1[i]` is set.
/// vd must not overlap vs1 or vs2.
///
/// Both register groups must have the same `vl`, which holds for groups created from the same
/// configuration, nothing is written otherwise.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_compress<Reg, Env>(
    env: &mut Env,
    vd: VRegGroup<Env::Hart>,
    vs2: VRegGroup<Env::Hart>,
    vs1: VReg,
) where
    Reg: Register,
    Env: VectorRegistersExt<Hart: HartConfig<Reg = Reg>>,
{
    let vl = vd.vl();
    let Some(vs2) = vs2.with_same_vl(vl) else {
        cold_path();
        return;
    };
    // The whole register is copied, which needs no bounds check, only bits below `vl` are read
    let vs1_buf = *env.read_vregs().get(vs1);
    let mut out_indices = vl.indices();
    for i in vl.indices() {
        if !mask_bit(&vs1_buf, i) {
            continue;
        }
        let val = env
            .read_vregs()
            .read(vs2, i)
            .expect("`vs2` has the same `vl` as `vd`, checked above; qed");
        // There are as many output indices as input indices, and output only advances when
        // input does
        let Some(out_idx) = out_indices.next() else {
            break;
        };
        env.write_vregs()
            .write(vd, out_idx, val)
            .expect("`out_idx < vl` of `vd`; qed");
    }
    env.mark_vs_dirty();
}

/// Copy `COUNT` whole vector registers from `src_base` to `dst_base`.
///
/// No masking, no vtype dependency. Uses snapshot semantics: all source registers are read into
/// a stack buffer before any destination registers are written, giving correct memmove-style
/// behaviour for all overlap patterns (including partial overlap such as src=V0, dst=V1, count=2).
///
/// Returns `None` if either register group doesn't fit into the register file, which doesn't
/// happen for groups aligned to `COUNT`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn execute_whole_reg_move<const COUNT: usize, Hart>(
    vregs: &mut VectorRegisterFile<Hart>,
    dst_base: VReg,
    src_base: VReg,
) -> Option<()>
where
    Hart: VectorHartConfig,
{
    let registers = vregs.as_bytes_mut();
    // Snapshot all source registers before writing any destination registers.
    // This is correct for all overlap patterns without direction-dependent logic.
    let src = *registers
        .get(usize::from(src_base.to_bits())..)?
        .first_chunk::<COUNT>()?;
    *registers
        .get_mut(usize::from(dst_base.to_bits())..)?
        .first_chunk_mut::<COUNT>()? = src;
    Some(())
}
