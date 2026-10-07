//! `TrivialType` derive must reject `#[repr(packed)]`, even in a separate `#[repr]` attribute that
//! the derive doesn't parse. Packed fields can be misaligned and the alignment wouldn't match the
//! metadata, `Unaligned` is the supported alternative.

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[repr(packed)]
struct Packed {
    a: u8,
    b: u32,
}

fn main() {
    let _ = Packed { a: 1, b: 2 }.as_bytes();
}
