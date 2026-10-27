//! The `validate` function takes a shared reference to the value and returns `bool`

use ab_io_type::trivial_type::TrivialType;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct ByValueReceiver {
    value: u32,
}

impl ByValueReceiver {
    const fn is_valid(self) -> bool {
        self.value != u32::MAX
    }
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = is_valid)]
struct ByValueArgument {
    value: u32,
}

const fn is_valid(value: ByValueArgument) -> bool {
    value.value != u32::MAX
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct MutableReference {
    value: u32,
}

impl MutableReference {
    const fn is_valid(&mut self) -> bool {
        self.value != u32::MAX
    }
}

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct NotBool {
    value: u32,
}

impl NotBool {
    const fn is_valid(&self) -> u8 {
        (self.value != u32::MAX) as u8
    }
}

fn main() {}
