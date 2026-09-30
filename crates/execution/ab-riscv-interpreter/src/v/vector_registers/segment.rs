//! Register groups of segment loads and stores, and their elements and fields

use crate::v::vector_registers::{VRegGroup, VectorRegisterFile};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Register group of a segment load or store: `nf` fields, each a [`VRegGroup`], in consecutive
/// register groups starting with the first one.
#[derive(Debug, Clone, Copy)]
pub struct VRegSegmentGroup<const VLEN: Vlen> {
    // Invariant: all `nf` field groups lie within the register file
    first: VRegGroup<VLEN>,
    nf: Nf,
}

impl<const VLEN: Vlen> VRegSegmentGroup<VLEN> {
    /// Segment group of `nf` fields, the first of which is `first`.
    ///
    /// Returns `None` if `NFIELDS * EMUL` exceeds 8 or the fields don't fit into the register
    /// file, each of which makes an instruction illegal.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new(first: VRegGroup<VLEN>, nf: Nf) -> Option<Self> {
        let regs = u32::from(nf.fields_per_segment()) * u32::from(first.group_regs().get());
        // Per spec, `NFIELDS * EMUL` must not exceed 8 for segment loads/stores, regardless of
        // whether the field groups would otherwise fit within the 32 vector registers
        if regs > 8 || u32::from(first.base.to_bits()) + regs > 32 {
            cold_path();
            return None;
        }
        Some(Self { first, nf })
    }

    /// Group of a single field, as used by loads and stores that are not segment ones
    #[inline(always)]
    pub const fn single(group: VRegGroup<VLEN>) -> Self {
        Self {
            first: group,
            nf: Nf::N1,
        }
    }

    /// The first field
    #[inline(always)]
    pub const fn first(self) -> VRegGroup<VLEN> {
        self.first
    }

    /// Number of fields
    #[inline(always)]
    pub const fn nf(self) -> Nf {
        self.nf
    }

    /// Register group of field `field`, `None` if it is not below the number of fields
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn field(self, field: u8) -> Option<VRegGroup<VLEN>> {
        if field >= self.nf.fields_per_segment() {
            cold_path();
            return None;
        }
        // SAFETY: Checked above
        let base = unsafe { self.field_base(field) };
        Some(VRegGroup { base, ..self.first })
    }

    /// First register of field `field`.
    ///
    /// # Safety
    /// `field` must be below the number of fields.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    const unsafe fn field_base(self, field: u8) -> VReg {
        // SAFETY: Field `field < nf` lies within the register file by the invariant and the
        // caller's precondition
        unsafe {
            VReg::from_bits(self.first.base.to_bits() + field * self.first.group_regs().get())
                .unwrap_unchecked()
        }
    }

    /// Register groups of all fields in order
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn fields(self) -> impl Iterator<Item = VRegGroup<VLEN>> {
        (0..self.nf.fields_per_segment()).filter_map(move |field| self.field(field))
    }

    /// Body elements `start..vl`, each of which gives access to its fields.
    ///
    /// The index of an element is below `vl` by construction, so access to its fields doesn't
    /// need the compiler to prove anything about indices, unlike [`VectorRegisterFile::read()`].
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn elements(self, start: u16) -> impl Iterator<Item = SegmentElement<VLEN>> {
        // `vl <= VLEN <= 65536`, so every index below it fits into `u16`
        (u32::from(start)..u32::from(self.first.vl.get())).map(move |index| SegmentElement {
            group: self,
            index: u32::truncate(index),
        })
    }

    /// This group together with the group of indices of an indexed load or store.
    ///
    /// Returns `None` if `index` has a different `vl`, which never happens for groups created from
    /// the same configuration.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn with_index(self, index: VRegGroup<VLEN>) -> Option<IndexedSegmentGroup<VLEN>> {
        if index.vl != self.first.vl {
            cold_path();
            return None;
        }
        Some(IndexedSegmentGroup { data: self, index })
    }
}

/// Body element of a [`VRegSegmentGroup`], see [`VRegSegmentGroup::elements()`]
#[derive(Debug, Clone, Copy)]
pub struct SegmentElement<const VLEN: Vlen> {
    group: VRegSegmentGroup<VLEN>,
    // Invariant: below `vl` of `group`
    index: u16,
}

impl<const VLEN: Vlen> SegmentElement<VLEN> {
    /// Element index
    #[inline(always)]
    pub const fn index(self) -> u16 {
        self.index
    }

    /// All fields of this element in order
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn fields(self) -> impl Iterator<Item = SegmentField<VLEN>> {
        (0..self.group.nf.fields_per_segment()).map(move |field| SegmentField {
            element: self,
            field,
        })
    }

    /// Read all fields of this element, zero-extended, into the first `nf` entries, the rest are
    /// zero
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn read_fields(
        self,
        vregs: &VectorRegisterFile<VLEN>,
    ) -> [u64; const { usize::from(Nf::MAX.fields_per_segment()) }] {
        let mut fields = [0; _];
        for (field, value) in (0..self.group.nf.fields_per_segment()).zip(&mut fields) {
            // SAFETY: `field < nf`, the field has the same layout as the first one by the invariant
            // of `VRegSegmentGroup` and the element index is below `vl <= vlmax` by the invariant
            // of `SegmentElement`
            unsafe {
                let base = self.group.field_base(field);
                *value = vregs.read_element(base, self.index, self.group.first.eew);
            }
        }
        fields
    }

    /// Write the low bits of `fields[f]` into field `f` of this element, for each field that
    /// exists, the rest of `fields` is ignored
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn write_fields(self, vregs: &mut VectorRegisterFile<VLEN>, fields: &[u64]) {
        for (field, &value) in (0..self.group.nf.fields_per_segment()).zip(fields) {
            // SAFETY: Same as in `Self::read_fields()`
            unsafe {
                let base = self.group.field_base(field);
                vregs.write_element(base, self.index, self.group.first.eew, value);
            }
        }
    }
}

/// Field of a [`SegmentElement`], see [`SegmentElement::fields()`]
#[derive(Debug, Clone, Copy)]
pub struct SegmentField<const VLEN: Vlen> {
    element: SegmentElement<VLEN>,
    // Invariant: below the number of fields of the group
    field: u8,
}

impl<const VLEN: Vlen> SegmentField<VLEN> {
    /// Field index
    #[inline(always)]
    pub const fn index(self) -> u8 {
        self.field
    }

    /// Write the low bits of `value` into this field
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn write(self, vregs: &mut VectorRegisterFile<VLEN>, value: u64) {
        let element = self.element;
        // SAFETY: `field < nf` by the invariant, the field has the same layout as the first one by
        // the invariant of `VRegSegmentGroup` and the element index is below `vl <= vlmax` by the
        // invariant of `SegmentElement`
        unsafe {
            let base = element.group.field_base(self.field);
            vregs.write_element(base, element.index, element.group.first.eew, value);
        }
    }
}

/// Segment group of an indexed load or store together with its group of indices, see
/// [`VRegSegmentGroup::with_index()`]
#[derive(Debug, Clone, Copy)]
pub struct IndexedSegmentGroup<const VLEN: Vlen> {
    data: VRegSegmentGroup<VLEN>,
    // Invariant: same `vl` as the first field of `data`
    index: VRegGroup<VLEN>,
}

impl<const VLEN: Vlen> IndexedSegmentGroup<VLEN> {
    /// Body elements `start..vl`, see [`VRegSegmentGroup::elements()`]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn elements(self, start: u16) -> impl Iterator<Item = IndexedSegmentElement<VLEN>> {
        self.data
            .elements(start)
            .map(move |element| IndexedSegmentElement {
                element,
                index: self.index,
            })
    }
}

/// Body element of an [`IndexedSegmentGroup`], see [`IndexedSegmentGroup::elements()`]
#[derive(Debug, Clone, Copy)]
pub struct IndexedSegmentElement<const VLEN: Vlen> {
    element: SegmentElement<VLEN>,
    // Invariant: same `vl` as the segment group of `element`
    index: VRegGroup<VLEN>,
}

impl<const VLEN: Vlen> IndexedSegmentElement<VLEN> {
    /// The element of the segment group
    #[inline(always)]
    pub const fn element(self) -> SegmentElement<VLEN> {
        self.element
    }

    /// The index of this element, zero-extended, which is the offset of indexed loads and stores
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn read_index(self, vregs: &VectorRegisterFile<VLEN>) -> u64 {
        // SAFETY: The element index is below `vl` of its segment group by the invariant of
        // `SegmentElement`, which is `vl <= vlmax` of the group of indices by the invariant, and
        // those elements lie within the register file by the invariant of `VRegGroup`
        unsafe { vregs.read_element(self.index.base, self.element.index, self.index.eew) }
    }
}
