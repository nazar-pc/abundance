//! A container that `Lent::get_mut()` returns is a new view of the lent memory, which can't be kept
//! while the `Lent` is given to a call, so it can't read contents that the call left behind without
//! parsing them

use ab_contracts_common::output::{Lent, OutputArg};
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_elements::VariableElements;
use core::mem::MaybeUninit;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
enum Mode {
    A,
    B,
}

fn main() {
    let mut uninit = [MaybeUninit::<Mode>::uninit(); 1];
    let mut uninit_size = 0;
    let mut lent =
        Lent::<VariableElements<'_, Mode>>::from_uninit(&mut uninit, &mut uninit_size).unwrap();
    let view = lent.get_mut().unwrap();
    // A call writes an invalid value and reports its size
    let (ptr, _, _) = lent.as_raw_output();
    // SAFETY: The pointer is valid for writes of the capacity, which is one byte
    unsafe {
        ptr.cast::<u8>().write(7);
    }
    // SAFETY: The size of one element, with that many bytes initialized above
    unsafe {
        lent.set_output_size(1);
    }
    let _ = view.get_initialized();
    let _ = Mode::B;
}
