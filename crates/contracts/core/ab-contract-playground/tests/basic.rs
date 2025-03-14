use ab_contract_playground::{LastAction, Playground, PlaygroundExt};
use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Address, Balance, Contract, ContractError, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_contracts_standards::fungible::{Fungible, FungibleExt};
use ab_contracts_test_utils::dummy_wallet::DummyWallet;
use ab_system_contract_code::CodeExt;

#[test]
fn basic() {
    tracing_subscriber::fmt::init();

    let shard_index = ShardIndex::from_u32(1).unwrap();
    let executor = NativeExecutor::builder(shard_index)
        .with_contract::<DummyWallet>()
        .with_contract::<Playground>()
        .with_contract_trait::<Playground, dyn Fungible>()
        .build()
        .unwrap();

    let slots = &mut executor.new_storage_slots().unwrap();

    // Create two wallets
    let (alice, bob) = executor.transaction_emulate(Address::NULL, slots, |env| {
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
    let playground_token = executor.transaction_emulate(alice, slots, |env| {
        let playground_address = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &Playground::code(),
            )
            .unwrap();
        env.playground_new_result(
            MethodContext::Keep,
            playground_address,
            &alice,
            &Balance::MAX,
        )
        .unwrap();

        playground_address
    });

    executor.transaction_emulate(Address::NULL, slots, |env| {
        // There must be no action initially
        assert_eq!(
            env.playground_last_action(MethodContext::Reset, playground_token)
                .unwrap(),
            LastAction::None
        );
    });

    executor.transaction_emulate(alice, slots, |env| {
        let mut previous_alice_balance = Balance::MAX;
        let mut previous_bob_balance = Balance::from(0);
        let amount = Balance::from(10);

        // Direct
        assert_eq!(
            env.playground_balance(playground_token, &alice).unwrap(),
            previous_alice_balance
        );
        // Through `Fungible` trait
        assert_eq!(
            env.fungible_balance(playground_token, &alice).unwrap(),
            previous_alice_balance
        );

        // Direct
        env.playground_transfer(MethodContext::Keep, playground_token, &alice, &bob, &amount)
            .unwrap();
        assert_eq!(
            env.playground_last_action(MethodContext::Reset, playground_token)
                .unwrap(),
            LastAction::Transfer
        );

        // Direct
        {
            let remaining_balance = env.playground_balance(playground_token, &alice).unwrap();
            let code_balance = env.playground_balance(playground_token, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(playground_token, &alice).unwrap();
            let code_balance = env.fungible_balance(playground_token, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }

        previous_alice_balance -= amount;
        previous_bob_balance += amount;

        // Through `Fungible` trait
        env.fungible_transfer(MethodContext::Keep, playground_token, &alice, &bob, &amount)
            .unwrap();

        // Direct
        {
            let remaining_balance = env.playground_balance(playground_token, &alice).unwrap();
            let code_balance = env.playground_balance(playground_token, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }
        // Through `Fungible` trait
        {
            let remaining_balance = env.fungible_balance(playground_token, &alice).unwrap();
            let code_balance = env.fungible_balance(playground_token, &bob).unwrap();

            assert_eq!(remaining_balance, previous_alice_balance - amount);
            assert_eq!(code_balance, previous_bob_balance + amount);
        }

        // Can't transfer from `bob` when transaction is authored by `alice`
        assert!(matches!(
            env.fungible_transfer(MethodContext::Keep, playground_token, &bob, &alice, &amount),
            Err(ContractError::Forbidden)
        ));
    });

    executor.transaction_emulate(Address::NULL, slots, |env| {
        // There must be no action after all transactions either
        assert_eq!(
            env.playground_last_action(MethodContext::Reset, playground_token)
                .unwrap(),
            LastAction::None
        );
    });
}
