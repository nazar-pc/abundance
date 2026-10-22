//! Construction and ordering of alignments

use ab_io_type::alignment::Alignment;
use ab_io_type::trivial_type::TrivialType;
use core::any::type_name;

/// Check that [`Alignment::of()`] gives the alignment of `T`
fn check_alignment_of<T>()
where
    T: TrivialType,
{
    assert_eq!(
        usize::from(Alignment::of::<T>().get()),
        align_of::<T>(),
        "{}",
        type_name::<T>()
    );
}

#[test]
fn alignment_from_power() {
    // Only powers up to that of `Alignment::MAX` give alignments
    for power in 0..=u8::MAX {
        assert_eq!(
            Alignment::from_power(power).map(Alignment::get),
            (power <= Alignment::MAX.power).then(|| 1 << power),
            "Power {power}"
        );
    }
}

#[test]
fn alignment_constructors() {
    assert_eq!(Alignment::MAX.power, 4);
    assert_eq!(Alignment::from_power(4), Some(Alignment::MAX));
    for power in 0..=4 {
        assert_eq!(Alignment::from_power(power).unwrap().power, power);
    }

    check_alignment_of::<()>();
    check_alignment_of::<u8>();
    check_alignment_of::<u16>();
    check_alignment_of::<u32>();
    check_alignment_of::<u64>();
    check_alignment_of::<u128>();
    check_alignment_of::<[u64; 3]>();
}

#[test]
fn alignment_order() {
    // Alignments compare like their values in bytes
    for power in 0..=Alignment::MAX.power {
        for other_power in 0..=Alignment::MAX.power {
            let alignment = Alignment::from_power(power).unwrap();
            let other = Alignment::from_power(other_power).unwrap();
            assert_eq!(
                alignment.cmp(&other),
                alignment.get().cmp(&other.get()),
                "Powers {power} and {other_power}"
            );
        }
    }
}
