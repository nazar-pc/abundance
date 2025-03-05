mod ffi_call;

use crate::context::ffi_call::FfiCall;
use crate::slots::Slots;
use ab_contracts_common::env::{EnvState, ExecutorContext, MethodContext, PreparedMethod};
use ab_contracts_common::method::{ExternalArgs, MethodFingerprint};
use ab_contracts_common::{Address, ContractError, ExitCode, ShardIndex};
use ab_system_contract_address_allocator::ffi::allocate_address::AddressAllocatorAllocateAddressArgs;
use halfbrown::HashMap;
use parking_lot::Mutex;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use tracing::{error, info_span};

#[derive(Debug, Copy, Clone)]
pub(super) struct MethodDetails {
    pub(super) recommended_state_capacity: u32,
    pub(super) recommended_slot_capacity: u32,
    pub(super) recommended_tmp_capacity: u32,
    pub(super) method_metadata: &'static [u8],
    pub(super) ffi_fn: unsafe extern "C" fn(NonNull<NonNull<c_void>>) -> ExitCode,
}

#[derive(Debug)]
pub(super) struct NativeExecutorContext {
    shard_index: ShardIndex,
    system_allocator_address: Address,
    /// Indexed by contract's code and method fingerprint
    methods_by_code: Arc<HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>>,
    slots: Arc<Mutex<Slots>>,
    allow_env_mutation: bool,
}

impl ExecutorContext for NativeExecutorContext {
    fn call_many(
        &self,
        previous_env_state: &EnvState,
        prepared_methods: &mut [PreparedMethod<'_>],
    ) -> Result<(), ContractError> {
        if prepared_methods.len() == 1 {
            self.call_single_method(previous_env_state, &mut prepared_methods[0])?;
        } else {
            self.call_many_methods(previous_env_state, prepared_methods)?;
        }

        if self.slots.lock().access_violation() {
            return Err(ContractError::Forbidden);
        }

        Ok(())
    }
}

impl NativeExecutorContext {
    pub(super) fn new(
        shard_index: ShardIndex,
        methods_by_code: Arc<HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>>,
        slots: Arc<Mutex<Slots>>,
        allow_env_mutation: bool,
    ) -> Self {
        Self {
            shard_index,
            system_allocator_address: Address::system_address_allocator(shard_index),
            methods_by_code,
            slots,
            allow_env_mutation,
        }
    }

    fn new_nested(&self, slots: Arc<Mutex<Slots>>, allow_env_mutation: bool) -> Self {
        Self {
            shard_index: self.shard_index,
            system_allocator_address: self.system_allocator_address,
            methods_by_code: Arc::clone(&self.methods_by_code),
            slots,
            allow_env_mutation,
        }
    }

    fn nested_slots(&self) -> Arc<Mutex<Slots>> {
        if self.allow_env_mutation {
            // Create nested slots instance to avoid persisting any access in slots owned by the
            // context
            self.slots.lock().new_nested()
        } else {
            // If mutation wasn't allowed on higher level, then reuse existing slots instance
            Arc::clone(&self.slots)
        }
    }

    fn prepare_ffi_call<'a>(
        &self,
        previous_env_state: &EnvState,
        prepared_method: &'a mut PreparedMethod<'_>,
        force_view_only: bool,
        nested_slots: Arc<Mutex<Slots>>,
    ) -> Result<FfiCall<'a>, ContractError> {
        let PreparedMethod {
            contract,
            fingerprint,
            external_args,
            method_context,
            ..
        } = prepared_method;

        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: Default::default(),
            own_address: *contract,
            context: match method_context {
                MethodContext::Keep => previous_env_state.context,
                MethodContext::Reset => Address::NULL,
                MethodContext::Replace => previous_env_state.own_address,
            },
            caller: previous_env_state.own_address,
        };

        let span = info_span!("NativeExecutorContext", %contract);
        let _span_guard = span.enter();

        let method_details = {
            let code = nested_slots.lock().get_code(*contract).ok_or_else(|| {
                error!("Contract or its code not found");
                ContractError::NotFound
            })?;
            *self
                .methods_by_code
                .get(&(code.as_slice(), fingerprint))
                .ok_or_else(|| {
                    let code = String::from_utf8_lossy(code.as_slice());
                    error!(
                        %code,
                        %fingerprint,
                        "Contract's code or fingerprint not found in methods map"
                    );
                    ContractError::NotImplemented
                })?
        };
        let is_allocate_new_address_method = contract == &self.system_allocator_address
            && fingerprint == &AddressAllocatorAllocateAddressArgs::FINGERPRINT;

        FfiCall::new(
            self,
            force_view_only,
            is_allocate_new_address_method,
            nested_slots,
            *contract,
            method_details,
            external_args,
            env_state,
        )
    }

    fn call_single_method(
        &self,
        previous_env_state: &EnvState,
        prepared_method: &mut PreparedMethod<'_>,
    ) -> Result<(), ContractError> {
        // TODO: Special read-only access that doesn't track any changes at all because there will
        //  be none
        let nested_slots = self.nested_slots();

        let result: Result<(), ContractError> = try {
            self.prepare_ffi_call(
                previous_env_state,
                prepared_method,
                // For call to multiple methods only read-only `#[view]` is allowed
                false,
                Arc::clone(&nested_slots),
            )?
            .dispatch()?
            .persist()?
        };

        if result.is_err() && self.allow_env_mutation {
            Arc::into_inner(nested_slots)
                .expect("All child references already dropped; qed")
                .into_inner()
                .reset();
        }

        result
    }

    fn call_many_methods(
        &self,
        previous_env_state: &EnvState,
        prepared_methods: &mut [PreparedMethod<'_>],
    ) -> Result<(), ContractError> {
        // TODO: Special read-only access that doesn't track any changes at all because there will
        //  be none
        let nested_slots = self.nested_slots();

        let ffi_calls = prepared_methods
            .iter_mut()
            .map(|prepared_method| {
                self.prepare_ffi_call(
                    previous_env_state,
                    prepared_method,
                    // For call to multiple methods only read-only `#[view]` is allowed
                    true,
                    Arc::clone(&nested_slots),
                )
            })
            .collect::<Result<Vec<FfiCall>, _>>()?;

        // TODO: Parallelism, but with panic unwinding it'll require to catch panics, which is
        //  really annoying
        // Collect all results regardless of success for deterministic behavior
        let results = ffi_calls
            .into_iter()
            .map(|ffi_call| {
                let result = ffi_call.dispatch()?;
                result.persist()
            })
            .collect::<Vec<_>>();

        for result in results {
            let () = result?;
        }

        Ok(())
    }
}
