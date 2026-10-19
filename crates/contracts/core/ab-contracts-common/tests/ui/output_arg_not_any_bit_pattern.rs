//! A plain value whose contents don't accept every bit pattern can't be given to a call as an
//! output directly, a failed call could leave an invalid value in it

use ab_contracts_common::output::OutputArg;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
enum Mode {
    A,
    B,
}

fn give<Output>(_output: &mut Output)
where
    Output: OutputArg<Mode>,
{
}

fn main() {
    let mut mode = Mode::A;
    give(&mut mode);
    let _ = Mode::B;
}
