use crate::v::vector_config::{BoundedVl, VectorConfig};
use crate::v::vector_registers::{
    SegmentElement, SegmentField, VRegGroup, VRegSegmentGroup, VectorRegisterFile,
};
use ab_riscv_primitives::prelude::*;

const ELEN: Elen = Elen::L64;
const VLEN: Vlen = Vlen::L128;
const VLENB: usize = 16;

const EEWS: [Eew; 4] = [Eew::E8, Eew::E16, Eew::E32, Eew::E64];

/// Every valid `vtype` with `vta = vma = false`
fn all_vtypes() -> impl Iterator<Item = Vtype<ELEN, VLEN>> {
    (0..4u64).flat_map(|vsew| {
        [0b000u64, 0b001, 0b010, 0b011, 0b101, 0b110, 0b111]
            .into_iter()
            .filter_map(move |vlmul| Vtype::from_raw::<Reg<u64>>(vlmul | (vsew << 3)))
    })
}

fn all_vregs() -> impl Iterator<Item = VReg> {
    (0..32).map(|bits| VReg::from_bits(bits).unwrap())
}

fn config(vtype: Vtype<ELEN, VLEN>, vl: u32) -> VectorConfig<ELEN, VLEN> {
    VectorConfig::new(vtype, Vl::new(vl).unwrap()).unwrap()
}

#[test]
fn group_exists_for_legal_aligned_emul_only() {
    for vtype in all_vtypes() {
        let config = config(vtype, u32::from(vtype.vlmax()));
        for eew in EEWS {
            let emul = vtype.vlmul().emul(eew, vtype.vsew());
            for base in all_vregs() {
                let group = VRegGroup::new(config, base, eew);
                let expected = emul.filter(|emul| base.is_group_aligned(emul.register_count()));
                assert_eq!(
                    group.map(VRegGroup::emul),
                    expected,
                    "{vtype:?} {eew:?} {base:?}"
                );
                if let Some(group) = group {
                    assert_eq!(group.base(), base);
                    assert_eq!(group.eew(), eew);
                    assert_eq!(group.vl(), config.vl());
                    assert_eq!(group.vlmax(), config.vlmax());
                }
            }
        }
    }
}

#[test]
fn group_rejects_eew_above_elen() {
    let vtype = Vtype::<{ Elen::L32 }, VLEN>::from_raw::<Reg<u64>>(0b010 << 3).unwrap();
    let config = VectorConfig::new(vtype, vtype.vlmax()).unwrap();
    assert!(VRegGroup::new(config, VReg::V8, Eew::E32).is_some());
    assert!(VRegGroup::new(config, VReg::V8, Eew::E64).is_none());
}

/// Every element below `vlmax` and nothing else is accessible, `read()` and `write()` stop at `vl`
#[test]
fn group_elements_are_within_register_file() {
    for vtype in all_vtypes() {
        let vlmax = u32::from(vtype.vlmax());
        for vl in [0, 1, vlmax / 2, vlmax] {
            let config = config(vtype, vl);
            for eew in EEWS {
                for base in all_vregs() {
                    let Some(group) = VRegGroup::new(config, base, eew) else {
                        continue;
                    };
                    let mut vregs = VectorRegisterFile::<VLEN>::default();
                    for elem_i in 0..u16::try_from(vlmax).unwrap() {
                        let in_body = u32::from(elem_i) < vl;
                        assert_eq!(vregs.write(group, elem_i, u64::MAX).is_some(), in_body);
                        assert_eq!(vregs.read(group, elem_i).is_some(), in_body);
                        assert!(vregs.read_up_to_vlmax(group, elem_i).is_some());
                    }
                    let vlmax = u16::try_from(vlmax).unwrap();
                    assert!(vregs.read_up_to_vlmax(group, vlmax).is_none());
                    assert!(vregs.write(group, vlmax, 0).is_none());

                    // Writes stay within the bytes of the body elements of the group
                    let group_start = usize::from(base.to_bits()) * VLENB;
                    let body_end =
                        group_start + usize::try_from(vl).unwrap() * usize::from(eew.bytes_width());
                    for (offset, byte) in vregs.as_bytes().as_flattened().iter().enumerate() {
                        let expected = if (group_start..body_end).contains(&offset) {
                            u8::MAX
                        } else {
                            0
                        };
                        assert_eq!(*byte, expected, "{vtype:?} {eew:?} {base:?} {offset}");
                    }
                }
            }
        }
    }
}

#[test]
fn group_read_write_const() {
    let vtype = Vtype::<ELEN, VLEN>::from_raw::<Reg<u64>>(0b010 << 3).unwrap();
    let config = config(vtype, 4);
    let group = VRegGroup::new(config, VReg::V4, Eew::E32).unwrap();
    let mut vregs = VectorRegisterFile::<VLEN>::default();
    for elem_i in 0..4 {
        vregs
            .write_const::<{ Eew::E32 }>(group, elem_i, u64::from(elem_i) + 0x1_0000_0001)
            .unwrap();
    }
    for elem_i in 0..4 {
        assert_eq!(
            vregs.read_const::<{ Eew::E32 }>(group, elem_i),
            Some(u64::from(elem_i) + 1)
        );
        assert_eq!(vregs.read(group, elem_i), Some(u64::from(elem_i) + 1));
    }
    assert_eq!(vregs.read_const::<{ Eew::E32 }>(group, 4), None);
    // A different element width is rejected rather than reinterpreting the group
    assert_eq!(vregs.read_const::<{ Eew::E16 }>(group, 0), None);
    assert_eq!(vregs.write_const::<{ Eew::E64 }>(group, 0, 0), None);
}

#[test]
fn group_with_vl() {
    let vtype = Vtype::<ELEN, VLEN>::from_raw::<Reg<u64>>(0b000 << 3).unwrap();
    let vl8 = config(vtype, 8);
    let vl16 = config(vtype, 16);
    let group = VRegGroup::new(vl8, VReg::V1, Eew::E8).unwrap();
    let other = VRegGroup::new(vl16, VReg::V2, Eew::E8).unwrap();

    assert_eq!(
        group.with_same_vl(vl8.vl()).map(VRegGroup::vl),
        Some(vl8.vl())
    );
    assert!(group.with_same_vl(vl16.vl()).is_none());
    assert!(other.with_same_vl(vl8.vl()).is_none());

    let shorter = group.with_vl(Vl::new(3).unwrap()).unwrap();
    assert_eq!(shorter.vl().get(), Vl::new(3).unwrap());
    assert_eq!(shorter.vlmax(), group.vlmax());
    assert!(group.with_vl(Vl::new(9).unwrap()).is_none());
}

#[test]
fn whole_register_groups() {
    for (group_regs, eew) in [
        (VRegGroupSize::R1, Eew::E8),
        (VRegGroupSize::R2, Eew::E16),
        (VRegGroupSize::R4, Eew::E32),
        (VRegGroupSize::R8, Eew::E64),
    ] {
        for base in all_vregs() {
            let group = VRegGroup::<VLEN>::whole_registers(base, group_regs, eew);
            assert_eq!(group.is_some(), base.is_group_aligned(group_regs));
            let Some(group) = group else {
                continue;
            };
            let elements = u32::from(group_regs.get()) * u32::try_from(VLENB).unwrap()
                / u32::from(eew.bytes_width());
            assert_eq!(u32::from(group.vl().get()), elements);
            assert_eq!(u32::from(group.vlmax()), elements);
            assert_eq!(group.group_regs(), group_regs);
        }
    }
}

#[test]
fn group_overlaps_and_contains() {
    let vtype = Vtype::<ELEN, VLEN>::from_raw::<Reg<u64>>(0b010 | (0b010 << 3)).unwrap();
    let config = config(vtype, 1);
    // LMUL=4
    let group = VRegGroup::new(config, VReg::V8, Eew::E32).unwrap();
    assert!(!group.contains(VReg::V7));
    assert!(group.contains(VReg::V8));
    assert!(group.contains(VReg::V11));
    assert!(!group.contains(VReg::V12));

    // EMUL=2
    let narrow_low = VRegGroup::new(config, VReg::V8, Eew::E16).unwrap();
    let narrow_high = VRegGroup::new(config, VReg::V10, Eew::E16).unwrap();
    let narrow_outside = VRegGroup::new(config, VReg::V12, Eew::E16).unwrap();
    assert!(group.overlaps(narrow_low) && narrow_low.overlaps(group));
    assert!(group.overlaps(narrow_high) && narrow_high.overlaps(group));
    assert!(!group.overlaps(narrow_outside) && !narrow_outside.overlaps(group));
}

/// Register file with byte `i` of flattened registers set to `i + 1` (wrapping)
fn numbered_vregs() -> VectorRegisterFile<VLEN> {
    let mut vregs = VectorRegisterFile::<VLEN>::default();
    for (byte, value) in vregs
        .as_bytes_mut()
        .as_flattened_mut()
        .iter_mut()
        .zip((0..=u8::MAX).cycle().skip(1))
    {
        *byte = value;
    }
    vregs
}

#[test]
fn copy_elements() {
    // LMUL=1, SEW=8
    let vtype = Vtype::<ELEN, VLEN>::from_raw::<Reg<u64>>(0b000 << 3).unwrap();
    let config = config(vtype, 8);
    let dst = VRegGroup::new(config, VReg::V1, Eew::E8).unwrap();
    let src = VRegGroup::new(config, VReg::V2, Eew::E8).unwrap();

    // Source elements up to `vlmax`, destination elements up to `vl`
    let mut vregs = numbered_vregs();
    assert!(vregs.copy_elements(dst, 2, src, 10, 6));
    assert_eq!(
        vregs.get(VReg::V1),
        &[
            17, 18, 43, 44, 45, 46, 47, 48, 25, 26, 27, 28, 29, 30, 31, 32
        ]
    );

    // Within the same group, like `memmove`
    let mut vregs = numbered_vregs();
    assert!(vregs.copy_elements(src, 1, src, 0, 7));
    assert_eq!(
        vregs.get(VReg::V2)[..9],
        [33, 33, 34, 35, 36, 37, 38, 39, 41]
    );

    // Out of bounds ranges or mismatched element widths write nothing
    let mut vregs = numbered_vregs();
    assert!(!vregs.copy_elements(dst, 3, src, 0, 6));
    assert!(!vregs.copy_elements(dst, 0, src, 11, 6));
    let wider = VRegGroup::new(config, VReg::V4, Eew::E16).unwrap();
    assert!(!vregs.copy_elements(dst, 0, wider, 0, 1));
    assert_eq!(vregs.as_bytes(), numbered_vregs().as_bytes());
}

#[test]
fn elements_bytes() {
    let vtype = Vtype::<ELEN, VLEN>::from_raw::<Reg<u64>>(0b000 << 3).unwrap();
    let config = config(vtype, 8);
    let src = VRegGroup::new(config, VReg::V2, Eew::E8).unwrap();
    let mut vregs = VectorRegisterFile::<VLEN>::default();

    assert_eq!(vregs.elements_bytes(src, 1, 7).map(<[u8]>::len), Some(7));
    assert!(vregs.elements_bytes(src, 1, 8).is_none());
    assert!(vregs.elements_bytes_mut(src, 0, 9).is_none());
}

#[test]
fn first_element() {
    let mut vregs = VectorRegisterFile::<VLEN>::default();
    vregs.write_first(VReg::V31, Eew::E64, u64::MAX).unwrap();
    assert_eq!(
        vregs.read_first(VReg::V31, Eew::E32),
        Some(u64::from(u32::MAX))
    );
    assert_eq!(vregs.get(VReg::V31)[8..], [0; 8]);

    // An element wider than a register doesn't exist
    let mut vregs = VectorRegisterFile::<{ Vlen::L32 }>::default();
    assert!(vregs.read_first(VReg::V0, Eew::E32).is_some());
    assert!(vregs.read_first(VReg::V0, Eew::E64).is_none());
    assert!(vregs.write_first(VReg::V0, Eew::E64, 0).is_none());
}

#[test]
fn segment_groups() {
    // LMUL=2
    let vtype = Vtype::<ELEN, VLEN>::from_raw::<Reg<u64>>(0b001).unwrap();
    let config = config(vtype, 4);
    let first = VRegGroup::new(config, VReg::V4, Eew::E8).unwrap();

    let segment = VRegSegmentGroup::new(first, Nf::N4).unwrap();
    assert!(
        segment
            .fields()
            .map(VRegGroup::base)
            .eq([VReg::V4, VReg::V6, VReg::V8, VReg::V10])
    );
    assert!(segment.fields().all(|field| field.vl() == first.vl()));

    // `NFIELDS * EMUL` above 8
    assert!(VRegSegmentGroup::new(first, Nf::N5).is_none());
    // Beyond the end of the register file
    let last = VRegGroup::new(config, VReg::V28, Eew::E8).unwrap();
    assert!(VRegSegmentGroup::new(last, Nf::N2).is_some());
    assert!(VRegSegmentGroup::new(last, Nf::N3).is_none());

    // Field 2 is `v8`
    assert_eq!(segment.field(2).map(VRegGroup::base), Some(VReg::V8));
    assert!(segment.field(4).is_none());

    // Elements from `start` to `vl`
    assert!(segment.elements(1).map(SegmentElement::index).eq(1..4));
    assert_eq!(segment.elements(4).count(), 0);
    assert_eq!(segment.elements(u16::MAX).count(), 0);

    // All fields of an element at once, entries beyond the number of fields are zero
    let vregs = numbered_vregs();
    let element = segment.elements(1).next().unwrap();
    assert_eq!(
        element.read_fields(&vregs),
        [4 * 16 + 2, 6 * 16 + 2, 8 * 16 + 2, 10 * 16 + 2, 0, 0, 0, 0]
    );
    let mut vregs = numbered_vregs();
    element.write_fields(&mut vregs, &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(element.read_fields(&vregs), [1, 2, 3, 4, 0, 0, 0, 0]);
    // Only the element itself is written
    assert_eq!(vregs.get(VReg::V4)[..3], [4 * 16 + 1, 1, 4 * 16 + 3]);
    assert_eq!(vregs.get(VReg::V12), numbered_vregs().get(VReg::V12));

    // Fields of an element one at a time
    let mut vregs = numbered_vregs();
    assert!(element.fields().map(SegmentField::index).eq(0..4));
    let field = element.fields().nth(3).unwrap();
    field.write(&mut vregs, 0xab);
    assert_eq!(vregs.get(VReg::V10)[1], 0xab);
    assert_eq!(element.read_fields(&vregs)[3], 0xab);

    // Indices of an indexed access, only with the same `vl`
    let vregs = numbered_vregs();
    let index = VRegGroup::new(config, VReg::V16, Eew::E16).unwrap();
    let indexed = segment.with_index(index).unwrap();
    let element = indexed.elements(1).next().unwrap();
    assert_eq!(element.element().index(), 1);
    // Bytes of `v16` start at 256, where the numbering wraps around
    assert_eq!(
        element.read_index(&vregs),
        u64::from(u16::from_le_bytes([3, 4]))
    );
    assert_eq!(indexed.elements(0).count(), 4);
    let shorter = index.with_vl(Vl::new(3).unwrap()).unwrap();
    assert!(segment.with_index(shorter).is_none());

    let single = VRegSegmentGroup::single(first);
    assert_eq!(single.fields().count(), 1);
}

#[test]
fn bounded_vl() {
    assert!(BoundedVl::<VLEN>::new(Vl::new(128).unwrap()).is_some());
    assert!(BoundedVl::<VLEN>::new(Vl::new(129).unwrap()).is_none());
    let vl = BoundedVl::<VLEN>::new(Vl::new(5).unwrap()).unwrap();
    assert!(vl.indices().eq(0..5));
}
