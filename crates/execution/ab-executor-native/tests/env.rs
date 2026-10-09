use ab_contracts_common::env::{Env, EnvState, ExecutorContext, MethodContext, PreparedMethod};
use ab_contracts_common::{Contract, ContractError};
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_executor_native::NativeExecutor;
use ab_io_type::trivial_type::TrivialType;
use ab_system_contract_code::CodeExt;

/// Executor context that the executor knows nothing about
#[derive(Debug)]
struct ForeignContext;

impl ExecutorContext for ForeignContext {
    fn call(&self, _: &EnvState, _: &PreparedMethod<'_>) -> Result<(), ContractError> {
        Err(ContractError::Forbidden)
    }
}

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct EnvSwapper {
    pub value: u8,
}

#[contract]
impl EnvSwapper {
    #[init]
    pub fn init() -> Self {
        Self { value: 0 }
    }

    /// Replaces the environment provided by the executor with one that has a different executor
    /// context
    #[update]
    pub fn swap_env(&mut self, #[env] env: &mut Env<'_>) {
        let env_state = EnvState {
            shard_index: env.shard_index(),
            padding_0: [0; _],
            own_address: env.own_address(),
            context: env.context(),
            caller: env.caller(),
        };
        // Zero-sized, so it doesn't actually leak memory
        let foreign_context = Box::leak(Box::new(ForeignContext));
        let mut foreign_env = Env::with_executor_context(env_state, foreign_context);
        core::mem::swap(env, &mut foreign_env);

        self.value = 1;
    }

    #[view]
    pub fn value(&self) -> u8 {
        self.value
    }
}

#[test]
fn contract_replaces_env() {
    let executor = NativeExecutor::builder(ShardIndex::new(1).unwrap())
        .with_contract::<EnvSwapper>()
        .build()
        .unwrap();
    let slots = &mut executor.new_storage_slots().unwrap();

    let env_swapper = executor.transaction_emulate(Address::NULL, slots, |env| {
        let env_swapper = env
            .code_deploy(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &EnvSwapper::code(),
            )
            .unwrap();
        env.env_swapper_init(MethodContext::Keep, env_swapper)
            .unwrap();
        env_swapper
    });

    // The executor must keep using its own context after the call, not the one from the replaced
    // environment, so the call succeeds and its state changes are persisted
    executor.transaction_emulate(Address::NULL, slots, |env| {
        env.env_swapper_swap_env(MethodContext::Keep, env_swapper)
            .unwrap();
    });
    let value = executor.with_env_ro(slots, |env| env.env_swapper_value(env_swapper).unwrap());
    assert_eq!(value, 1);
}
