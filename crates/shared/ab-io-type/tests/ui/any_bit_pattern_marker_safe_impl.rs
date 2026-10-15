//! `AnyBitPatternMarker` can't be implemented without `unsafe`, for the enum below it would allow
//! creating invalid discriminants from bytes

use ab_io_type::trivial_type::{AnyBitPatternMarker, TrivialType};

#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum Ctx {
    A,
    B,
}

impl AnyBitPatternMarker for Ctx {}

fn main() {}
