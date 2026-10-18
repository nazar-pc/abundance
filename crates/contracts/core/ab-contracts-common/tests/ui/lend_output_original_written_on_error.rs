//! The original of memory lent with `Env::lend_output()` can't be written to after a failed
//! call, since it stays borrowed until the method returns

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

fn write_on_error<'a>(env: &Env<'a>, mode: &'a mut Mode) {
    let mut lent = env.lend_output(mode);
    if call(&mut lent).is_err() {
        *mode = Mode::A;
    }
}

fn main() {
    let _ = (write_on_error, Mode::B);
}
