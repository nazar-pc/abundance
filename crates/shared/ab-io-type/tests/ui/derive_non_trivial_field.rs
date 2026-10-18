//! Every field of a derived `TrivialType` must itself be `TrivialType`. `usize` is rejected because
//! its size depends on the platform.

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
struct WithUsize {
    value: usize,
}

fn main() {
    let _ = WithUsize { value: 1 };
}
