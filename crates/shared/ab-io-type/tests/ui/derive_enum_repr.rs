//! `TrivialType` derive must reject enums that are not exactly `#[repr(u8)]`, metadata encodes
//! the discriminant as a single byte

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(u16)]
enum Wide {
    A,
    B,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
enum ReprC {
    A,
    B,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(i8)]
enum Signed {
    A,
    B,
}

#[derive(Copy, Clone, TrivialType)]
#[repr(u8, align(2))]
enum Aligned {
    A,
    B,
}

fn main() {}
