use ab_contracts_common::env::MethodContext;
use ab_contracts_common::{Address, Contract, ShardIndex};
use ab_example_contract_flipper::{Flipper, FlipperExt};
use ab_executor_native::NativeExecutor;
use ab_io_type::bool::Bool;
use ab_system_contract_code::CodeExt;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn criterion_benchmark(c: &mut Criterion) {
    let shard_index = ShardIndex::from_u32(1).unwrap();
    let executor = NativeExecutor::builder(shard_index)
        .with_contract::<Flipper>()
        .build()
        .unwrap();

    let slots = &mut executor.new_storage_slots().unwrap();

    let flipper_address = executor.transaction_emulate(Address::NULL, slots, |env| {
        // Deploy
        let flipper_address = env
            .code_deploy(MethodContext::Keep, Address::SYSTEM_CODE, &Flipper::code())
            .unwrap();

        // Initialize state
        env.flipper_new(MethodContext::Keep, flipper_address, &Bool::new(true))
            .unwrap();

        flipper_address
    });

    let mut group = c.benchmark_group("flipper");
    group.throughput(Throughput::Elements(1));

    group.bench_function("direct", |b| {
        executor.transaction_emulate(Address::NULL, slots, |env| {
            b.iter(|| {
                env.flipper_flip(MethodContext::Keep, flipper_address)
                    .unwrap();
            });
        });
    });

    group.bench_function("transaction", |b| {
        b.iter(|| {
            executor.transaction_emulate(Address::NULL, slots, |env| {
                env.flipper_flip(MethodContext::Keep, flipper_address)
                    .unwrap();
            });
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
