mod ffi_call;

use crate::context::ffi_call::make_ffi_call;
use ab_contracts_common::env::{EnvState, ExecutorContext, MethodContext, PreparedMethod};
use ab_contracts_common::method::{ExternalArgs, MethodFingerprint};
use ab_contracts_common::{Address, ContractError, ExitCode, ShardIndex};
use ab_contracts_slots::slots::NestedSlots;
use ab_system_contract_address_allocator::ffi::allocate_address::AddressAllocatorAllocateAddressArgs;
use halfbrown::HashMap;
use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::ptr::NonNull;
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
pub(super) struct NativeExecutorContext<'a> {
    shard_index: ShardIndex,
    system_allocator_address: Address,
    /// Indexed by contract's code and method fingerprint
    methods_by_code: &'a HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>,
    slots: UnsafeCell<NestedSlots<'a>>,
    tmp_owners: &'a UnsafeCell<Vec<Address>>,
    allow_env_mutation: bool,
}

impl<'a> ExecutorContext for NativeExecutorContext<'a> {
    fn call(
        &self,
        previous_env_state: &EnvState,
        prepared_method: &mut PreparedMethod<'_>,
    ) -> Result<(), ContractError> {
        // SAFETY: `NativeExecutorContext` is not `Sync`, slots instance was provided as `&mut` in
        // the constructor (meaning exclusive access) and this function is the only place where it
        // is accessed without recursive calls to itself
        let slots = unsafe { &mut *self.slots.get().cast::<NestedSlots<'a>>() };

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
            let code = slots.get_code(*contract).ok_or_else(|| {
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

        make_ffi_call(
            self.allow_env_mutation,
            is_allocate_new_address_method,
            slots,
            *contract,
            method_details,
            external_args,
            env_state,
            self.tmp_owners,
            |slots, allow_env_mutation| self.new_nested(slots, allow_env_mutation),
        )
    }
}

impl<'a> NativeExecutorContext<'a> {
    #[inline(always)]
    pub(super) fn new(
        shard_index: ShardIndex,
        methods_by_code: &'a HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>,
        slots: NestedSlots<'a>,
        tmp_owners: &'a UnsafeCell<Vec<Address>>,
        allow_env_mutation: bool,
    ) -> Self {
        Self {
            shard_index,
            system_allocator_address: Address::system_address_allocator(shard_index),
            methods_by_code,
            slots: UnsafeCell::new(slots),
            tmp_owners,
            allow_env_mutation,
        }
    }

    #[inline(always)]
    fn new_nested(
        &self,
        slots: NestedSlots<'a>,
        allow_env_mutation: bool,
    ) -> NativeExecutorContext<'a> {
        Self {
            shard_index: self.shard_index,
            system_allocator_address: self.system_allocator_address,
            methods_by_code: self.methods_by_code,
            slots: UnsafeCell::new(slots),
            tmp_owners: self.tmp_owners,
            allow_env_mutation,
        }
    }
}
