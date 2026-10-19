//! The original of memory lent with `Env::lend_output()` stays borrowed until the method
//! returns, so it can't be read after a call that may have left an invalid value in it

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

fn read_after_lend<'a>(env: &Env<'a>, mode: &'a mut Mode) {
    let mut lent = env.lend_output(mode);
    let _ = call(&mut lent);
    let _value = *mode;
}

fn main() {
    let _ = (read_after_lend, Mode::A, Mode::B);
}
