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
#[should_panic = "Input value exceeds 24 bits"]
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
#[should_panic = "Input value exceeds 24 bits"]
fn i24_from_i32_overflow_positive_panics_in_debug() {
    // 0x0080_0000 = 8_388_608, one above max positive I24
    let _: I24 = I24::from_i32(0x0080_0000);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic = "Input value exceeds 24 bits"]
fn i24_from_i32_overflow_negative_panics_in_debug() {
    // -8_388_609, one below min I24
    let _: I24 = I24::from_i32(-0x0080_0001);
}

#[test]
fn i24_default_is_zero() {
    assert_eq!(I24::default().to_i32(), 0);
}

// I24WithZeroedBits

// LOW_ZEROED_BITS = 8 (the smallest supported, all 24 stored bits are significant)

#[test]
fn i24_with_zeroed_bits_eight_bits_zero_roundtrip() {
    assert_eq!(I24WithZeroedBits::<8>::from_i32(0).to_i32(), 0);
}

#[test]
fn i24_with_zeroed_bits_eight_bits_max_representable_positive() {
    let v = 0x7FFF_FF00_i32;
    assert_eq!(I24WithZeroedBits::<8>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_eight_bits_min_representable_negative() {
    let v = i32::MIN;
    assert_eq!(I24WithZeroedBits::<8>::from_i32(v).to_i32(), v);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic = "Input has non-zero low bits"]
fn i24_with_zeroed_bits_eight_bits_unaligned_panics_in_debug() {
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<8>::from_i32(0x0000_0180);
}

// LOW_ZEROED_BITS = 12 (as used by `lui` and `auipc`)

#[test]
fn i24_with_zeroed_bits_twelve_bits_aligned_positive_roundtrip() {
    let v = 0x0000_1000_i32;
    assert_eq!(I24WithZeroedBits::<12>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_twelve_bits_aligned_negative_roundtrip() {
    let v = -0x0000_1000_i32;
    assert_eq!(I24WithZeroedBits::<12>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_twelve_bits_max_representable_positive() {
    let v = 0x7FFF_F000_i32;
    assert_eq!(I24WithZeroedBits::<12>::from_i32(v).to_i32(), v);
}

#[test]
fn i24_with_zeroed_bits_twelve_bits_min_representable_negative() {
    let v = i32::MIN;
    assert_eq!(I24WithZeroedBits::<12>::from_i32(v).to_i32(), v);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic = "Input has non-zero low bits"]
fn i24_with_zeroed_bits_twelve_bits_unaligned_positive_panics_in_debug() {
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<12>::from_i32(0x0000_1800);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic = "Input has non-zero low bits"]
fn i24_with_zeroed_bits_twelve_bits_unaligned_negative_panics_in_debug() {
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<12>::from_i32(-1);
}

#[test]
fn i24_with_zeroed_bits_twelve_bits_mask_zeroes_low_bits() {
    // to_i32 must always return a value with low 12 bits clear for aligned inputs
    for raw in [i32::MIN, -0x0000_1000, 0, 0x0000_1000, 0x7FFF_F000] {
        let out = I24WithZeroedBits::<12>::from_i32(raw).to_i32();
        assert_eq!(out & 0xFFF, 0, "low bits not zeroed for input {raw}");
    }
}

// LOW_ZEROED_BITS = 31 (the largest supported, only the sign bit is significant)

#[test]
fn i24_with_zeroed_bits_thirty_one_bits_roundtrip() {
    for v in [0, i32::MIN] {
        assert_eq!(I24WithZeroedBits::<31>::from_i32(v).to_i32(), v);
    }
}

#[test]
#[cfg(debug_assertions)]
#[should_panic = "Input has non-zero low bits"]
fn i24_with_zeroed_bits_thirty_one_bits_unaligned_panics_in_debug() {
    let _: I24WithZeroedBits<_> = I24WithZeroedBits::<31>::from_i32(i32::MAX);
}

#[test]
fn i24_with_zeroed_bits_default_is_zero() {
    assert_eq!(I24WithZeroedBits::<12>::default().to_i32(), 0);
}
