//! `AnyBitPattern` is implemented for every `TrivialType` that implements `AnyBitPatternMarker`,
//! the marker is the only trait that implementations write

use ab_io_type::trivial_type::{AnyBitPattern, TrivialType};

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct Header {
    a: u8,
    b: [u8; 3],
}

impl AnyBitPattern for Header {}

fn main() {}
