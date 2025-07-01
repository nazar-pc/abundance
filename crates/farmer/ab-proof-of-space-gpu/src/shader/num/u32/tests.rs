use super::*;

#[test]
fn test_from_lo_hi() {
    for num in [0u32, 1, 42, 0x7FFF_FFFF, u32::MAX] {
        let lo = num;
        let hi = 42u32;
        let correct = (u64::from(hi) << u32::BITS) | u64::from(lo);
        let u64_poly = U64::from_lo_hi(lo, hi);
        assert_eq!(u64_poly.to_be_bytes(), correct.to_be_bytes());
    }
}

#[test]
fn test_from_u32() {
    for num in [0u32, 1, 42, 0x7FFF_FFFF, u32::MAX] {
        let u64_poly = U64::from(num);
        assert_eq!(u64_poly.to_be_bytes(), u64::from(num).to_be_bytes());
    }
}

#[test]
fn test_as_u32() {
    for num in [0u32, 1, 42, 0x7FFF_FFFF, u32::MAX] {
        // Create U64 from bytes and verify round-trip
        let u64_poly = U64::from_be_bytes((num as u64).to_be_bytes());
        assert_eq!(u64_poly.as_u32(), num);
    }
}

#[test]
fn test_u64_add() {
    let cases = [
        (0u64, 0u64),
        (1, 2),
        (u32::MAX as u64, 1),
        (u64::MAX - 1, 1),
    ];
    for &(a, b) in &cases {
        let a_u = U64::from_be_bytes(a.to_be_bytes());
        let b_u = U64::from_be_bytes(b.to_be_bytes());
        let sum = a + b;
        let sum_u = a_u + b_u;
        assert_eq!(sum.to_be_bytes(), sum_u.to_be_bytes());
    }
}

#[test]
fn test_u64_sub() {
    let cases = [(2u64, 1u64), (u64::MAX, 1), (1, 0)];
    for &(a, b) in &cases {
        let a_u = U64::from_be_bytes(a.to_be_bytes());
        let b_u = U64::from_be_bytes(b.to_be_bytes());
        let diff = a - b;
        let diff_u = a_u - b_u;
        assert_eq!(diff.to_be_bytes(), diff_u.to_be_bytes());
    }
}

#[test]
fn test_u64_bitwise() {
    let a = 0xFF00FF00FF00FF00u64;
    let b = 0x00FF00FF00FF00FFu64;

    let a_u = U64::from_be_bytes(a.to_be_bytes());
    let b_u = U64::from_be_bytes(b.to_be_bytes());

    assert_eq!((a & b).to_be_bytes(), (a_u & b_u).to_be_bytes());
    assert_eq!((a | b).to_be_bytes(), (a_u | b_u).to_be_bytes());
    assert_eq!((a ^ b).to_be_bytes(), (a_u ^ b_u).to_be_bytes());
}

#[test]
fn test_u64_shifts() {
    let val = 0x0123456789ABCDEFu64;
    let val_u = U64::from_be_bytes(val.to_be_bytes());

    for shift in 0..64_u32 {
        assert_eq!((val << shift).to_be_bytes(), (val_u << shift).to_be_bytes());
        assert_eq!((val >> shift).to_be_bytes(), (val_u >> shift).to_be_bytes());
    }
}

#[test]
fn test_u64_roundtrip_bytes() {
    let values = [0u64, 1u64, u32::MAX as u64, u64::MAX];
    for v in values {
        let bytes = v.to_be_bytes();
        let u = U64::from_be_bytes(bytes);
        assert_eq!(
            unsafe { (&v as *const u64).cast::<[u8; 8]>().read() },
            unsafe { (&u as *const U64).cast::<[u8; 8]>().read() },
        );
        assert_eq!(bytes, u.to_be_bytes());
    }
}

#[test]
fn test_u128_add() {
    let cases = [
        (0u128, 0u128),
        (1, 2),
        (u64::MAX as u128, 1),
        (u128::MAX - 1, 1),
    ];
    for &(a, b) in &cases {
        let a_u = U128::from_be_bytes(a.to_be_bytes());
        let b_u = U128::from_be_bytes(b.to_be_bytes());
        let sum = a + b;
        let sum_u = a_u + b_u;
        assert_eq!(sum.to_be_bytes(), sum_u.to_be_bytes());
    }
}

#[test]
fn test_u128_sub() {
    let cases = [(2u128, 1u128), (u128::MAX, 1), (1, 0)];
    for &(a, b) in &cases {
        let a_u = U128::from_be_bytes(a.to_be_bytes());
        let b_u = U128::from_be_bytes(b.to_be_bytes());
        let diff = a - b;
        let diff_u = a_u - b_u;
        assert_eq!(diff.to_be_bytes(), diff_u.to_be_bytes());
    }
}

#[test]
fn test_u128_bitwise() {
    let a = 0xFF00FF00FF00FF00FF00FF00FF00FF00u128;
    let b = 0x00FF00FF00FF00FF00FF00FF00FF00FFu128;

    let a_u = U128::from_be_bytes(a.to_be_bytes());
    let b_u = U128::from_be_bytes(b.to_be_bytes());

    assert_eq!((a & b).to_be_bytes(), (a_u & b_u).to_be_bytes());
    assert_eq!((a | b).to_be_bytes(), (a_u | b_u).to_be_bytes());
    assert_eq!((a ^ b).to_be_bytes(), (a_u ^ b_u).to_be_bytes());
}

#[test]
fn test_u128_shifts() {
    let val = 0x0123456789ABCDEF0123456789ABCDEFu128;
    let val_u = U128::from_be_bytes(val.to_be_bytes());

    for shift in 0..128_u32 {
        assert_eq!((val << shift).to_be_bytes(), (val_u << shift).to_be_bytes());
        assert_eq!((val >> shift).to_be_bytes(), (val_u >> shift).to_be_bytes());
    }
}

#[test]
fn test_u128_roundtrip_bytes() {
    let values = [0u128, 1u128, u64::MAX as u128, u128::MAX];
    for v in values {
        let bytes = v.to_be_bytes();
        let u = U128::from_be_bytes(bytes);
        assert_eq!(
            unsafe { (&v as *const u128).cast::<[u8; 16]>().read() },
            unsafe { (&u as *const U128).cast::<[u8; 16]>().read() },
        );
        assert_eq!(bytes, u.to_be_bytes());
    }
}

#[test]
fn test_as_be_bytes_to_le_u32_words() {
    let values = [
        0u128,
        1,
        u32::MAX as u128,
        u64::MAX as u128,
        u128::MAX,
        0x0011_2233_4455_6677_8899_aabb_ccdd_eeff,
    ];

    for v in values {
        let u = U128::from_be_bytes(v.to_be_bytes());
        let words = u.as_be_bytes_to_le_u32_words();

        let bytes = v.to_be_bytes();
        let expected = [
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        ];

        assert_eq!(words, expected, "v={v}");
    }
}

#[test]
fn test_from_le_u32_words_as_be_bytes() {
    let values = [
        0u128,
        1,
        u32::MAX as u128,
        u64::MAX as u128,
        u128::MAX,
        0x0011_2233_4455_6677_8899_aabb_ccdd_eeff,
    ];

    for &v in &values {
        let u = U128::from_be_bytes(v.to_be_bytes());
        let words = u.as_be_bytes_to_le_u32_words();
        let reconstructed = U128::from_le_u32_words_as_be_bytes(&words);

        assert_eq!(u.to_be_bytes(), reconstructed.to_be_bytes(), "v={v}");
    }
}
