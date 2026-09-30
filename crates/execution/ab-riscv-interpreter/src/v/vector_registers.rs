//! Vector registers

mod segment;
#[cfg(test)]
mod tests;

use crate::Csrs;
use crate::v::vector_config::{BoundedVl, VectorConfig};
use ab_riscv_primitives::prelude::*;
use core::hint::{assert_unchecked, cold_path};
use core::marker::Destruct;
use core::ptr;

pub use segment::{
    IndexedSegmentElement, IndexedSegmentGroup, SegmentElement, SegmentField, VRegSegmentGroup,
};

pub(crate) const VLENB_USIZE<const VLEN: Vlen>: usize = VLEN.bytes() as usize;
/// Element width in bytes as `usize`
const EEW_BYTES<const EEW: Eew>: usize = EEW.bytes_width() as usize;

/// Register group of elements of width `EEW` that lies within the vector register file.
///
/// Created from a [`VectorConfig`] with `EMUL = LMUL * EEW / SEW`, it holds `VLMAX` elements, the
/// first `vl` of which are the body. This makes the element accesses of
/// [`VectorRegisterFile::read()`] and friends safe, since the group carries the bounds they check
/// indices against.
#[derive(Debug, Clone, Copy)]
pub struct VRegGroup<const VLEN: Vlen> {
    base: VReg,
    emul: Vlmul,
    eew: Eew,
    // Invariant: `vl <= vlmax` and `vlmax` elements of `eew` starting at `base` end within the
    // register file
    vl: BoundedVl<VLEN>,
    vlmax: Vl,
}

impl<const VLEN: Vlen> VRegGroup<VLEN> {
    /// Register group starting at `base` with elements of width `eew` under `config`.
    ///
    /// Returns `None` if `eew` exceeds `ELEN`, `EMUL = LMUL * EEW / SEW` is outside `[1/8, 8]` or
    /// `base` is not aligned to `EMUL`, each of which makes an instruction illegal.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new<const ELEN: Elen>(
        config: VectorConfig<ELEN, VLEN>,
        base: VReg,
        eew: Eew,
    ) -> Option<Self>
    where
        [(); SUPPORTED_ELEN_VLEN::<ELEN, VLEN>]:,
    {
        if u32::from(eew.bits_width()) > u32::from(ELEN) {
            cold_path();
            return None;
        }
        let vtype = config.vtype();
        let emul = vtype.vlmul().emul(eew, vtype.vsew())?;
        if !base.is_group_aligned(emul.register_count()) {
            cold_path();
            return None;
        }

        let vl = config.vl();
        let vlmax = config.vlmax();
        // Both always hold for a legal `EMUL`, but are checked here rather than derived from how
        // `VectorConfig` and `Vlmul` compute them, since unsafe code relies on the invariant
        let group_end = u32::from(base.to_bits()) * VLEN.bytes()
            + u32::from(vlmax) * u32::from(eew.bytes_width());
        if u32::from(vl.get()) > u32::from(vlmax) || group_end > 32 * VLEN.bytes() {
            cold_path();
            return None;
        }

        Some(Self {
            base,
            emul,
            eew,
            vl,
            vlmax,
        })
    }

    /// Register group of `group_regs` whole registers starting at `base` with elements of width
    /// `eew`, regardless of `vtype`, like the one of whole register loads, stores and moves.
    ///
    /// Both `vl` and `VLMAX` are the number of elements the registers hold. Returns `None` if
    /// `base` is not aligned to `group_regs` or `eew` is wider than `VLEN`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn whole_registers(base: VReg, group_regs: VRegGroupSize, eew: Eew) -> Option<Self> {
        if !base.is_group_aligned(group_regs) {
            cold_path();
            return None;
        }
        let emul = match group_regs {
            VRegGroupSize::R1 => Vlmul::M1,
            VRegGroupSize::R2 => Vlmul::M2,
            VRegGroupSize::R4 => Vlmul::M4,
            VRegGroupSize::R8 => Vlmul::M8,
        };
        let elements = u32::from(group_regs.get()) * VLEN.bytes() / u32::from(eew.bytes_width());
        let vlmax = Vl::new(elements)?;
        // At most `8 * VLEN / 8`, hence never above `VLEN`
        let vl = BoundedVl::new(vlmax)?;

        Some(Self {
            base,
            emul,
            eew,
            vl,
            vlmax,
        })
    }

    /// The same group with `vl` reduced to `vl`, returns `None` if `vl` is above the current one
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn with_vl(self, vl: Vl) -> Option<Self> {
        if u32::from(vl) > u32::from(self.vl.get()) {
            cold_path();
            return None;
        }
        let vl = BoundedVl::new(vl)?;

        Some(Self { vl, ..self })
    }

    /// The same group if its `vl` is `vl`, `None` otherwise.
    ///
    /// The returned group carries `vl` itself, so that a check of an element index against it is
    /// trivially the same as a check against `vl` that loops over body elements do anyway, which
    /// is what makes groups of the same instruction interchangeable for the compiler.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn with_same_vl(self, vl: BoundedVl<VLEN>) -> Option<Self> {
        if self.vl != vl {
            cold_path();
            return None;
        }
        Some(Self { vl, ..self })
    }

    /// First register of the group
    #[inline(always)]
    pub const fn base(self) -> VReg {
        self.base
    }

    /// Effective multiplier `EMUL` of the group
    #[inline(always)]
    pub const fn emul(self) -> Vlmul {
        self.emul
    }

    /// Number of registers in the group, which is one for fractional `EMUL` too
    #[inline(always)]
    pub const fn group_regs(self) -> VRegGroupSize {
        self.emul.register_count()
    }

    /// Element width
    #[inline(always)]
    pub const fn eew(self) -> Eew {
        self.eew
    }

    /// Number of body elements, which is `vl`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn vl(self) -> BoundedVl<VLEN> {
        // SAFETY: Invariant of `VRegGroup`
        unsafe {
            assert_unchecked(u32::from(self.vl.get()) <= u32::from(self.vlmax));
        }
        self.vl
    }

    /// Whether `reg` is one of the registers of this group
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn contains(self, reg: VReg) -> bool {
        let (start, reg) = (self.base.to_bits(), reg.to_bits());
        start <= reg && reg < start + self.group_regs().get()
    }

    /// Whether the registers of this group and `other` overlap
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn overlaps(self, other: Self) -> bool {
        let (start, other_start) = (self.base.to_bits(), other.base.to_bits());
        start < other_start + other.group_regs().get()
            && other_start < start + self.group_regs().get()
    }

    /// Number of elements the group holds, which is `VLMAX` and at least [`Self::vl()`]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn vlmax(self) -> Vl {
        // SAFETY: Invariant of `VRegGroup`
        unsafe {
            assert_unchecked(u32::from(self.vl.get()) <= u32::from(self.vlmax));
        }
        self.vlmax
    }
}

/// Alignment wrapper for vector registers
#[derive(Debug, Clone, Copy)]
// Aligned to 128 bytes, which is u32 * 32 registers, the minimum reasonable value to use in most
// cases
#[repr(align(128))]
pub struct VectorRegisterFile<const VLEN: Vlen>([[u8; VLENB_USIZE::<VLEN>]; 32]);

const impl<const VLEN: Vlen> Default for VectorRegisterFile<VLEN> {
    #[inline(always)]
    fn default() -> Self {
        Self([[0; _]; _])
    }
}

impl<const VLEN: Vlen> VectorRegisterFile<VLEN> {
    /// Get reference to a vector register
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn get(&self, index: VReg) -> &[u8; VLENB_USIZE::<VLEN>] {
        self.0
            .get(usize::from(index.to_bits()))
            .expect("There are exactly 32 vector registers, one for each `VReg`; qed")
    }

    /// Get mutable reference to a vector register
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn get_mut(&mut self, index: VReg) -> &mut [u8; VLENB_USIZE::<VLEN>] {
        self.0
            .get_mut(usize::from(index.to_bits()))
            .expect("There are exactly 32 vector registers, one for each `VReg`; qed")
    }

    /// All vector registers as one contiguous array of bytes.
    ///
    /// Register `v` occupies bytes `[v * VLENB, (v + 1) * VLENB)` of the flattened array, so a
    /// register group is a contiguous range and its elements are at [`Self::element_offset()`].
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[[u8; VLENB_USIZE::<VLEN>]; 32] {
        &self.0
    }

    /// All vector registers as one contiguous mutable array of bytes, see [`Self::as_bytes()`]
    #[inline(always)]
    pub const fn as_bytes_mut(&mut self) -> &mut [[u8; VLENB_USIZE::<VLEN>]; 32] {
        &mut self.0
    }

    /// Byte offset within flattened [`Self::as_bytes()`] of element `elem_i` in the register
    /// group starting at `base_reg`, with `eew`-wide elements.
    ///
    /// Element widths divide `VLENB`, so elements never straddle registers and a register group
    /// is one contiguous array of elements.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn element_offset<W>(base_reg: VReg, elem_i: u16, eew: W) -> usize
    where
        W: [const] Into<Eew> + [const] Destruct,
    {
        usize::from(base_reg.to_bits()) * VLENB_USIZE::<VLEN>
            + usize::from(elem_i) * usize::from(eew.into().bytes_width())
    }

    /// Read body element `elem_i` of `group`, zero-extended.
    ///
    /// Returns `None` if `elem_i` is not below [`VRegGroup::vl()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn read(&self, group: VRegGroup<VLEN>, elem_i: u16) -> Option<u64> {
        if u32::from(elem_i) >= u32::from(group.vl.get()) {
            cold_path();
            return None;
        }
        // SAFETY: `elem_i < vl <= vlmax`, and `vlmax` elements of the group lie within the
        // register file by the invariant of `VRegGroup`
        Some(unsafe { self.read_element(group.base, elem_i, group.eew) })
    }

    /// Write the low bits of `value` into body element `elem_i` of `group`.
    ///
    /// Returns `None` if `elem_i` is not below [`VRegGroup::vl()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn write(&mut self, group: VRegGroup<VLEN>, elem_i: u16, value: u64) -> Option<()> {
        if u32::from(elem_i) >= u32::from(group.vl.get()) {
            cold_path();
            return None;
        }
        // SAFETY: `elem_i < vl <= vlmax`, and `vlmax` elements of the group lie within the
        // register file by the invariant of `VRegGroup`
        unsafe {
            self.write_element(group.base, elem_i, group.eew, value);
        }
        Some(())
    }

    /// Read any element `elem_i` of `group`, including those past `vl`, zero-extended.
    ///
    /// This is for instructions that index a source by data, like `vrgather`. Returns `None` if
    /// `elem_i` is not below [`VRegGroup::vlmax()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn read_up_to_vlmax(&self, group: VRegGroup<VLEN>, elem_i: u16) -> Option<u64> {
        if u32::from(elem_i) >= u32::from(group.vlmax) {
            cold_path();
            return None;
        }
        // SAFETY: `elem_i < vlmax`, and `vlmax` elements of the group lie within the register
        // file by the invariant of `VRegGroup`
        Some(unsafe { self.read_element(group.base, elem_i, group.eew) })
    }

    /// Copy `count` elements starting at `src_first` of `src` to elements starting at `dst_first`
    /// of `dst`, like `memmove`, so the two ranges may overlap.
    ///
    /// Destination elements must be body elements, source elements may be any elements below
    /// [`VRegGroup::vlmax()`], like for `vslidedown`. Returns `false` without writing anything if
    /// either range is out of bounds or the groups have different element widths.
    #[inline(always)]
    #[must_use]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn copy_elements(
        &mut self,
        dst: VRegGroup<VLEN>,
        dst_first: u16,
        src: VRegGroup<VLEN>,
        src_first: u16,
        count: u32,
    ) -> bool {
        if dst.eew != src.eew
            || u32::from(dst_first) + count > u32::from(dst.vl.get())
            || u32::from(src_first) + count > u32::from(src.vlmax)
        {
            cold_path();
            return false;
        }
        let dst_offset = Self::element_offset(dst.base, dst_first, dst.eew);
        let src_offset = Self::element_offset(src.base, src_first, src.eew);
        let Ok(count) = usize::try_from(count) else {
            cold_path();
            return false;
        };
        let len = count * usize::from(dst.eew.bytes_width());
        let bytes = self.as_bytes_mut().as_flattened_mut().as_mut_ptr();
        // SAFETY: Both element ranges were checked above to lie within `vlmax` elements of their
        // groups, which lie within the register file by the invariant of `VRegGroup`. Overlapping
        // ranges are fine for `ptr::copy()`, and both pointers derive from the same mutable one.
        unsafe {
            ptr::copy(bytes.byte_add(src_offset), bytes.byte_add(dst_offset), len);
        }
        true
    }

    /// Bytes of `count` body elements starting at `first` of `group`.
    ///
    /// Returns `None` if the range is not within [`VRegGroup::vl()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn elements_bytes(&self, group: VRegGroup<VLEN>, first: u16, count: u32) -> Option<&[u8]> {
        if u32::from(first) + count > u32::from(group.vl().get()) {
            cold_path();
            return None;
        }
        let offset = Self::element_offset(group.base, first, group.eew);
        let len = usize::try_from(count).ok()? * usize::from(group.eew.bytes_width());
        // SAFETY: The element range was checked above to lie within `vl` elements of the group,
        // which lie within the register file by the invariant of `VRegGroup`
        Some(unsafe {
            self.as_bytes()
                .as_flattened()
                .get_unchecked(offset..offset + len)
        })
    }

    /// Mutable bytes of `count` body elements starting at `first` of `group`.
    ///
    /// Returns `None` if the range is not within [`VRegGroup::vl()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn elements_bytes_mut(
        &mut self,
        group: VRegGroup<VLEN>,
        first: u16,
        count: u32,
    ) -> Option<&mut [u8]> {
        if u32::from(first) + count > u32::from(group.vl().get()) {
            cold_path();
            return None;
        }
        let offset = Self::element_offset(group.base, first, group.eew);
        let len = usize::try_from(count).ok()? * usize::from(group.eew.bytes_width());
        // SAFETY: The element range was checked above to lie within `vl` elements of the group,
        // which lie within the register file by the invariant of `VRegGroup`
        Some(unsafe {
            self.as_bytes_mut()
                .as_flattened_mut()
                .get_unchecked_mut(offset..offset + len)
        })
    }

    /// Read element 0 of register `reg` with width `eew`, zero-extended.
    ///
    /// This is the scalar operand of reductions and scalar moves, which is a single register
    /// regardless of `LMUL`. Returns `None` if `eew` is wider than `VLEN`, which never happens for
    /// an element width that doesn't exceed `ELEN`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn read_first(&self, reg: VReg, eew: Eew) -> Option<u64> {
        if u32::from(eew.bytes_width()) > VLEN.bytes() {
            cold_path();
            return None;
        }
        // SAFETY: Element 0 of `reg` occupies the first `eew.bytes_width() <= VLEN.bytes()` bytes
        // of the register, which is within the register file
        Some(unsafe { self.read_element(reg, 0, eew) })
    }

    /// Write the low bits of `value` into element 0 of register `reg` with width `eew`.
    ///
    /// Returns `None` if `eew` is wider than `VLEN`, see [`Self::read_first()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn write_first(&mut self, reg: VReg, eew: Eew, value: u64) -> Option<()> {
        if u32::from(eew.bytes_width()) > VLEN.bytes() {
            cold_path();
            return None;
        }
        // SAFETY: Element 0 of `reg` occupies the first `eew.bytes_width() <= VLEN.bytes()` bytes
        // of the register, which is within the register file
        unsafe {
            self.write_element(reg, 0, eew, value);
        }
        Some(())
    }

    /// [`Self::read()`] with the element width known at compile time, which makes the access a
    /// fixed-size load.
    ///
    /// Returns `None` if `EEW` is not the element width of `group` or `elem_i` is not below
    /// [`VRegGroup::vl()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn read_const<const EEW: Eew>(
        &self,
        group: VRegGroup<VLEN>,
        elem_i: u16,
    ) -> Option<u64> {
        if group.eew != EEW || u32::from(elem_i) >= u32::from(group.vl.get()) {
            cold_path();
            return None;
        }
        // SAFETY: `elem_i < vl <= vlmax`, and `vlmax` elements of `EEW` of the group lie within
        // the register file by the invariant of `VRegGroup`
        Some(unsafe { self.read_element_const::<EEW>(group.base, elem_i) })
    }

    /// [`Self::write()`] with the element width known at compile time, which makes the access a
    /// fixed-size store.
    ///
    /// Returns `None` if `EEW` is not the element width of `group` or `elem_i` is not below
    /// [`VRegGroup::vl()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn write_const<const EEW: Eew>(
        &mut self,
        group: VRegGroup<VLEN>,
        elem_i: u16,
        value: u64,
    ) -> Option<()> {
        if group.eew != EEW || u32::from(elem_i) >= u32::from(group.vl.get()) {
            cold_path();
            return None;
        }
        // SAFETY: `elem_i < vl <= vlmax`, and `vlmax` elements of `EEW` of the group lie within
        // the register file by the invariant of `VRegGroup`
        unsafe {
            self.write_element_const::<EEW>(group.base, elem_i, value);
        }
        Some(())
    }

    /// Read element `elem_i` of the register group starting at `base_reg`, with `eew`-wide
    /// elements, zero-extended.
    ///
    /// # Safety
    /// The element must lie within the register file, which holds for any `elem_i < vl` of a
    /// register group `[base_reg, base_reg + group_regs)` that ends within the register file,
    /// since `vl <= group_regs * VLENB / eew.bytes_width()`.
    #[inline(always)]
    const unsafe fn read_element<W>(&self, base_reg: VReg, elem_i: u16, eew: W) -> u64
    where
        W: [const] Into<Eew> + [const] Destruct,
    {
        // SAFETY: Guaranteed by the caller's precondition
        unsafe {
            match eew.into() {
                Eew::E8 => self.read_element_const::<{ Eew::E8 }>(base_reg, elem_i),
                Eew::E16 => self.read_element_const::<{ Eew::E16 }>(base_reg, elem_i),
                Eew::E32 => self.read_element_const::<{ Eew::E32 }>(base_reg, elem_i),
                Eew::E64 => self.read_element_const::<{ Eew::E64 }>(base_reg, elem_i),
            }
        }
    }

    /// Write the low `eew.bytes_width()` bytes of `value` into element `elem_i` of the register
    /// group starting at `base_reg`, with `eew`-wide elements.
    ///
    /// # Safety
    /// Same as [`Self::read_element()`]
    #[inline(always)]
    const unsafe fn write_element<W>(&mut self, base_reg: VReg, elem_i: u16, eew: W, value: u64)
    where
        W: [const] Into<Eew> + [const] Destruct,
    {
        // SAFETY: Guaranteed by the caller's precondition
        unsafe {
            match eew.into() {
                Eew::E8 => self.write_element_const::<{ Eew::E8 }>(base_reg, elem_i, value),
                Eew::E16 => self.write_element_const::<{ Eew::E16 }>(base_reg, elem_i, value),
                Eew::E32 => self.write_element_const::<{ Eew::E32 }>(base_reg, elem_i, value),
                Eew::E64 => self.write_element_const::<{ Eew::E64 }>(base_reg, elem_i, value),
            }
        }
    }

    /// [`Self::read_element()`] with the element width known at compile time, which makes the
    /// access a fixed-size load
    ///
    /// # Safety
    /// Same as [`Self::read_element()`]
    #[inline(always)]
    const unsafe fn read_element_const<const EEW: Eew>(&self, base_reg: VReg, elem_i: u16) -> u64 {
        let offset = Self::element_offset(base_reg, elem_i, EEW);
        // SAFETY: `offset + EEW.bytes_width() <= 32 * VLENB` by the caller's precondition
        let element = unsafe {
            self.as_bytes()
                .as_flattened()
                .get_unchecked(offset..)
                .first_chunk::<{ EEW_BYTES::<EEW> }>()
                .unwrap_unchecked()
        };
        let mut bytes = 0u64.to_le_bytes();
        if let Some((low, _)) = bytes.split_first_chunk_mut::<{ EEW_BYTES::<EEW> }>() {
            *low = *element;
        }
        u64::from_le_bytes(bytes)
    }

    /// [`Self::write_element()`] with the element width known at compile time, which makes the
    /// access a fixed-size store
    ///
    /// # Safety
    /// Same as [`Self::read_element()`]
    #[inline(always)]
    const unsafe fn write_element_const<const EEW: Eew>(
        &mut self,
        base_reg: VReg,
        elem_i: u16,
        value: u64,
    ) {
        let offset = Self::element_offset(base_reg, elem_i, EEW);
        // SAFETY: `offset + EEW.bytes_width() <= 32 * VLENB` by the caller's precondition
        let element = unsafe {
            self.as_bytes_mut()
                .as_flattened_mut()
                .get_unchecked_mut(offset..)
                .first_chunk_mut::<{ EEW_BYTES::<EEW> }>()
                .unwrap_unchecked()
        };
        if let Some((low, _)) = value
            .to_le_bytes()
            .split_first_chunk::<{ EEW_BYTES::<EEW> }>()
        {
            *element = *low;
        }
    }
}

/// Vector register state.
///
/// This trait contains only methods that implementations genuinely need to provide. Derived
/// accessors for simpler CSRs are in [`VectorRegistersExt`].
///
/// Note that due to Rust type system limitations, you should use [`VectorRegistersExt`] in trait
/// bounds instead of this trait directly or else the solver will fail.
pub const trait VectorRegisters {
    /// Maximum vector element width `ELEN` in bits
    const ELEN: Elen;
    /// Vector register width `VLEN` in bits
    const VLEN: Vlen;

    /// Read the vector register file
    fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }>;

    /// Mutable access to the vector register file
    fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }>;

    /// Check whether vector instructions are currently permitted.
    ///
    /// Returns `false` when `mstatus.VS == Off` (or equivalent like `sstatus`/`vstatus`). In
    /// environments without these status registers, returns `true` always.
    fn vector_instructions_allowed(&self) -> bool;

    /// Mark the vector state as dirty.
    ///
    /// Must set VS to Dirty in `mstatus` (and `sstatus`/`vsstatus` shadows) when those registers
    /// exist. No-op otherwise.
    fn mark_vs_dirty(&mut self);
}

/// Derived convenience accessors for vector CSRs that are simple read/write fields (vstart, vxrm,
/// vxsat, vcsr).
///
/// Intended for types that implement both [`VectorRegisters`] and [`Csrs`].
///
/// NOTE: While the default methods implemented via the [`Csrs`] trait are correct, custom
/// higher-performance implementations are often possible by overriding them and, for example,
/// caching various CSRs as separate pre-decoded values rather than going through a generic code
/// path with XLEN-sized raw CSR values during reads.
pub const trait VectorRegistersExt<Reg>
where
    Self: [const] Csrs<Reg> + [const] VectorRegisters,
    [(); SUPPORTED_ELEN_VLEN::<{ Self::ELEN }, { Self::VLEN }>]:,
    Reg: [const] Register,
{
    /// Initialize the vector state to the recommended default configuration.
    ///
    /// Per spec: `vtype.vill` = 1, remaining `vtype` bits = `0`, `vl` = 0.
    /// `vstart`, `vxrm`, `vxsat` may have arbitrary values at reset but are zeroed here for
    /// deterministic behavior.
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn initialize_vector_state(&mut self) {
        self.set_vector_config(None);
        self.set_vstart(Vstart::ZERO);
        self.set_vxrm(Vxrm::default());
        self.set_vxsat(false);
    }

    /// Get current `vstart`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vstart(&self) -> Vstart {
        let raw = self
            .read_csr(VectorCsr::Vstart.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        Vstart::from(raw as u16)
    }

    /// Set `vstart`.
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vstart(&mut self, vstart: Vstart) {
        let result = self.write_csr(
            VectorCsr::Vstart.to_csr_index(),
            Reg::Type::from(u16::from(vstart)),
        );
        debug_assert!(
            result.is_ok(),
            "Implementation must initialize `vstart` CSR"
        );
    }

    /// Reset `vstart` to zero.
    ///
    /// Per spec, all vector instructions reset `vstart` to zero at the end of execution.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn reset_vstart(&mut self) {
        self.set_vstart(Vstart::ZERO);
    }

    /// Get `vxsat` (single bit)
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vxsat(&self) -> bool {
        let raw = self
            .read_csr(VectorCsr::Vxsat.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        (raw & 1) == 1
    }

    /// Set `vxsat`.
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vxsat(&mut self, vxsat: bool) {
        let masked = Reg::Type::from(u8::from(vxsat));
        let result = self.write_csr(VectorCsr::Vxsat.to_csr_index(), masked);
        debug_assert!(result.is_ok(), "Implementation must initialize `vxsat` CSR");
        // Mirror `vxsat` into `vcsr[0]`, preserving `vcsr[2:1]` (`vxrm`)
        let old_vcsr = self
            .read_csr(VectorCsr::Vcsr.to_csr_index())
            .unwrap_or_default();
        let new_vcsr = (old_vcsr & !Reg::Type::from(1u8)) | masked;
        let result = self.write_csr(VectorCsr::Vcsr.to_csr_index(), new_vcsr);
        debug_assert!(result.is_ok(), "Implementation must initialize `vcsr` CSR");
    }

    /// Get `vxrm`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vxrm(&self) -> Vxrm {
        let raw = self
            .read_csr(VectorCsr::Vxrm.to_csr_index())
            .unwrap_or_default()
            .as_u64();
        Vxrm::from_bits(raw as u8)
    }

    /// Set `vxrm`.
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vxrm(&mut self, vxrm: Vxrm) {
        let masked = Reg::Type::from(vxrm.to_bits());
        let result = self.write_csr(VectorCsr::Vxrm.to_csr_index(), masked);
        debug_assert!(result.is_ok(), "Implementation must initialize `vxrm` CSR");
        // Mirror `vxrm` into `vcsr[2:1]`, preserving `vcsr[0]` (`vxsat`)
        let old_vcsr = self
            .read_csr(VectorCsr::Vcsr.to_csr_index())
            .unwrap_or_default();
        let new_vcsr = (old_vcsr & !Reg::Type::from(0b110u8)) | (masked << 1u8);
        let result = self.write_csr(VectorCsr::Vcsr.to_csr_index(), new_vcsr);
        debug_assert!(result.is_ok(), "Implementation must initialize `vcsr` CSR");
    }

    /// Get the current vector configuration, `None` when `vill` is set.
    ///
    /// This is the only source of `vtype` and `vl` for instructions. An instruction reads it once
    /// and derives everything from that single value, so even an implementation that returns
    /// different configurations from different calls can't make it combine a `vl` with a `vtype`
    /// it does not belong to.
    ///
    /// The default implementation decodes the raw `vtype` and `vl` CSRs and treats an inconsistent
    /// pair as `vill`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn vector_config(&self) -> Option<VectorConfig<{ Self::ELEN }, { Self::VLEN }>> {
        let vtype = self.read_csr(VectorCsr::Vtype.to_csr_index()).ok()?;
        let vl = self.read_csr(VectorCsr::Vl.to_csr_index()).ok()?;
        VectorConfig::from_raw::<Reg>(vtype, vl)
    }

    /// Set the vector configuration, `None` sets `vill` (and `vl` to zero).
    ///
    /// The implementation must also make the raw `vtype` and `vl` values available for reads via
    /// Zicsr (writes via Zicsr are not allowed).
    ///
    /// The default implementation ignores writes to uninitialized CSR in release mode and panics in
    /// debug.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic(const))]
    fn set_vector_config(
        &mut self,
        vector_config: Option<VectorConfig<{ Self::ELEN }, { Self::VLEN }>>,
    ) {
        let (vtype_raw, vl_raw) = if let Some(vector_config) = vector_config {
            (
                vector_config.vtype().to_raw::<Reg>(),
                Reg::Type::from(u32::from(vector_config.vl().get())),
            )
        } else {
            (
                Vtype::<{ Self::ELEN }, { Self::VLEN }>::illegal_raw::<Reg>(),
                Reg::Type::from(0u8),
            )
        };

        let result = self.write_csr(VectorCsr::Vtype.to_csr_index(), vtype_raw);
        debug_assert!(result.is_ok(), "Implementation must initialize `vtype` CSR");
        let result = self.write_csr(VectorCsr::Vl.to_csr_index(), vl_raw);
        debug_assert!(result.is_ok(), "Implementation must initialize `vl` CSR");
    }
}

// Convenience for threaded execution
// TODO: Forward generically instead, once the compiler normalizes
//  `<&mut T as VectorRegisters>::VLEN` to `T::VLEN`:
//  https://github.com/rust-lang/rust/issues/161264
#[macro_export]
macro_rules! impl_vector_registers_for_mut_ref {
    ($env:ty, $reg:ty) => {
        impl VectorRegisters for &mut $env {
            const ELEN: Elen = <$env as VectorRegisters>::ELEN;
            const VLEN: Vlen = <$env as VectorRegisters>::VLEN;

            #[inline(always)]
            fn read_vregs(&self) -> &VectorRegisterFile<{ Self::VLEN }> {
                <$env as VectorRegisters>::read_vregs(self)
            }

            #[inline(always)]
            fn write_vregs(&mut self) -> &mut VectorRegisterFile<{ Self::VLEN }> {
                <$env as VectorRegisters>::write_vregs(self)
            }

            #[inline(always)]
            fn vector_instructions_allowed(&self) -> bool {
                <$env as VectorRegisters>::vector_instructions_allowed(self)
            }

            #[inline(always)]
            fn mark_vs_dirty(&mut self) {
                <$env as VectorRegisters>::mark_vs_dirty(self);
            }
        }

        // Every method is forwarded explicitly rather than left to `VectorRegistersExt`'s
        // defaults: those go through `Csrs::write_csr()`, which the blanket `Csrs for &mut T`
        // impl also forwards to `$env`, so an empty impl here would silently observe `$env`'s
        // overrides for some accessors and the trait defaults for others
        impl VectorRegistersExt<$reg> for &mut $env {
            #[inline(always)]
            fn vstart(&self) -> Vstart {
                <$env as VectorRegistersExt<$reg>>::vstart(self)
            }

            #[inline(always)]
            fn set_vstart(&mut self, vstart: Vstart) {
                <$env as VectorRegistersExt<$reg>>::set_vstart(self, vstart);
            }

            #[inline(always)]
            fn reset_vstart(&mut self) {
                <$env as VectorRegistersExt<$reg>>::reset_vstart(self);
            }

            #[inline(always)]
            fn vxsat(&self) -> bool {
                <$env as VectorRegistersExt<$reg>>::vxsat(self)
            }

            #[inline(always)]
            fn set_vxsat(&mut self, vxsat: bool) {
                <$env as VectorRegistersExt<$reg>>::set_vxsat(self, vxsat);
            }

            #[inline(always)]
            fn vxrm(&self) -> Vxrm {
                <$env as VectorRegistersExt<$reg>>::vxrm(self)
            }

            #[inline(always)]
            fn set_vxrm(&mut self, vxrm: Vxrm) {
                <$env as VectorRegistersExt<$reg>>::set_vxrm(self, vxrm);
            }

            #[inline(always)]
            fn vector_config(
                &self,
            ) -> Option<$crate::v::vector_config::VectorConfig<{ Self::ELEN }, { Self::VLEN }>>
            {
                <$env as VectorRegistersExt<$reg>>::vector_config(self)
            }

            #[inline(always)]
            fn set_vector_config(
                &mut self,
                vector_config: Option<
                    $crate::v::vector_config::VectorConfig<{ Self::ELEN }, { Self::VLEN }>,
                >,
            ) {
                <$env as VectorRegistersExt<$reg>>::set_vector_config(self, vector_config);
            }

            #[inline(always)]
            fn initialize_vector_state(&mut self) {
                <$env as VectorRegistersExt<$reg>>::initialize_vector_state(self);
            }
        }
    };
}
