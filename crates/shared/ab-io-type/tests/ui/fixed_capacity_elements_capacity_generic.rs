//! Capacity of fixed capacity elements, bytes and strings is checked for generic code in crates
//! with `generic_const_args` too, which doesn't have to propagate the bound, the check is evaluated
//! for every instance instead. Each case uses a different capacity since an error is only reported
//! once per capacity.

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]

use ab_io_type::fixed_capacity_bytes::FixedCapacityBytes;
use ab_io_type::fixed_capacity_elements::FixedCapacityElements;
use ab_io_type::fixed_capacity_string::FixedCapacityString;
use ab_io_type::trivial_type::TrivialType;

fn elements_try_from_slice<Element, const CAPACITY: usize>(elements: &[Element])
where
    Element: TrivialType + Default,
{
    let _ = FixedCapacityElements::<Element, CAPACITY>::try_from_slice(elements);
}

fn bytes_default<const CAPACITY: usize>() {
    let _ = FixedCapacityBytes::<CAPACITY>::default();
}

// Methods don't create instances, so the check is evaluated with the layout instead
fn elements_len<Element, const CAPACITY: usize>()
-> fn(&FixedCapacityElements<Element, CAPACITY>) -> u32
where
    Element: TrivialType,
{
    FixedCapacityElements::<Element, CAPACITY>::len
}

fn string_try_from_str<const CAPACITY: usize>(string: &str) {
    let _ = FixedCapacityString::<CAPACITY>::try_from_str(string);
}

fn metadata<Element, const CAPACITY: usize>() -> &'static [u8]
where
    Element: TrivialType,
{
    <FixedCapacityElements<Element, CAPACITY> as TrivialType>::METADATA
}

fn main() {
    // Capacity above `u32::MAX`, the size of zero-sized elements fits
    elements_try_from_slice::<(), { 1 << 32 }>(&[]);
    // Capacity fits, but the size doesn't because of the length
    bytes_default::<{ u32::MAX as usize - 3 }>();
    string_try_from_str::<{ u32::MAX as usize - 2 }>("a");
    // Capacity fits, but the size doesn't because of the length extended to the alignment of
    // elements
    elements_len::<u128, { (1 << 28) - 1 }>();
    // Capacity fits, but the size of elements doesn't
    metadata::<u16, { 1 << 31 }>();
}
