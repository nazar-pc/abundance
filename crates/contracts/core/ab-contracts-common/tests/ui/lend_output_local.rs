//! Local memory can't be lent with `Env::lend_output()`, it doesn't live until the method returns

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

fn lend_local(env: &Env<'_>) {
    let mut local = Mode::A;
    let mut lent = env.lend_output(&mut local);
    let _ = call(&mut lent);
}

fn main() {
    let _ = (lend_local, Mode::B);
}
