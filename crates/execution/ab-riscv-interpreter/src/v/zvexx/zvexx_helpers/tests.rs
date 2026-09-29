use crate::v::zvexx::zvexx_helpers::{ExtensionSew, WideningSew};
use ab_riscv_primitives::prelude::*;

/// `(narrow, wide)` element widths of [`WideningSew::new()`] for `ELEN`
fn widening_sew<const ELEN: Elen>(sew: Vsew) -> Option<(Vsew, Vsew)> {
    WideningSew::<ELEN>::new(sew).map(|widening_sew| (widening_sew.narrow(), widening_sew.wide()))
}

#[test]
fn widening_sew_requires_double_width_within_elen() {
    for (sew, elen, expected_wide) in [
        (Vsew::E8, Elen::L64, Some(Vsew::E16)),
        (Vsew::E16, Elen::L64, Some(Vsew::E32)),
        (Vsew::E32, Elen::L64, Some(Vsew::E64)),
        (Vsew::E64, Elen::L64, None),
        (Vsew::E8, Elen::L32, Some(Vsew::E16)),
        (Vsew::E16, Elen::L32, Some(Vsew::E32)),
        // Zve32x: a 64-bit wide operand exceeds `ELEN`
        (Vsew::E32, Elen::L32, None),
        (Vsew::E8, Elen::L16, Some(Vsew::E16)),
        (Vsew::E16, Elen::L16, None),
        (Vsew::E8, Elen::L8, None),
    ] {
        let widening_sew = match elen {
            Elen::L8 => widening_sew::<{ Elen::L8 }>(sew),
            Elen::L16 => widening_sew::<{ Elen::L16 }>(sew),
            Elen::L32 => widening_sew::<{ Elen::L32 }>(sew),
            Elen::L64 => widening_sew::<{ Elen::L64 }>(sew),
            _ => unreachable!("Only ELEN up to 64 is tested"),
        };
        assert_eq!(
            widening_sew,
            expected_wide.map(|wide| (sew, wide)),
            "{sew:?} {elen:?}"
        );
    }
}

#[test]
fn extension_sew_requires_source_of_at_least_8_bits() {
    for (sew, factor, expected_source) in [
        (Vsew::E8, VsewFactor::F2, None),
        (Vsew::E16, VsewFactor::F2, Some(Vsew::E8)),
        (Vsew::E32, VsewFactor::F2, Some(Vsew::E16)),
        (Vsew::E64, VsewFactor::F2, Some(Vsew::E32)),
        (Vsew::E16, VsewFactor::F4, None),
        (Vsew::E32, VsewFactor::F4, Some(Vsew::E8)),
        (Vsew::E64, VsewFactor::F4, Some(Vsew::E16)),
        (Vsew::E32, VsewFactor::F8, None),
        (Vsew::E64, VsewFactor::F8, Some(Vsew::E8)),
    ] {
        let extension_sew = ExtensionSew::new(sew, factor);
        assert_eq!(
            extension_sew.map(ExtensionSew::source),
            expected_source,
            "{sew:?} {factor:?}"
        );
        if let Some(extension_sew) = extension_sew {
            assert_eq!(extension_sew.dest(), sew);
        }
    }
}
