//! Containers with contents that don't accept every bit pattern can't be given to a call as an
//! output directly, neither when a method forwards a container it received nor as a view that a
//! constructor returns

use ab_contracts_common::output::OutputArg;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_io_type::variable_elements::VariableElements;
use core::mem::MaybeUninit;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
enum Mode {
    A,
    B,
}

fn give_maybe_data<'a, Output>(_output: &mut Output)
where
    Output: OutputArg<MaybeData<'a, Mode>>,
{
}

fn give_elements<'a, Output>(_output: &mut Output)
where
    Output: OutputArg<VariableElements<'a, Mode>>,
{
}

fn forward(output: &mut MaybeData<'_, Mode>) {
    give_maybe_data(output);
}

fn main() {
    let mut uninit = [MaybeUninit::<Mode>::uninit(); 2];
    let mut size = 0;
    give_elements(&mut VariableElements::<Mode>::from_uninit(&mut uninit, &mut size).unwrap());
    let _ = (forward, Mode::A, Mode::B);
}
