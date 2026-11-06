//! Array limits are checked for generic code in crates with `gca_const_items` too, which doesn't
//! have to propagate the bound, the check is evaluated when the impl is used instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    gca_const_items,
    gca_macroless_args,
    gca_min_const_items,
    generic_const_items
)]

use ab_io_type::trivial_type::TrivialType;

fn metadata<T: TrivialType, const N: usize>() -> &'static [u8] {
    <[T; N] as TrivialType>::METADATA
}

fn main() {
    // Too many (zero-sized) elements, the size itself is zero
    metadata::<(), { 1 << 32 }>();
    // Too large size, the number of elements fits
    metadata::<u64, { 1 << 29 }>();
}
