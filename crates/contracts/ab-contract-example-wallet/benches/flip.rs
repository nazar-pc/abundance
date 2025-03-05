use crate::ffi::flip::FlipperFlipArgs;
use ab_contract_example_wallet::{ExampleWallet, ExampleWalletExt};
use ab_contracts_common::env::{Blake3Hash, MethodContext, Transaction, TransactionHeader};
use ab_contracts_common::{Address, Contract, ShardIndex};
use ab_contracts_executor::NativeExecutor;
use ab_contracts_io_type::trivial_type::TrivialType;
use ab_contracts_macros::contract;
use ab_contracts_standards::tx_handler::TxHandler;
use ab_system_contract_code::CodeExt;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::TransactionPayloadBuilder;
use ab_system_contract_simple_wallet_base::seal::hash_and_sign;
use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use schnorrkel::Keypair;

#[derive(Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Flipper {
    pub value: bool,
}

#[contract]
impl Flipper {
    #[init]
    pub fn new(#[input] &init_value: &bool) -> Self {
        Self { value: init_value }
    }

    #[update]
    pub fn flip(&mut self) {
        self.value = !self.value;
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let shard_index = ShardIndex::from_u32(1).unwrap();
    let executor = NativeExecutor::builder(shard_index)
        .with_contract::<ExampleWallet>()
        .with_contract_trait::<ExampleWallet, dyn TxHandler>()
        .with_contract::<Flipper>()
        .build()
        .unwrap();

    let storage = &mut executor.new_storage().unwrap();

    let keypair = Keypair::generate();

    let wallet_address = executor.transaction_emulate(Address::NULL, storage, |env| {
        // Deploy
        let wallet_address = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &ExampleWallet::code(),
            )
            .unwrap();

        // Initialize state
        env.example_wallet_initialize(
            MethodContext::Keep,
            wallet_address,
            &keypair.public.to_bytes(),
        )
        .unwrap();

        wallet_address
    });

    let flipper_address = executor.transaction_emulate(Address::NULL, storage, |env| {
        // Deploy
        let flipper_address = env
            .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Flipper::code())
            .unwrap();

        // Initialize state
        env.flipper_new(MethodContext::Keep, flipper_address, &true)
            .unwrap();

        flipper_address
    });

    let header = TransactionHeader {
        genesis_hash: Blake3Hash::default(),
        block_hash: Blake3Hash::default(),
        gas_limit: Default::default(),
        contract: wallet_address,
    };
    let payload = {
        let mut builder = TransactionPayloadBuilder::default();
        builder
            .with_method_call(
                &flipper_address,
                &FlipperFlipArgs::new(),
                TransactionMethodContext::Null,
                &[],
            )
            .unwrap();
        builder.into_aligned_bytes()
    };
    let mut nonce = 0;

    c.bench_function("verify", |b| {
        let seal = hash_and_sign(&keypair, &header, &payload, nonce);

        b.iter(|| {
            executor
                .transaction_verify(
                    black_box(Transaction {
                        header: &header,
                        payload: &payload,
                        seal: seal.as_bytes(),
                    }),
                    black_box(storage),
                )
                .unwrap();
        })
    });

    c.bench_function("execute", |b| {
        b.iter_batched(
            || {
                let seal = hash_and_sign(&keypair, &header, &payload, nonce);
                nonce += 1;
                seal
            },
            |seal| {
                executor
                    .transaction_execute(
                        black_box(Transaction {
                            header: &header,
                            payload: &payload,
                            seal: seal.as_bytes(),
                        }),
                        black_box(storage),
                    )
                    .unwrap();
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
