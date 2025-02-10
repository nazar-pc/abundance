mod ffi_call;

use crate::aligned_buffer::OwnedAlignedBuffer;
use crate::context::ffi_call::FfiCall;
use crate::slots::{Slots, UsedSlots};
use ab_contracts_common::env::{EnvState, ExecutorContext, MethodContext, PreparedMethod};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{Address, ContractError, ExitCode, ShardIndex};
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use tracing::{error, info_span};

/// Stores details about arguments that need to be processed after FFI call
enum DelayedProcessing {
    SlotReadOnly {
        size: u32,
    },
    SlotReadWrite {
        /// Pointer to `InternalArgs` where guest will store a pointer to potentially updated slot
        /// contents
        data_ptr: NonNull<*mut u8>,
        /// Pointer to slot's bytes buffer here bytes from `data_ptr` will need to be written
        /// after FFI function call
        slot_ptr: NonNull<OwnedAlignedBuffer>,
        /// Pointer to `InternalArgs` where guest will store potentially updated slot size,
        /// corresponds to `data_ptr`, filled during the second pass through the arguments
        /// (while reading `ExternalArgs`)
        size: u32,
        capacity: u32,
    },
}

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
    /// Indexed by contract's code (crate name is treated as "code")
    methods_by_code: Arc<HashMap<&'static [u8], HashMap<MethodFingerprint, MethodDetails>>>,
    // TODO: Think about optimizing locking
    slots: Slots,
    allow_env_mutation: bool,
}

impl ExecutorContext for NativeExecutorContext {
    fn call_many(
        &self,
        previous_env_state: &EnvState,
        prepared_methods: &mut [PreparedMethod<'_>],
    ) -> Result<(), ContractError> {
        // TODO: Check slot misuse across recursive calls
        // `used_slots` must be before processing of the method because in the process of method
        // handling, some data structures will store pointers to `UsedSlot`'s internals.
        let mut used_slots = UsedSlots::new(self.slots.clone());

        // TODO: Parallelism
        for prepared_method in prepared_methods {
            let PreparedMethod {
                contract,
                fingerprint,
                external_args,
                method_context,
                ..
            } = prepared_method;

            let env_state = EnvState {
                shard_index: self.shard_index,
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
                let code = self
                    .slots
                    .get(contract, &Address::SYSTEM_CODE)
                    .ok_or_else(|| {
                        error!("Contract or its code not found");
                        ContractError::NotFound
                    })?;
                *self
                    .methods_by_code
                    .get(code.as_slice())
                    .ok_or_else(|| {
                        let code = String::from_utf8_lossy(&code);
                        error!(%code, "Contract's code not found in methods map");
                        ContractError::InternalError
                    })?
                    .get(fingerprint)
                    .ok_or_else(|| {
                        let code = String::from_utf8_lossy(&code);
                        error!(%code, %fingerprint, "Method's fingerprint not found");
                        ContractError::NotImplemented
                    })?
            };

            let ffi_call = FfiCall::new(
                self,
                &mut used_slots,
                *contract,
                method_details,
                external_args,
                env_state,
            )?;

            let result = ffi_call.dispatch()?;
            result.persist()?;
        }

        used_slots.persist();

        Ok(())
    }
}

impl NativeExecutorContext {
    pub(super) fn new(
        shard_index: ShardIndex,
        methods_by_code: Arc<HashMap<&'static [u8], HashMap<MethodFingerprint, MethodDetails>>>,
        slots: Slots,
        allow_env_mutation: bool,
    ) -> Self {
        Self {
            shard_index,
            methods_by_code,
            slots,
            allow_env_mutation,
        }
    }

    fn new_nested(&self, allow_env_mutation: bool) -> Self {
        Self::new(
            self.shard_index,
            Arc::clone(&self.methods_by_code),
            self.slots.clone(),
            allow_env_mutation,
        )
    }
}
