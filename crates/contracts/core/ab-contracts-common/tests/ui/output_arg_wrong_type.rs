//! Memory of another type can't be given to a call as an output, and the message doesn't blame the
//! contents of the expected type, which accept every bit pattern here

use ab_contracts_common::output::OutputArg;

fn give<Output>(_output: &mut Output)
where
    Output: OutputArg<u64>,
{
}

fn forward(output: &mut u64) {
    // An exclusive reference to the reference instead of the reference itself
    give(&mut &mut *output);
}

fn main() {
    let mut value = 0u32;
    give(&mut value);
    let _ = forward;
}
