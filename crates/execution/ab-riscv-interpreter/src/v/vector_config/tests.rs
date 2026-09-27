use crate::v::vector_config::VectorConfig;
use ab_riscv_primitives::prelude::*;

const VSEWS: [Vsew; 4] = [Vsew::E8, Vsew::E16, Vsew::E32, Vsew::E64];
const VLMULS: [Vlmul; 7] = [
    Vlmul::Mf8,
    Vlmul::Mf4,
    Vlmul::Mf2,
    Vlmul::M1,
    Vlmul::M2,
    Vlmul::M4,
    Vlmul::M8,
];

/// Every valid `vtype` with `vta = vma = false`
fn all_vtypes<const ELEN: Elen, const VLEN: Vlen>() -> impl Iterator<Item = Vtype<ELEN, VLEN>>
where
    [(); SUPPORTED_ELEN_VLEN::<ELEN, VLEN>]:,
{
    VSEWS.into_iter().flat_map(|vsew| {
        VLMULS.into_iter().filter_map(move |vlmul| {
            let raw = u64::from(vlmul.to_bits()) | (u64::from(vsew.to_bits()) << 3);
            Vtype::from_raw::<Reg<u64>>(raw)
        })
    })
}

/// The properties unsafe code relies on to access vector register elements without bounds checks
fn check_vector_config_invariants<const ELEN: Elen, const VLEN: Vlen>()
where
    [(); SUPPORTED_ELEN_VLEN::<ELEN, VLEN>]:,
{
    let mut count = 0;
    for vtype in all_vtypes::<ELEN, VLEN>() {
        count += 1;
        let vlmax = vtype.vlmax();
        let vlmax_u32 = u32::from(vlmax);

        let config = VectorConfig::new(vtype, vlmax).unwrap();
        assert_eq!(config.vl(), vlmax);
        assert_eq!(config.vlmax(), vlmax);
        assert_eq!(config.vtype(), vtype);
        // Not representable at all for the largest `VLMAX`
        if let Some(vl) = Vl::new(vlmax_u32 + 1) {
            assert!(VectorConfig::new(vtype, vl).is_none());
        }

        for avl in [
            0,
            1,
            vlmax_u32 - 1,
            vlmax_u32,
            vlmax_u32 + 1,
            2 * vlmax_u32,
            u32::MAX,
        ] {
            let config = VectorConfig::from_avl(vtype, Vl::new_saturating(avl));
            assert_eq!(
                u32::from(config.vl()),
                avl.min(vlmax_u32),
                "{vtype:?} {avl}"
            );
        }

        let config = VectorConfig::from_avl(vtype, Vl::new(3).unwrap());
        let expected = 3.min(vlmax_u32);
        assert_eq!(
            u32::from(config.with_vl_at_most(Vl::new(2).unwrap()).vl()),
            2.min(expected)
        );
        assert_eq!(
            u32::from(config.with_vl_at_most(Vl::new(100).unwrap()).vl()),
            expected
        );
    }
    assert!(count > 0);
}

#[test]
fn vector_config_invariants() {
    check_vector_config_invariants::<{ Elen::L64 }, { Vlen::L64 }>();
    check_vector_config_invariants::<{ Elen::L64 }, { Vlen::L128 }>();
    check_vector_config_invariants::<{ Elen::L64 }, { Vlen::L256 }>();
    check_vector_config_invariants::<{ Elen::L64 }, { Vlen::L1024 }>();
    check_vector_config_invariants::<{ Elen::L64 }, { Vlen::L65_536 }>();
    check_vector_config_invariants::<{ Elen::L32 }, { Vlen::L32 }>();
    check_vector_config_invariants::<{ Elen::L32 }, { Vlen::L128 }>();
}

#[test]
fn vector_config_from_raw_fails_closed() {
    type Config = VectorConfig<{ Elen::L64 }, { Vlen::L128 }>;
    // e64, m1: VLMAX = 2
    let vtype = u64::from(Vsew::E64.to_bits()) << 3;
    let vill = 1 << 63;

    assert_eq!(
        Config::from_raw::<Reg<u64>>(vtype, 2).map(|config| u32::from(config.vl())),
        Some(2)
    );
    // `vl` above `VLMAX` is not corrected, it is an inconsistent state
    assert!(Config::from_raw::<Reg<u64>>(vtype, 3).is_none());
    assert!(Config::from_raw::<Reg<u64>>(vtype, 1 << 32).is_none());
    assert!(Config::from_raw::<Reg<u64>>(vtype, u64::MAX).is_none());
    assert!(Config::from_raw::<Reg<u64>>(vill, 0).is_none());
    assert!(Config::from_raw::<Reg<u64>>(vill | vtype, 1).is_none());
}
