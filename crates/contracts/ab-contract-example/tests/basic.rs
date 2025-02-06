use ab_contract_example::{Example, ExampleExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Address, Balance, Contract, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_standards::FungibleExt;
use ab_system_contract_code::CodeExt;

#[test]
fn basic() {
    tracing_subscriber::fmt::init();

    let shard_index = ShardIndex::from_u32(1).unwrap();
    let mut executor = NativeExecutor::in_memory(shard_index).unwrap();
    executor.deploy_typical_system_contracts().unwrap();
    let example_token = {
        let env = &mut *executor.null_env();

        let example_address = env
            .code_deploy(
                &MethodContext::Keep,
                &Address::SYSTEM_CODE,
                &VariableBytes::from_buffer(
                    Example::CRATE_NAME.as_bytes(),
                    &(Example::CRATE_NAME.len() as u32),
                ),
            )
            .unwrap();
        env.example_new(
            &MethodContext::Keep,
            &example_address,
            &example_address,
            &Balance::MAX,
        )
        .unwrap();

        example_address
    };

    {
        let env = &mut *executor.env(example_token, Address::NULL);

        let from = example_token;
        let mut previous_from_balance = Balance::MAX;
        let to = Address::SYSTEM_CODE;
        let mut previous_to_balance = Balance::from(0);
        let amount = Balance::from(10);

        // Direct
        assert_eq!(
            env.example_balance(&example_token, &from).unwrap(),
            previous_from_balance
        );
        // Through `Fungible` trait
        assert_eq!(
            env.fungible_balance(&example_token, &from).unwrap(),
            previous_from_balance
        );

        // Direct
        env.example_transfer(&MethodContext::Keep, &example_token, &from, &to, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.example_balance(&example_token, &from).unwrap();
            let code_balance = env.example_balance(&example_token, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(&example_token, &from).unwrap();
            let code_balance = env.fungible_balance(&example_token, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }

        previous_from_balance -= amount;
        previous_to_balance += amount;

        // Through `Fungible` trait
        env.fungible_transfer(&MethodContext::Keep, &example_token, &from, &to, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.example_balance(&example_token, &from).unwrap();
            let code_balance = env.example_balance(&example_token, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(&example_token, &from).unwrap();
            let code_balance = env.fungible_balance(&example_token, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }
    }
}
