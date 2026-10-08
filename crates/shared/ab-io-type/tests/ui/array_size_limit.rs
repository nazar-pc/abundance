//! Arrays are only `TrivialType` when the number of elements (encoded in metadata) and size fit
//! into `u32`, which is checked for any usage, not only when metadata or size are accessed

use ab_io_type::trivial_type::TrivialType;

fn trivial_type<T: TrivialType>() {}

fn main() {
    // Too many (zero-sized) elements
    trivial_type::<[(); 1 << 32]>();
    // Too large size
    trivial_type::<[u16; 1 << 31]>();
}
