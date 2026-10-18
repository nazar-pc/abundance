//! `Env::with_executor_context()` can only be called in `unsafe` code, since methods rely on the
//! lifetime of the `Env` they get

use ab_contracts_common::ContractError;
use ab_contracts_common::env::{Env, EnvState, ExecutorContext, PreparedMethod};

#[derive(Debug)]
struct FailingContext;

// SAFETY: Never returns `Ok(())` and doesn't access anything behind pointers in prepared methods
unsafe impl ExecutorContext for FailingContext {
    fn call(&self, _: &EnvState, _: &PreparedMethod<'_>) -> Result<(), ContractError> {
        Err(ContractError::Forbidden)
    }
}

fn create(state: EnvState, context: &FailingContext) -> Env<'_> {
    Env::with_executor_context(state, context)
}

fn main() {
    let _ = create;
}
