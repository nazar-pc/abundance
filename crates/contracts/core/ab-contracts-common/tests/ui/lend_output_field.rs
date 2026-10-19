//! A field of an `#[output]` lent with `Env::lend_output()` stays borrowed until the method
//! returns, while other fields stay accessible

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_common::output::OutputArg;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(u8)]
enum Mode {
    A,
    B,
}

fn call<Output>(_output: &mut Output) -> Result<(), ContractError>
where
    Output: OutputArg<Mode>,
{
    Ok(())
}

struct Pair {
    mode: Mode,
    counter: u8,
}

fn output_field<'a>(env: &Env<'a>, output: &'a mut Pair) {
    let mut lent = env.lend_output(&mut output.mode);
    let _ = call(&mut lent);
    output.counter = 1;
    let _value = output.mode;
}

fn main() {
    let _ = (output_field, Mode::A, Mode::B);
}
