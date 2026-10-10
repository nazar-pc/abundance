use ab_contracts_common::Contract;
use ab_contracts_common::env::MethodContext;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;

#[derive(Debug, Copy, Clone, PartialEq, Eq, TrivialType)]
#[repr(C)]
pub struct InitReturnAndOutput {
    pub value: u64,
}

#[contract]
impl InitReturnAndOutput {
    /// The return value is the state, while the output of the same type goes to the caller
    #[init]
    pub fn init(#[output] other: &mut MaybeData<Self>) -> Self {
        other.replace(Self { value: 2 });
        Self { value: 1 }
    }

    #[view]
    pub fn value(&self) -> u64 {
        self.value
    }
}

#[test]
fn init_return_value_is_state() {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<InitReturnAndOutput>()
        .build()
        .unwrap();
    let slots = &mut executor.new_storage_slots().unwrap();

    let (contract, other, other_size) = executor.transaction_emulate(Address::NULL, slots, |env| {
        let contract = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &InitReturnAndOutput::code(),
            )
            .unwrap();
        let mut other = InitReturnAndOutput { value: 0 };
        let mut other_size = 0;
        env.init_return_and_output_init(
            MethodContext::Keep,
            contract,
            &mut MaybeData::from_mut(&mut other, &mut other_size),
        )
        .unwrap();
        (contract, other, other_size)
    });

    assert_eq!(other_size, InitReturnAndOutput::SIZE);
    assert_eq!(other, InitReturnAndOutput { value: 2 });
    let value = executor.with_env_ro(slots, |env| {
        env.init_return_and_output_value(contract).unwrap()
    });
    assert_eq!(value, 1);
}
