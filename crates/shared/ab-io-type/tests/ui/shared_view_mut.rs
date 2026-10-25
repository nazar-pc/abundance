//! Containers over shared memory are only accessible through a shared reference, methods that take
//! `&mut self` would write into that memory otherwise

use ab_io_type::maybe_data::MaybeData;
use ab_io_type::variable_elements::VariableElements;

fn main() {
    let buffer = [1u8; 4];
    let size = 4;
    let _ = VariableElements::<u8>::from_buffer(&buffer, &size)
        .unwrap()
        .truncate(0);

    let data = 1u64;
    MaybeData::from_ref(Some(&data)).remove();
}
