//! `TrivialType` derive must reject `#[repr(packed)]`, also in a separate `#[repr]` attribute.
//! Packed fields can be misaligned and the alignment wouldn't match the metadata, `Unaligned` is
//! the supported alternative.

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[repr(packed)]
struct Packed {
    a: u8,
    b: u32,
}

fn main() {}
