//! `IoTypeMut` can't be implemented without `unsafe`, generic code relies on `io_type_mut()`
//! returning the same value on every call, which the implementation below doesn't do

use ab_io_type::IoTypeMut;

struct Alternating<'a> {
    first: &'a mut u64,
    second: &'a mut u64,
    use_second: bool,
}

impl IoTypeMut for Alternating<'_> {
    type Target = u64;

    fn io_type_mut(&mut self) -> &mut Self::Target {
        self.use_second = !self.use_second;
        if self.use_second {
            self.second
        } else {
            self.first
        }
    }
}

fn main() {}
