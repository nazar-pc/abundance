//! Lending a reborrow in an inner block doesn't make the original accessible after the block, it
//! stays borrowed until the method returns

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

fn reborrow_in_block<'a>(env: &Env<'a>, mode: &'a mut Mode) {
    {
        let mut lent = env.lend_output(&mut *mode);
        let _ = call(&mut lent);
    }
    let _value = *mode;
}

fn main() {
    let _ = (reborrow_in_block, Mode::A, Mode::B);
}
