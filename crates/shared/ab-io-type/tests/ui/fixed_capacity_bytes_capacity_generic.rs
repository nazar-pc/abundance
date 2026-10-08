//! Capacity of fixed capacity bytes and strings is checked for generic code in crates with
//! `generic_const_args` too, which doesn't have to propagate the bound, the check is evaluated for
//! every instance instead. Each case uses a different capacity since an error is only reported
//! once per capacity.

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_io_type::fixed_capacity_bytes::{FixedCapacityBytesU8, FixedCapacityBytesU16};
use ab_io_type::fixed_capacity_string::{FixedCapacityStringU8, FixedCapacityStringU16};

fn bytes_u8_try_from_bytes<const CAPACITY: usize>(bytes: &[u8]) {
    let _ = FixedCapacityBytesU8::<CAPACITY>::try_from_bytes(bytes);
}

fn bytes_u8_default<const CAPACITY: usize>() {
    let _ = FixedCapacityBytesU8::<CAPACITY>::default();
}

// Methods don't create instances, so the check is evaluated with the layout instead
fn bytes_u8_len<const CAPACITY: usize>() -> fn(&FixedCapacityBytesU8<CAPACITY>) -> u8 {
    FixedCapacityBytesU8::<CAPACITY>::len
}

fn string_u8_try_from_str<const CAPACITY: usize>(string: &str) {
    let _ = FixedCapacityStringU8::<CAPACITY>::try_from_str(string);
}

fn bytes_u16_try_from_bytes<const CAPACITY: usize>(bytes: &[u8]) {
    let _ = FixedCapacityBytesU16::<CAPACITY>::try_from_bytes(bytes);
}

fn bytes_u16_default<const CAPACITY: usize>() {
    let _ = FixedCapacityBytesU16::<CAPACITY>::default();
}

fn bytes_u16_len<const CAPACITY: usize>() -> fn(&FixedCapacityBytesU16<CAPACITY>) -> u16 {
    FixedCapacityBytesU16::<CAPACITY>::len
}

fn string_u16_try_from_str<const CAPACITY: usize>(string: &str) {
    let _ = FixedCapacityStringU16::<CAPACITY>::try_from_str(string);
}

fn main() {
    // Capacity above `u8::MAX`
    bytes_u8_try_from_bytes::<256>(&[1; 256]);
    bytes_u8_default::<257>();
    bytes_u8_len::<258>();
    string_u8_try_from_str::<259>("a");
    // Capacity above `u16::MAX`, but even
    bytes_u16_try_from_bytes::<65536>(&[1; 65536]);
    bytes_u16_default::<65538>();
    // Odd capacity
    bytes_u16_len::<1>();
    string_u16_try_from_str::<3>("a");
}
