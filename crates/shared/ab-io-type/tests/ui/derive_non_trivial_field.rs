//! Every field of a derived `TrivialType` must itself be `TrivialType`. `bool` is rejected because
//! it has invalid bit patterns, `Bool` exists for this purpose.

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithBool {
    flag: bool,
}

fn main() {
    let _ = WithBool { flag: true };
}
