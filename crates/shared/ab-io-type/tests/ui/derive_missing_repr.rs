//! `TrivialType` derive must reject types without `#[repr(..)]`, their layout is unspecified

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
struct NoReprStruct {
    value: u8,
}

#[derive(Copy, Clone, TrivialType)]
enum NoReprEnum {
    A,
    B,
}

fn main() {}
