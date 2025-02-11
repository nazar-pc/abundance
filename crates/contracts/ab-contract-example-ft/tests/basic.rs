use ab_contract_example_ft::{ExampleFt, ExampleFtExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Address, Balance, Contract, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_standards::FungibleExt;
use ab_system_contract_code::CodeExt;

#[test]
fn basic() {
    let shard_index = ShardIndex::from_u32(1).unwrap();
    let mut executor = NativeExecutor::in_memory(shard_index).unwrap();
    executor.deploy_typical_system_contracts().unwrap();
    let token_address = {
        let env = &mut *executor.null_env();

        let token_address = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &VariableBytes::from_buffer(
                    ExampleFt::CRATE_NAME.as_bytes(),
                    &(ExampleFt::CRATE_NAME.len() as u32),
                ),
            )
            .unwrap();
        env.example_ft_new(
            MethodContext::Keep,
            token_address,
            &token_address,
            &Balance::MAX,
        )
        .unwrap();

        token_address
    };

    {
        let env = &mut *executor.env(token_address, Address::NULL);

        let from = token_address;
        let mut previous_from_balance = Balance::MAX;
        let to = Address::SYSTEM_CODE;
        let mut previous_to_balance = Balance::from(0);
        let amount = Balance::from(10);

        // Direct
        assert_eq!(
            env.example_ft_balance(token_address, &from).unwrap(),
            previous_from_balance
        );
        // Through `Fungible` trait
        assert_eq!(
            env.fungible_balance(token_address, &from).unwrap(),
            previous_from_balance
        );

        // Direct
        env.example_ft_transfer(MethodContext::Keep, token_address, &from, &to, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.example_ft_balance(token_address, &from).unwrap();
            let code_balance = env.example_ft_balance(token_address, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(token_address, &from).unwrap();
            let code_balance = env.fungible_balance(token_address, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }

        previous_from_balance -= amount;
        previous_to_balance += amount;

        // Through `Fungible` trait
        env.fungible_transfer(MethodContext::Keep, token_address, &from, &to, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.example_ft_balance(token_address, &from).unwrap();
            let code_balance = env.example_ft_balance(token_address, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(token_address, &from).unwrap();
            let code_balance = env.fungible_balance(token_address, &to).unwrap();

            assert_eq!(remaining_balance, previous_from_balance - amount);
            assert_eq!(code_balance, previous_to_balance + amount);
        }
    }
}
