//! Tests of `AnyBitPattern` implementations for built-in types and for types with derived
//! `TrivialType`.
//!
//! The assertions are checked at compile time, there is nothing to run. Types that don't implement
//! `AnyBitPattern` are covered by ui tests.

use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};
use ab_io_type::unaligned::Unaligned;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Named {
    a: u8,
    b: [u8; 3],
    c: u32,
    d: Unaligned<u64>,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Tuple(u16, i16, [u32; 2]);

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Empty;

#[derive(Copy, Clone, TrivialType)]
#[repr(transparent)]
struct Newtype(u64);

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Nested {
    named: Named,
    newtypes: [Newtype; 2],
    empty: Empty,
}

/// Compiles only if `T` implements `AnyBitPattern`
const fn assert_any_bit_pattern<T>()
where
    T: AnyBitPattern,
{
}

const _: () = {
    assert_any_bit_pattern::<()>();
    assert_any_bit_pattern::<u8>();
    assert_any_bit_pattern::<i128>();
    assert_any_bit_pattern::<[[u8; 32]; 4]>();
    assert_any_bit_pattern::<Unaligned<u64>>();
    assert_any_bit_pattern::<[Unaligned<[u64; 2]>; 3]>();
    assert_any_bit_pattern::<Named>();
    assert_any_bit_pattern::<Tuple>();
    assert_any_bit_pattern::<Empty>();
    assert_any_bit_pattern::<Newtype>();
    assert_any_bit_pattern::<Nested>();
};
