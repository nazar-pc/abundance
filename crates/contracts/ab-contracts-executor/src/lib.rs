#![feature(non_null_from_ref, pointer_is_aligned_to, try_blocks)]

mod aligned_buffer;
mod context;
mod slots;

use crate::aligned_buffer::SharedAlignedBuffer;
use crate::context::{MethodDetails, NativeExecutorContext};
use crate::slots::{SlotKey, Slots};
use ab_contracts_common::env::{Env, EnvState, MethodContext, TransactionRef};
use ab_contracts_common::metadata::decode::{MetadataDecoder, MetadataDecodingError, MetadataItem};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{
    Address, Contract, ContractError, ContractTrait, ContractTraitDefinition,
    NativeExecutorContactMethod, ShardIndex,
};
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_io_type::variable_elements::VariableElements;
use ab_contracts_standards::tx_handler::TxHandlerExt;
use ab_system_contract_address_allocator::{AddressAllocator, AddressAllocatorExt};
use ab_system_contract_code::{Code, CodeExt};
use ab_system_contract_simple_wallet_base::SimpleWalletBase;
use ab_system_contract_state::State;
use halfbrown::HashMap;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::error;

// TODO: API for serialization/deserialization or some kind of access to internal contents
#[derive(Debug)]
pub struct Storage {
    slots: Arc<Mutex<Slots>>,
}

impl Storage {
    // TODO: Remove this once there is a better way to do this
    /// Mark slot as used, such that execution environment can read/write from/to it
    pub fn use_slot(&mut self, owner: Address, contract: Address) {
        self.slots.lock().use_slot(SlotKey { owner, contract });
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
    #[error("Duplicate method fingerprint {method_fingerprint} for contract code {contact_code}")]
    DuplicateMethodInContract {
        /// Name of the crate in which method was duplicated
        contact_code: &'static str,
        /// Method fingerprint
        method_fingerprint: &'static MethodFingerprint,
    },
}

#[derive(Clone)]
struct MethodsEntry {
    contact_code: &'static str,
    main_contract_metadata: &'static [u8],
    native_executor_methods: &'static [NativeExecutorContactMethod],
}

/// Builder for [`NativeExecutor`]
#[derive(Clone)]
pub struct NativeExecutorBuilder {
    shard_index: ShardIndex,
    methods: Vec<MethodsEntry>,
}

impl NativeExecutorBuilder {
    fn new(shard_index: ShardIndex) -> Self {
        Self {
            shard_index,
            // Start with system contracts
            methods: vec![
                MethodsEntry {
                    contact_code: AddressAllocator::CODE,
                    main_contract_metadata: AddressAllocator::MAIN_CONTRACT_METADATA,
                    native_executor_methods: AddressAllocator::NATIVE_EXECUTOR_METHODS,
                },
                MethodsEntry {
                    contact_code: Code::CODE,
                    main_contract_metadata: Code::MAIN_CONTRACT_METADATA,
                    native_executor_methods: Code::NATIVE_EXECUTOR_METHODS,
                },
                MethodsEntry {
                    contact_code: State::CODE,
                    main_contract_metadata: State::MAIN_CONTRACT_METADATA,
                    native_executor_methods: State::NATIVE_EXECUTOR_METHODS,
                },
            ],
        }
    }

    /// Make the native execution environment aware of the contract specified in generic argument.'
    ///
    /// Here `C` is the contract type:
    /// ```ignore
    /// # fn foo(mut builder: NativeExecutorBuilder) -> NativeExecutorBuilder {
    ///     builder.with_contract::<Flipper>()
    /// # }
    /// ```
    ///
    /// NOTE: System contracts are already included by default.
    pub fn with_contract<C>(mut self) -> Self
    where
        C: Contract,
    {
        self.methods.push(MethodsEntry {
            contact_code: C::CODE,
            main_contract_metadata: C::MAIN_CONTRACT_METADATA,
            native_executor_methods: C::NATIVE_EXECUTOR_METHODS,
        });
        self
    }

    /// Make the native execution environment aware of the trait implemented by the contract
    /// specified in generic argument.
    ///
    /// Here `C` is the contract type and `DynCT` is a trait it implements in the form of
    /// `dyn ContractTrait`:
    /// ```ignore
    /// # fn foo(mut builder: NativeExecutorBuilder) -> NativeExecutorBuilder {
    ///     builder
    ///         .with_contract::<Token>()
    ///         .with_contract_trait::<Token, dyn Fungible>()
    /// # }
    /// ```
    pub fn with_contract_trait<C, DynCT>(mut self) -> Self
    where
        C: Contract + ContractTrait<DynCT>,
        DynCT: ContractTraitDefinition + ?Sized,
    {
        self.methods.push(MethodsEntry {
            contact_code: C::CODE,
            main_contract_metadata: C::MAIN_CONTRACT_METADATA,
            native_executor_methods: <C as ContractTrait<DynCT>>::NATIVE_EXECUTOR_METHODS,
        });
        self
    }

    /// Build native execution configuration
    pub fn build(self) -> Result<NativeExecutor, NativeExecutorError> {
        // 10 is a decent capacity for many typical cases without reallocation
        let mut methods_by_code = HashMap::with_capacity(10);
        for methods_entry in self.methods {
            let MethodsEntry {
                contact_code,
                main_contract_metadata,
                native_executor_methods,
            } = methods_entry;
            for &native_executor_method in native_executor_methods {
                let NativeExecutorContactMethod {
                    method_fingerprint,
                    method_metadata,
                    ffi_fn,
                } = native_executor_method;
                let recommended_capacities = match MetadataDecoder::new(main_contract_metadata)
                    .decode_next()
                    .ok_or(NativeExecutorError::ContractMetadataNotFound)?
                    .map_err(|error| NativeExecutorError::ContractMetadataDecodingError { error })?
                {
                    MetadataItem::Contract {
                        state_type_details,
                        slot_type_details,
                        tmp_type_details,
                        ..
                    } => (
                        state_type_details.recommended_capacity,
                        slot_type_details.recommended_capacity,
                        tmp_type_details.recommended_capacity,
                    ),
                    MetadataItem::Trait { .. } => {
                        return Err(NativeExecutorError::ExpectedContractMetadataFoundTrait);
                    }
                };
                let (
                    recommended_state_capacity,
                    recommended_slot_capacity,
                    recommended_tmp_capacity,
                ) = recommended_capacities;

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
        }

        Ok(NativeExecutor {
            shard_index: self.shard_index,
            methods_by_code: Arc::new(methods_by_code),
        })
    }
}

// TODO: Some kind of transaction notion with `#[tmp]` wiped at the end of it
pub struct NativeExecutor {
    shard_index: ShardIndex,
    /// Indexed by contract's code and method fingerprint
    methods_by_code: Arc<HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>>,
}

impl NativeExecutor {
    /// Create a new storage instance with system contracts already deployed
    pub fn new_storage(&self) -> Result<Storage, ContractError> {
        // Manually deploy code of system code contract
        let slots = [(
            SlotKey {
                owner: Address::SYSTEM_CODE,
                contract: Address::SYSTEM_CODE,
            },
            SharedAlignedBuffer::from_bytes(Code::code().get_initialized()),
        )];

        let address_allocator_address = Address::system_address_allocator(self.shard_index);
        let mut storage = Storage {
            slots: Slots::new(slots),
        };
        {
            let nested_slots = storage.slots.lock().new_nested();
            let nested_slots = &mut *nested_slots.lock();
            // Allow deployment of system contracts
            assert!(nested_slots.add_new_contract(address_allocator_address));
            assert!(nested_slots.add_new_contract(Address::SYSTEM_STATE));
            assert!(nested_slots.add_new_contract(Address::SYSTEM_SIMPLE_WALLET_BASE));
        }

        // Deploy and initialize other system contacts
        self.transaction_emulate(Address::SYSTEM_CODE, &mut storage, |env| {
            env.code_store(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &Address::SYSTEM_STATE,
                &State::code(),
            )?;

            env.code_store(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &address_allocator_address,
                &AddressAllocator::code(),
            )?;
            env.address_allocator_new(MethodContext::Keep, address_allocator_address)?;

            env.code_store(
                MethodContext::Keep,
                Address::SYSTEM_CODE,
                &Address::SYSTEM_SIMPLE_WALLET_BASE,
                &SimpleWalletBase::code(),
            )
        })?;

        Ok(storage)
    }

    /// Builder of native executor for specified shard index
    pub fn builder(shard_index: ShardIndex) -> NativeExecutorBuilder {
        NativeExecutorBuilder::new(shard_index)
    }

    /// Verify the provided transaction.
    ///
    /// For transaction execution [`Self::transaction_verify()`] can be used.
    pub fn transaction_verify(
        &self,
        transaction: TransactionRef<'_>,
        storage: &Storage,
    ) -> Result<(), ContractError> {
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: Default::default(),
            own_address: Address::NULL,
            context: Address::NULL,
            caller: Address::NULL,
        };

        let payload_size = size_of_val::<[u128]>(transaction.payload) as u32;
        let payload = VariableElements::from_buffer(transaction.payload, &payload_size);
        let seal_size = size_of_val::<[u8]>(transaction.seal) as u32;
        let seal = VariableBytes::from_buffer(transaction.seal, &seal_size);

        // TODO: Make it more efficient by not recreating NativeExecutorContext twice here
        let env = Env::with_executor_context(
            env_state,
            Box::new(NativeExecutorContext::new(
                self.shard_index,
                Arc::clone(&self.methods_by_code),
                Arc::clone(&storage.slots),
                false,
            )),
        );
        env.tx_handler_authorize(
            transaction.header.contract,
            transaction.header,
            &payload,
            &seal,
        )
    }

    /// Execute the provided transaction.
    ///
    /// If only verification is needed, [`Self::transaction_verify()`] can be used instead.
    pub fn transaction_execute(
        &self,
        transaction: TransactionRef<'_>,
        storage: &mut Storage,
    ) -> Result<(), ContractError> {
        // TODO: This is a pretty large data structure to copy around, try to make it a reference
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: Default::default(),
            own_address: Address::NULL,
            context: Address::NULL,
            caller: Address::NULL,
        };

        let payload_size = size_of_val::<[u128]>(transaction.payload) as u32;
        let payload = VariableElements::from_buffer(transaction.payload, &payload_size);
        let seal_size = size_of_val::<[u8]>(transaction.seal) as u32;
        let seal = VariableBytes::from_buffer(transaction.seal, &seal_size);

        // TODO: Make it more efficient by not recreating NativeExecutorContext twice here
        {
            let env = Env::with_executor_context(
                env_state,
                Box::new(NativeExecutorContext::new(
                    self.shard_index,
                    Arc::clone(&self.methods_by_code),
                    Arc::clone(&storage.slots),
                    false,
                )),
            );
            env.tx_handler_authorize(
                transaction.header.contract,
                transaction.header,
                &payload,
                &seal,
            )?;
        }

        {
            let mut env = Env::with_executor_context(
                env_state,
                Box::new(NativeExecutorContext::new(
                    self.shard_index,
                    Arc::clone(&self.methods_by_code),
                    Arc::clone(&storage.slots),
                    true,
                )),
            );
            env.tx_handler_execute(
                MethodContext::Reset,
                transaction.header.contract,
                transaction.header,
                &payload,
                &seal,
            )?;
        }

        Ok(())
    }

    /// Emulate a transaction submitted by `contract` with method calls happening inside `calls`
    /// without going through `TxHandler`.
    ///
    /// NOTE: This is primarily useful for testing environment, usually changes are done in the
    /// transaction execution using [`Self::transaction_execute()`].
    pub fn transaction_emulate<Calls, T>(
        &self,
        contract: Address,
        storage: &mut Storage,
        calls: Calls,
    ) -> T
    where
        Calls: FnOnce(&mut Env) -> T,
    {
        let mut env = self.env_internal(contract, storage, true);
        calls(&mut env)
    }

    /// Get a read-only `Env` instance for calling `#[view]` methods on it directly.
    ///
    /// For stateful methods, execute a transaction using [`Self::transaction_execute()`] or
    /// emulate one with [`Self::transaction_emulate()`].
    pub fn env_ro(&self, storage: &Storage) -> Env<'_> {
        self.env_internal(Address::NULL, storage, false)
    }

    fn env_internal(
        &self,
        contract: Address,
        storage: &Storage,
        allow_env_mutation: bool,
    ) -> Env<'_> {
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: Default::default(),
            own_address: contract,
            context: contract,
            caller: Address::NULL,
        };

        Env::with_executor_context(
            env_state,
            Box::new(NativeExecutorContext::new(
                self.shard_index,
                Arc::clone(&self.methods_by_code),
                Arc::clone(&storage.slots),
                allow_env_mutation,
            )),
        )
    }
}
