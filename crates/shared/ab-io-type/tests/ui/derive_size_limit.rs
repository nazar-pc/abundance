//! Derived `TrivialType` must be smaller than 2^32 bytes, since `SIZE` and all `IoType` sizes are
//! represented as `u32`

use ab_io_type::trivial_type::TrivialType;

// Each field is supported on its own, but not together
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Huge {
    a: [u8; 1 << 31],
    b: [u8; 1 << 31],
}

fn main() {}
