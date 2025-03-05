use ab_contract_example_ft::{ExampleFt, ExampleFtExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Address, Balance, Contract, ContractError, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_contracts_standards::fungible::{Fungible, FungibleExt};
use ab_contracts_test_utils::dummy_wallet::DummyWallet;
use ab_system_contract_code::CodeExt;

#[test]
fn basic() {
    let shard_index = ShardIndex::from_u32(1).unwrap();
    let executor = NativeExecutor::builder(shard_index)
        .with_contract::<DummyWallet>()
        .with_contract::<ExampleFt>()
        .with_contract_trait::<ExampleFt, dyn Fungible>()
        .build()
        .unwrap();

    let storage = &mut executor.new_storage().unwrap();

    // Create two wallets
    let (alice, bob) = executor.transaction_emulate(Address::NULL, storage, |env| {
        let alice = env
            .code_deploy(
                MethodContext::Reset,
                Address::SYSTEM_CODE,
                &DummyWallet::code(),
            )
            .unwrap();
        let bob = env
            .code_deploy(
                MethodContext::Reset,
                Address::SYSTEM_CODE,
                &DummyWallet::code(),
            )
            .unwrap();

        (alice, bob)
    });

    // Deploy and initialize
    let token_address = executor.transaction_emulate(alice, storage, |env| {
        let token_address = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &ExampleFt::code(),
            )
            .unwrap();
        env.example_ft_new(MethodContext::Keep, token_address, &alice, &Balance::MAX)
            .unwrap();

        token_address
    });

    executor.transaction_emulate(alice, storage, |env| {
        let mut previous_alice_balance = Balance::MAX;
        let mut previous_bob_balance = Balance::from(0);
        let amount = Balance::from(10);

        // Direct
        assert_eq!(
            env.example_ft_balance(token_address, &alice).unwrap(),
            previous_alice_balance
        );
        // Through `Fungible` trait
        assert_eq!(
            env.fungible_balance(token_address, &alice).unwrap(),
            previous_alice_balance
        );

        // Direct
        env.example_ft_transfer(MethodContext::Keep, token_address, &alice, &bob, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.example_ft_balance(token_address, &alice).unwrap();
            let code_balance = env.example_ft_balance(token_address, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(token_address, &alice).unwrap();
            let code_balance = env.fungible_balance(token_address, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }

        previous_alice_balance -= amount;
        previous_bob_balance += amount;

        // Through `Fungible` trait
        env.fungible_transfer(MethodContext::Keep, token_address, &alice, &bob, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.example_ft_balance(token_address, &alice).unwrap();
            let code_balance = env.example_ft_balance(token_address, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(token_address, &alice).unwrap();
            let code_balance = env.fungible_balance(token_address, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }

        // Can't transfer from `bob` when transaction is authored by `alice`
        assert!(matches!(
            env.fungible_transfer(MethodContext::Keep, token_address, &bob, &alice, &amount),
            Err(ContractError::Forbidden)
        ));
    });
}
