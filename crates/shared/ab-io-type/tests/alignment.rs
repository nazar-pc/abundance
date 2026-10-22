//! Construction and ordering of alignments

use ab_io_type::alignment::Alignment;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::unaligned::Unaligned;
use core::any::type_name;

/// Check that [`Alignment::of()`] gives `expected` bytes, which is at least the alignment of `T`
fn check_alignment_of<T>(expected: u8)
where
    T: TrivialType,
{
    assert_eq!(Alignment::of::<T>().get(), expected, "{}", type_name::<T>());
    assert!(
        align_of::<T>() <= usize::from(expected),
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

    // Every implementation of `TrivialType` in the crate
    check_alignment_of::<()>(1);
    check_alignment_of::<bool>(1);
    check_alignment_of::<u8>(1);
    check_alignment_of::<u16>(2);
    check_alignment_of::<u32>(4);
    check_alignment_of::<u64>(8);
    check_alignment_of::<u128>(16);
    check_alignment_of::<i8>(1);
    check_alignment_of::<i16>(2);
    check_alignment_of::<i32>(4);
    check_alignment_of::<i64>(8);
    check_alignment_of::<i128>(16);
    check_alignment_of::<[u128; 0]>(16);
    check_alignment_of::<[u64; 3]>(8);
    check_alignment_of::<Unaligned<u128>>(1);
    check_alignment_of::<FixedCapacityElements<u8, 2>>(1);
    check_alignment_of::<FixedCapacityElements<u64, 2>>(8);
    check_alignment_of::<FixedCapacityElements<u128, 2>>(16);
    check_alignment_of::<FixedCapacityString<3>>(1);
}

#[test]
fn alignment_of_derived_types_and_containers() {
    #[derive(Copy, Clone, TrivialType)]
    #[repr(C)]
    struct Wide(u128);

    check_alignment_of::<Wide>(16);
    check_alignment_of::<[Wide; 2]>(16);
    // The length before elements takes 16 bytes, so elements are aligned
    assert_eq!(size_of::<FixedCapacityElements<u128, 2>>(), 48);
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
