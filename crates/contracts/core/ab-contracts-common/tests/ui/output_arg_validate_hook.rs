//! A type with a validate hook can't be given to a call as an output directly, even if all of its
//! fields accept every bit pattern, a failed call could leave a value behind that the hook rejects

use ab_contracts_common::output::OutputArg;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
#[trivial_type(validate = Self::is_valid)]
struct Small {
    value: u8,
}

impl Small {
    const fn is_valid(value: Self) -> bool {
        value.value < 10
    }
}

fn give<Output>(_output: &mut Output)
where
    Output: OutputArg<Small>,
{
}

fn main() {
    give(&mut Small { value: 1 });
}
