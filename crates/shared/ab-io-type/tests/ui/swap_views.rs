//! Containers borrow the memory they view, so after two of them are swapped, neither can be used
//! once the memory of the other is gone. Otherwise, the remaining one would read freed memory.

use ab_io_type::maybe_data::MaybeData;
use ab_io_type::variable_elements::VariableElements;
use core::mem;

fn swap_with_local(elements: &mut VariableElements<'_, u8>) {
    let mut buffer = [2u8; 4];
    let mut size = 4;
    let mut local = VariableElements::<u8>::from_buffer_mut(&mut buffer, &mut size).unwrap();
    mem::swap(elements, &mut local);
}

fn main() {
    let mut buffer_a = [1u8; 4];
    let mut size_a = 4;
    let mut a = VariableElements::<u8>::from_buffer_mut(&mut buffer_a, &mut size_a).unwrap();
    {
        let mut buffer_b = vec![2u8; 4];
        let mut size_b = 4;
        let mut b = VariableElements::<u8>::from_buffer_mut(&mut buffer_b, &mut size_b).unwrap();
        mem::swap(&mut a, &mut b);
    }
    let _ = a.get_initialized();

    let mut data_a = 1u64;
    let mut size_a = 8;
    let mut a = MaybeData::from_mut(&mut data_a, &mut size_a).unwrap();
    {
        let mut data_b = 2u64;
        let mut size_b = 8;
        let b = MaybeData::from_mut(&mut data_b, &mut size_b).unwrap();
        let _ = mem::replace(&mut a, b);
    }
    let _ = a.get();
}
