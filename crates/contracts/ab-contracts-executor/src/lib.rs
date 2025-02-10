#![feature(non_null_from_ref, pointer_is_aligned_to)]

mod aligned_buffer;
mod context;
mod slots;

use crate::aligned_buffer::SharedAlignedBuffer;
use crate::context::{MethodDetails, NativeExecutorContext};
use crate::slots::Slots;
use ab_contracts_common::env::{Env, EnvState, MethodContext};
use ab_contracts_common::metadata::decode::{MetadataDecoder, MetadataDecodingError, MetadataItem};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{
    Address, Contract, ContractError, ContractsMethodsFnPointer, ShardIndex,
};
#[cfg(feature = "system-contracts")]
use ab_system_contract_address_allocator::{AddressAllocator, AddressAllocatorExt};
#[cfg(feature = "system-contracts")]
use ab_system_contract_code::Code;
#[cfg(feature = "system-contracts")]
use ab_system_contract_state::State;
use parking_lot::Mutex;
use std::collections::HashMap;
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
}

// TODO: `NativeExecutorBuilder` that allows to inject contracts and trait implementations
//  explicitly instead of relying on `inventory` that silently doesn't work under Miri
pub struct NativeExecutor {
    shard_index: ShardIndex,
    methods_by_code: Arc<HashMap<&'static [u8], HashMap<MethodFingerprint, MethodDetails>>>,
    slots: Arc<Mutex<Slots>>,
}

impl NativeExecutor {
    /// Instantiate in-memory native executor.
    ///
    /// Returns error in case of method duplicates.
    pub fn in_memory(shard_index: ShardIndex) -> Result<Self, NativeExecutorError> {
        let mut methods_by_code = HashMap::<_, HashMap<_, _>>::new();
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
                .entry(contact_code.as_bytes())
                .or_default()
                .insert(
                    *method_fingerprint,
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

        let methods_by_code = Arc::new(methods_by_code);

        Ok(Self {
            shard_index,
            methods_by_code,
            slots: Arc::default(),
        })
    }

    /// Run a function under fresh execution environment
    pub fn env(&mut self, context: Address, caller: Address) -> Env<'_> {
        let env_state = EnvState {
            shard_index: self.shard_index,
            own_address: Address::NULL,
            context,
            caller,
        };

        Env::with_executor_context(
            env_state,
            Box::new(NativeExecutorContext::new(
                self.shard_index,
                Arc::clone(&self.methods_by_code),
                Arc::clone(&self.slots),
                true,
            )),
        )
    }

    /// Shortcut for [`Self::env`] with context and caller set to [`Address::NULL`]
    #[inline]
    pub fn null_env(&mut self) -> Env<'_> {
        self.env(Address::NULL, Address::NULL)
    }

    /// Deploy typical system contracts at default addresses.
    ///
    /// It uses low-level method [`Self::deploy_system_contract_at()`].
    #[cfg(feature = "system-contracts")]
    pub fn deploy_typical_system_contracts(&mut self) -> Result<(), ContractError> {
        let address_allocator_address = Address::system_address_allocator(self.shard_index);
        self.deploy_system_contract_at::<AddressAllocator>(address_allocator_address);
        self.deploy_system_contract_at::<Code>(Address::SYSTEM_CODE);
        self.deploy_system_contract_at::<State>(Address::SYSTEM_STATE);

        // Initialize shard state
        let env = &mut self.null_env();
        env.address_allocator_new(MethodContext::Keep, address_allocator_address)?;

        Ok(())
    }

    /// Deploy a system contract at a known address.
    ///
    /// It is used by convenient high-level helper method `Self::deploy_typical_system_contracts()`
    /// and often doesn't need to be called directly.
    pub fn deploy_system_contract_at<C>(&mut self, address: Address)
    where
        C: Contract,
    {
        self.slots.lock().put(
            address,
            Address::SYSTEM_CODE,
            SharedAlignedBuffer::from_bytes(C::code().get_initialized()),
        );
    }
}
