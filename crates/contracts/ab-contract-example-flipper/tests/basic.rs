use ab_contract_example_flipper::{Flipper, FlipperExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Address, Contract, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_system_contract_code::CodeExt;

#[test]
fn basic() {
    let shard_index = ShardIndex::from_u32(1).unwrap();
    let mut executor = NativeExecutor::in_memory_empty(shard_index)
        .with_contract::<Flipper>()
        .build()
        .unwrap();

    let mut env = executor.null_env();

    // Deploy
    let flipper_address = env
        .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Flipper::code())
        .unwrap();

    let init_value = true;

    // Initialize state
    env.flipper_new(MethodContext::Keep, flipper_address, &init_value)
        .unwrap();

    // Check initial value
    assert_eq!(env.flipper_value(flipper_address).unwrap(), init_value);

    // Flip
    env.flipper_flip(MethodContext::Keep, flipper_address)
        .unwrap();

    // Check new value
    assert_eq!(env.flipper_value(flipper_address).unwrap(), !init_value);
}
