//! `TrivialType` derive must reject types with implicit padding. Padding bytes are uninitialized,
//! and the safe `as_bytes()` would expose them.

use ab_io_type::trivial_type::TrivialType;

// Padding between fields of a struct
#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct PaddedStruct {
    a: u8,
    b: u32,
}

// Padding in an enum variant that is smaller than the largest variant
#[derive(Copy, Clone, TrivialType)]
#[repr(u8)]
enum PaddedEnum {
    A { a: u8, b: u8 },
    B { c: u16 },
}

fn main() {
    let _ = PaddedStruct { a: 1, b: 2 }.as_bytes();
    let _ = PaddedEnum::A { a: 1, b: 2 }.as_bytes();
}
