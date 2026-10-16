//! The `validate` function takes the value by value

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct NonMax {
    value: u32,
}

impl NonMax {
    const fn is_valid(&self) -> bool {
        self.value != u32::MAX
    }
}

fn main() {}
