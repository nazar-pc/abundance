#![feature(non_null_from_ref, pointer_is_aligned_to)]

mod context;

use crate::context::{MethodDetails, NativeExecutorContext};
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
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tracing::error;

pub struct NativeEnv<'a> {
    env: Env,
    phantom_data: PhantomData<&'a ()>,
}

impl Deref for NativeEnv<'_> {
    type Target = Env;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.env
    }
}

impl DerefMut for NativeEnv<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.env
    }
}

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
    #[error("Duplicate method in contract {crate_name}: {method_fingerprint}")]
    DuplicateMethodInContract {
        /// Name of the crate in which method was duplicated
        crate_name: &'static str,
        /// Method fingerprint
        method_fingerprint: &'static MethodFingerprint,
    },
}

pub struct NativeExecutor {
    context: Arc<NativeExecutorContext>,
}

impl NativeExecutor {
    /// Instantiate in-memory native executor.
    ///
    /// Returns error in case of method duplicates.
    pub fn in_memory(shard_index: ShardIndex) -> Result<Self, NativeExecutorError> {
        let mut methods_by_code = HashMap::<_, HashMap<_, _>>::new();
        for &contract_methods_fn_pointer in inventory::iter::<ContractsMethodsFnPointer> {
            let ContractsMethodsFnPointer {
                crate_name,
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
                .entry(crate_name.as_bytes())
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
                    crate_name,
                    method_fingerprint,
                });
            }
        }

        let context = NativeExecutorContext::new(shard_index, methods_by_code);

        Ok(Self { context })
    }

    /// Run a function under fresh execution environment
    pub fn env(&mut self, context: Address, caller: Address) -> NativeEnv<'_> {
        let env_state = EnvState {
            shard_index: self.context.shard_index(),
            own_address: Address::NULL,
            context,
            caller,
        };

        let env = Env::with_executor_context(env_state, Arc::clone(&self.context) as _);

        NativeEnv {
            env,
            phantom_data: PhantomData,
        }
    }

    /// Shortcut for [`Self::env`] with context and caller set to [`Address::NULL`]
    #[inline]
    pub fn null_env(&mut self) -> NativeEnv<'_> {
        self.env(Address::NULL, Address::NULL)
    }

    /// Deploy typical system contracts at default addresses.
    ///
    /// It uses low-level method [`Self::deploy_system_contract_at()`].
    #[cfg(feature = "system-contracts")]
    pub fn deploy_typical_system_contracts(&mut self) -> Result<(), ContractError> {
        let address_allocator_address =
            Address::system_address_allocator(self.context.shard_index());
        self.deploy_system_contract_at::<AddressAllocator>(address_allocator_address);
        self.deploy_system_contract_at::<Code>(Address::SYSTEM_CODE);
        self.deploy_system_contract_at::<State>(Address::SYSTEM_STATE);

        // Initialize shard state
        let env = &mut *self.null_env();
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
        self.context
            .force_insert(address, Address::SYSTEM_CODE, C::CRATE_NAME.as_bytes());
    }
}
