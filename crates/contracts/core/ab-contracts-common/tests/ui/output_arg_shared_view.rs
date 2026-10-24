//! Containers over shared memory can't be given to a call as an output, even if their contents
//! accept every bit pattern, the call would write into that memory otherwise

use ab_contracts_common::output::OutputArg;
use ab_io_type::maybe_data::MaybeData;

fn give<'a, Output>(_output: &mut Output)
where
    Output: OutputArg<MaybeData<'a, u64>>,
{
}

fn main() {
    let value = 5u64;
    give(&mut MaybeData::from_ref(Some(&value)));
}
