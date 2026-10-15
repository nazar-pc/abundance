//! The `validate` function is called outside of the `unsafe` block of generated code, so it can't
//! be an `unsafe` function

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = is_valid)]
struct NonMax {
    value: u32,
}

/// # Safety
/// Never called
const unsafe fn is_valid(value: NonMax) -> bool {
    value.value != u32::MAX
}

fn main() {}
