//! Array limits are checked for generic code in crates with `generic_const_args` too, which doesn't
//! have to propagate the bound, the check is evaluated when the impl is used instead

#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
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
