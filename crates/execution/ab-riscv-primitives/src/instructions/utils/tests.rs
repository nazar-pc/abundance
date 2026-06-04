use crate::instructions::utils::{I24, I24WithZeroedBits, U24};

// U24

#[test]
fn u24_zero_roundtrip() {
    assert_eq!(U24::from_u32(0).to_u32(), 0);
}

#[test]
fn u24_one_roundtrip() {
    assert_eq!(U24::from_u32(1).to_u32(), 1);
}

#[test]
fn u24_max_value_roundtrip() {
    // 2^24 - 1 = 16_777_215
    let max = 0x00FF_FFFF_u32;
    assert_eq!(U24::from_u32(max).to_u32(), max);
}

#[test]
fn u24_midpoint_roundtrip() {
    let v = 0x0080_0000_u32;
    assert_eq!(U24::from_u32(v).to_u32(), v);
}

#[test]
fn u24_byte_boundary_values() {
    for v in [0x0000_00FF_u32, 0x0000_FF00, 0x00FF_0000] {
        assert_eq!(U24::from_u32(v).to_u32(), v);
    }
}

#[test]
fn u24_alternating_bit_pattern() {
    // 0xAAAAAA fits in 24 bits
    let v = 0x00AA_AAAA_u32;
    assert_eq!(U24::from_u32(v).to_u32(), v);
}

#[test]
fn u24_little_endian_byte_order() {
    // Verify internal storage is LE: from_u32(0x010203) => [0x03, 0x02, 0x01]
    let u = U24::from_u32(0x00_01_02_03);
    assert_eq!(u.0, [0x03, 0x02, 0x01]);
}

#[test]
fn u24_from_u32_into_u32_trait() {
    let v = 0x00AB_CDEF_u32;
    let u = U24::from_u32(v);
    let out: u32 = u.into();
    assert_eq!(out, v);
}

#[test]
fn u24_from_u32_into_u64_trait() {
    let v = 0x00AB_CDEF_u32;
    let u = U24::from_u32(v);
    let out: u64 = u.into();
    assert_eq!(out, u64::from(v));
}

#[test]
fn u24_high_byte_not_leaked() {
    // to_u32 must zero the 4th byte unconditionally
    let u = U24([0xFF, 0xFF, 0xFF]);
    assert_eq!(u.to_u32(), 0x00FF_FFFF);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn u24_from_u32_overflow_panics_in_debug() {
    // 0x0100_0000 exceeds 24 bits
    let _: U24 = U24::from_u32(0x0100_0000);
}

#[test]
fn u24_default_is_zero() {
    assert_eq!(U24::default().to_u32(), 0);
}

// I24

#[test]
fn i24_zero_roundtrip() {
    assert_eq!(I24::from_i32(0).to_i32(), 0);
}

#[test]
fn i24_positive_one_roundtrip() {
    assert_eq!(I24::from_i32(1).to_i32(), 1);
}

#[test]
fn i24_negative_one_roundtrip() {
    assert_eq!(I24::from_i32(-1).to_i32(), -1);
}

#[test]
fn i24_max_positive_roundtrip() {
    // 2^23 - 1 = 8_388_607
    let max = 0x007F_FFFF_i32;
    assert_eq!(I24::from_i32(max).to_i32(), max);
}

#[test]
fn i24_min_negative_roundtrip() {
    // -2^23 = -8_388_608
    let min = -0x0080_0000_i32;
    assert_eq!(I24::from_i32(min).to_i32(), min);
}

#[test]
fn i24_negative_one_stored_as_all_ff() {
    let i = I24::from_i32(-1);
    assert_eq!(i.0, [0xFF, 0xFF, 0xFF]);
}

#[test]
fn i24_sign_extension_positive_boundary() {
    // 0x7FFFFF is max positive; 0x800000 would be the sign bit - must not be stored
    let v = 0x007F_FFFF_i32;
    let recovered = I24::from_i32(v).to_i32();
    assert_eq!(recovered, v);
    assert!(recovered > 0);
}

#[test]
fn i24_sign_extension_negative_boundary() {
    let v = -0x0080_0000_i32;
    let recovered = I24::from_i32(v).to_i32();
    assert_eq!(recovered, v);
    assert!(recovered < 0);
}

#[test]
fn i24_alternating_bit_pattern_positive() {
    // 0x2AAAAA fits in 23 bits (positive)
    let v = 0x002A_AAAA_i32;
    assert_eq!(I24::from_i32(v).to_i32(), v);
}

#[test]
fn i24_alternating_bit_pattern_negative() {
    let v = -0x002A_AAAB_i32;
    assert_eq!(I24::from_i32(v).to_i32(), v);
}

#[test]
fn i24_little_endian_byte_order_positive() {
    // 0x010203: LE bytes [0x03, 0x02, 0x01]
    let i = I24::from_i32(0x0001_0203);
    assert_eq!(i.0, [0x03, 0x02, 0x01]);
}

#[test]
fn i24_into_i32_trait() {
    let v = -42_i32;
    let i = I24::from_i32(v);
    let out: i32 = i.into();
    assert_eq!(out, v);
}

#[test]
fn i24_into_i64_trait() {
    let v = -42_i32;
    let i = I24::from_i32(v);
    let out: i64 = i.into();
    assert_eq!(out, i64::from(v));
}

#[test]
fn i24_to_i32_sign_extends_high_bit() {
    // Manually construct a value with bit 23 set (sign bit for I24)
    let i = I24([0x00, 0x00, 0x80]);
    // Should sign-extend to -8_388_608
    assert_eq!(i.to_i32(), -8_388_608_i32);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_from_i32_overflow_positive_panics_in_debug() {
    // 0x0080_0000 = 8_388_608, one above max positive I24
    let _: I24 = I24::from_i32(0x0080_0000);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_from_i32_overflow_negative_panics_in_debug() {
    // -8_388_609, one below min I24
    let _: I24 = I24::from_i32(-0x0080_0001);
}

#[test]
fn i24_default_is_zero() {
    assert_eq!(I24::default().to_i32(), 0);
}

// I24WithZeroedBits

// LOW_ZEROED_BITS = 0 (degenerate: behaves identically to I24)

#[test]
fn i24_with_zeroed_bits_zero_bits_zero_roundtrip() {
    assert_eq!(I24WithZeroedBits::<0>::from_i32(0).to_i32(), 0);
}

#[test]
fn i24_with_zeroed_bits_zero_bits_positive_roundtrip() {
    let v = 0x007F_FFFF_i32;
    assert_eq!(I24WithZeroedBits::<0>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_zero_bits_negative_roundtrip() {
    let v = -0x0080_0000_i32;
    assert_eq!(I24WithZeroedBits::<0>::from_i32(v).to_i32(), v);
}

// LOW_ZEROED_BITS = 1

#[test]
fn i24_with_zeroed_bits_one_bit_even_value_roundtrip() {
    // Even values have low bit 0, so the round-trip is lossless
    let v = 100_i32;
    assert_eq!(I24WithZeroedBits::<1>::from_i32(v).to_i32(), v);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_one_bit_odd_value_panics_in_debug() {
    // Low bit is set; from_i32 now requires alignment, not truncation
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<1>::from_i32(101);
}

#[test]
fn i24_with_zeroed_bits_one_bit_negative_even_roundtrip() {
    let v = -100_i32;
    assert_eq!(I24WithZeroedBits::<1>::from_i32(v).to_i32(), v);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_one_bit_negative_odd_panics_in_debug() {
    // Low bit is set; from_i32 now requires alignment, not truncation
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<1>::from_i32(-101);
}

#[test]
fn i24_with_zeroed_bits_one_bit_max_representable_positive() {
    // Value must fit in 24 bits and have low bit clear: 0x7FFFFE
    let v = 0x007F_FFFE_i32;
    assert_eq!(I24WithZeroedBits::<1>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_one_bit_min_representable_negative() {
    // -0x00800000 is even, so round-trip is lossless
    let v = -0x0080_0000_i32;
    assert_eq!(I24WithZeroedBits::<1>::from_i32(v).to_i32(), v);
}

// LOW_ZEROED_BITS = 4

#[test]
fn i24_with_zeroed_bits_four_bits_aligned_positive_roundtrip() {
    // Must be a multiple of 16 and fit in 24 bits
    let v = 0x0000_0010_i32;
    assert_eq!(I24WithZeroedBits::<4>::from_i32(v).to_i32(), v);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_four_bits_unaligned_positive_panics_in_debug() {
    // Low nibble is set; from_i32 now requires alignment, not truncation
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<4>::from_i32(0x1F);
}

#[test]
fn i24_with_zeroed_bits_four_bits_aligned_negative_roundtrip() {
    let v = -0x0000_0010_i32;
    assert_eq!(I24WithZeroedBits::<4>::from_i32(v).to_i32(), v);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_four_bits_unaligned_negative_panics_in_debug() {
    // Low nibble is set; from_i32 now requires alignment, not truncation
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<4>::from_i32(-1);
}

#[test]
fn i24_with_zeroed_bits_four_bits_max_representable_positive() {
    // Value must fit in 24 bits with low 4 bits clear: 0x7FFFF0
    let v = 0x007F_FFF0_i32;
    assert_eq!(I24WithZeroedBits::<4>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_four_bits_min_representable_negative() {
    // -0x00800000 is 4-bit aligned and fits in 24 bits
    let v = -0x0080_0000_i32;
    assert_eq!(I24WithZeroedBits::<4>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_four_bits_zero_roundtrip() {
    assert_eq!(I24WithZeroedBits::<4>::from_i32(0).to_i32(), 0);
}

#[test]
fn i24_with_zeroed_bits_four_bits_mask_zeroes_low_bits() {
    // to_i32 must always return a value with low 4 bits clear for aligned inputs
    for raw in [-0x0080_0000_i32, -0x0000_0010, 0, 0x0000_0010, 0x007F_FFF0] {
        let out = I24WithZeroedBits::<4>::from_i32(raw).to_i32();
        assert_eq!(out & 0xF, 0, "low bits not zeroed for input {raw}");
    }
}

#[test]
fn i24_with_zeroed_bits_default_is_zero() {
    assert_eq!(I24WithZeroedBits::<4>::default().to_i32(), 0);
}

// overflow guards

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_four_bits_overflow_positive_panics_in_debug() {
    // Low bits set ensures round-trip mismatch regardless of overflow path
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<4>::from_i32(0x0080_0001);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_four_bits_overflow_negative_panics_in_debug() {
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<4>::from_i32(-0x0800_0001);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic]
fn i24_with_zeroed_bits_one_bit_overflow_positive_panics_in_debug() {
    // After storing, 0x01000000 does not fit in 24 bits
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<1>::from_i32(0x0100_0000);
}
