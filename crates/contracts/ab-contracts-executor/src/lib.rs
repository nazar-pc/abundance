#![feature(non_null_from_ref, pointer_is_aligned_to, try_blocks)]

mod aligned_buffer;
mod context;
mod slots;

use crate::aligned_buffer::SharedAlignedBuffer;
use crate::context::{MethodDetails, NativeExecutorContext};
use crate::slots::{HashMap, Slots};
use ab_contracts_common::env::{Env, EnvState, MethodContext};
use ab_contracts_common::metadata::decode::{MetadataDecoder, MetadataDecodingError, MetadataItem};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{
    Address, Contract, ContractError, ContractsMethodsFnPointer, ShardIndex,
};
use ab_system_contract_address_allocator::{AddressAllocator, AddressAllocatorExt};
use ab_system_contract_code::{Code, CodeExt};
use ab_system_contract_state::State;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::error;

/// Native executor errors
#[derive(Debug, thiserror::Error)]
pub enum NativeExecutorError {
    /// Contract metadata not found
    #[error("Contract metadata not found")]
    ContractMetadataNotFound,
    /// Contract metadata decoding error
    #[error("Contract metadata decoding error: {error}")]
    ContractMetadataDecodingError {
        error: MetadataDecodingError<'static>,
    },
    /// Expected contract metadata, found trait
    #[error("Expected contract metadata, found trait")]
    ExpectedContractMetadataFoundTrait,
    /// Duplicate method in contract
    #[error("Duplicate method fingerprint {method_fingerprint} for contract code {contact_code}")]
    DuplicateMethodInContract {
        /// Name of the crate in which method was duplicated
        contact_code: &'static str,
        /// Method fingerprint
        method_fingerprint: &'static MethodFingerprint,
    },
    /// Failed to deploy system contracts
    #[error("Failed to deploy system contracts: {error}")]
    FailedToDeploySystemContracts { error: ContractError },
}

// TODO: `NativeExecutorBuilder` that allows to inject contracts and trait implementations
//  explicitly instead of relying on `inventory` that silently doesn't work under Miri
// TODO: Some kind of transaction notion with `#[tmp]` wiped at the end of it
pub struct NativeExecutor {
    shard_index: ShardIndex,
    /// Indexed by contract's code and method fingerprint
    methods_by_code: Arc<HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>>,
    slots: Arc<Mutex<Slots>>,
}

impl NativeExecutor {
    /// Instantiate in-memory native executor with empty storage
    pub fn in_memory_empty(shard_index: ShardIndex) -> Result<Self, NativeExecutorError> {
        let mut methods_by_code = HashMap::with_capacity(
            inventory::iter::<ContractsMethodsFnPointer>
                .into_iter()
                .count(),
        );
        for &contract_methods_fn_pointer in inventory::iter::<ContractsMethodsFnPointer> {
            let ContractsMethodsFnPointer {
                contact_code,
                main_contract_metadata,
                method_fingerprint,
                method_metadata,
                ffi_fn,
            } = contract_methods_fn_pointer;
            let recommended_capacities = match MetadataDecoder::new(main_contract_metadata)
                .decode_next()
                .ok_or(NativeExecutorError::ContractMetadataNotFound)?
                .map_err(|error| NativeExecutorError::ContractMetadataDecodingError { error })?
            {
                MetadataItem::Contract {
                    recommended_state_capacity,
                    recommended_slot_capacity,
                    recommended_tmp_capacity,
                    ..
                } => (
                    recommended_state_capacity,
                    recommended_slot_capacity,
                    recommended_tmp_capacity,
                ),
                MetadataItem::Trait { .. } => {
                    return Err(NativeExecutorError::ExpectedContractMetadataFoundTrait);
                }
            };
            let (recommended_state_capacity, recommended_slot_capacity, recommended_tmp_capacity) =
                recommended_capacities;

            if methods_by_code
                .insert(
                    (contact_code.as_bytes(), method_fingerprint),
                    MethodDetails {
                        recommended_state_capacity,
                        recommended_slot_capacity,
                        recommended_tmp_capacity,
                        method_metadata,
                        ffi_fn,
                    },
                )
                .is_some()
            {
                return Err(NativeExecutorError::DuplicateMethodInContract {
                    contact_code,
                    method_fingerprint,
                });
            }
        }

        // Manually deploy code of system code contract
        let slots = HashMap::from_iter([(
            (Address::SYSTEM_CODE, Address::SYSTEM_CODE),
            SharedAlignedBuffer::from_bytes(Code::code().get_initialized()),
        )]);

        let address_allocator_address = Address::system_address_allocator(shard_index);
        let slots = Slots::new(slots);
        {
            let nested_slots = slots.lock().new_nested();
            let nested_slots = &mut *nested_slots.lock();
            // Allow deployment of system address allocator and state contracts
            assert!(nested_slots.add_new_contract(address_allocator_address));
            assert!(nested_slots.add_new_contract(Address::SYSTEM_STATE));
        }

        let mut instance = Self {
            shard_index,
            methods_by_code: Arc::new(methods_by_code),
            // TODO: Allow to specify initial slots as an argument and extract it from executor
            //  for persistence
            slots,
        };

        // Deploy and initialize other system contacts
        {
            let mut env = instance.env(Address::SYSTEM_CODE, Address::SYSTEM_CODE);
            env.code_store(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &Address::SYSTEM_STATE,
                &State::code(),
            )
            .map_err(|error| NativeExecutorError::FailedToDeploySystemContracts { error })?;
            env.code_store(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &address_allocator_address,
                &AddressAllocator::code(),
            )
            .map_err(|error| NativeExecutorError::FailedToDeploySystemContracts { error })?;
            env.address_allocator_new(MethodContext::Keep, address_allocator_address)
                .map_err(|error| NativeExecutorError::FailedToDeploySystemContracts { error })?;
        }

        Ok(instance)
    }

    // TODO: Remove this once there is a better way to do this
    /// Mark slot as used, such that execution environment can read/write from/to it
    pub fn use_slot(&mut self, owner: Address, contract: Address) {
        self.slots.lock().use_slot((owner, contract));
    }

    /// Run a function under fresh execution environment
    pub fn env(&mut self, context: Address, caller: Address) -> Env<'_> {
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: Default::default(),
            own_address: caller,
            context,
            caller,
        };

        Env::with_executor_context(
            env_state,
            Box::new(NativeExecutorContext::new(
                self.shard_index,
                Arc::clone(&self.methods_by_code),
                Arc::clone(&self.slots),
            )),
        )
    }

    /// Shortcut for [`Self::env`] with context and caller set to [`Address::NULL`]
    #[inline]
    pub fn null_env(&mut self) -> Env<'_> {
        self.env(Address::NULL, Address::NULL)
    }
}
