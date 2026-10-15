//! `ExecutorContext` can only be implemented with `unsafe impl`, since callers rely on what
//! implementations do with pointers in prepared methods

use ab_contracts_common::ContractError;
use ab_contracts_common::env::{EnvState, ExecutorContext, PreparedMethod};

#[derive(Debug)]
struct NoopContext;

impl ExecutorContext for NoopContext {
    fn call(&self, _: &EnvState, _: &PreparedMethod<'_>) -> Result<(), ContractError> {
        Ok(())
    }
}

fn main() {}
