//! `VariableElements` and `MaybeData` don't support zero-sized types. The number of elements is
//! computed by dividing by the element size, and missing data is represented by zero size.

use ab_io_type::maybe_data::MaybeData;
use ab_io_type::variable_elements::VariableElements;

fn main() {
    // Zero-sized element
    let buffer = [(), ()];
    let size = 0u32;
    let _ = VariableElements::<()>::from_buffer(&buffer, &size);
    // Zero-sized data
    let _ = MaybeData::<[u8; 0]>::from_ref(None);
}
