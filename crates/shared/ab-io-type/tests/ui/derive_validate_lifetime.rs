//! The `validate` function can't keep the reference to the value, which can point into the parsed
//! bytes, so it can't require a `'static` reference

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct NonMax {
    value: u32,
}

impl NonMax {
    const fn is_valid(&'static self) -> bool {
        self.value != u32::MAX
    }
}

fn main() {}
