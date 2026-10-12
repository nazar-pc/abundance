//! Safe code must not be able to change where an `IoType` instance points, `get_initialized()` and
//! other methods would then read and write arbitrary memory.

use ab_io_type::IoType;
use ab_io_type::variable_elements::VariableElements;
use core::ptr::NonNull;

fn repoint_elements(elements: &mut VariableElements<u32>) {
    *elements.as_mut_ptr() = NonNull::dangling();
}

fn repoint_io_type<T>(value: &mut T)
where
    T: IoType,
{
    *IoType::as_mut_ptr(value) = NonNull::dangling();
}

fn main() {}
