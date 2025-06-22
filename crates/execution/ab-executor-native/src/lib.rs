#![feature(ptr_as_ref_unchecked, unsafe_cell_access)]

mod context;

use crate::context::{MethodDetails, NativeExecutorContext};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_contracts_common::env::{Env, EnvState, MethodContext};
use ab_contracts_common::metadata::decode::{MetadataDecoder, MetadataDecodingError, MetadataItem};
use ab_contracts_common::method::MethodFingerprint;
use ab_contracts_common::{
    Contract, ContractError, ContractTrait, ContractTraitDefinition, MAX_CODE_SIZE,
    NativeExecutorContactMethod,
};
use ab_contracts_standards::fungible::Fungible;
use ab_contracts_standards::tx_handler::TxHandlerExt;
use ab_core_primitives::address::Address;
use ab_core_primitives::balance::Balance;
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::{Transaction, TransactionHeader, TransactionSlot};
use ab_executor_slots::{Slot, SlotKey, Slots};
use ab_io_type::variable_bytes::VariableBytes;
use ab_io_type::variable_elements::VariableElements;
use ab_system_contract_address_allocator::{AddressAllocator, AddressAllocatorExt};
use ab_system_contract_block::{Block, BlockExt};
use ab_system_contract_code::{Code, CodeExt};
use ab_system_contract_native_token::{NativeToken, NativeTokenExt};
use ab_system_contract_simple_wallet_base::SimpleWalletBase;
use ab_system_contract_state::State;
use halfbrown::HashMap;
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

#[derive(Debug, Clone)]
struct MethodsEntry {
    contact_code: &'static str,
    main_contract_metadata: &'static [u8],
    native_executor_methods: &'static [NativeExecutorContactMethod],
}

/// Builder for [`NativeExecutor`]
#[derive(Debug, Clone)]
pub struct NativeExecutorBuilder {
    shard_index: ShardIndex,
    methods: Vec<MethodsEntry>,
}

impl NativeExecutorBuilder {
    fn new(shard_index: ShardIndex) -> Self {
        let instance = Self {
            shard_index,
            methods: Vec::new(),
        };

        // Start with system contracts
        instance
            .with_contract::<AddressAllocator>()
            .with_contract::<Block>()
            .with_contract::<Code>()
            .with_contract::<NativeToken>()
            .with_contract_trait::<NativeToken, dyn Fungible>()
            .with_contract::<SimpleWalletBase>()
            .with_contract::<State>()
    }

    /// Make the native execution environment aware of the contract specified in generic argument.
    ///
    /// Here `C` is the contract type:
    /// ```ignore
    /// # fn foo(mut builder: NativeExecutorBuilder) -> NativeExecutorBuilder {
    ///     builder.with_contract::<Flipper>()
    /// # }
    /// ```
    ///
    /// NOTE: System contracts are already included by default.
    #[must_use]
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
    #[must_use]
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
            methods_by_code,
        })
    }
}

#[derive(Debug)]
pub struct NativeExecutor {
    shard_index: ShardIndex,
    /// Indexed by contract's code and method fingerprint
    methods_by_code: HashMap<(&'static [u8], &'static MethodFingerprint), MethodDetails>,
}

impl NativeExecutor {
    /// Create a new [`Slots`] instance with system contracts already deployed
    pub fn new_storage_slots(&self) -> Result<Slots, ContractError> {
        // Manually deploy code of system code contract
        let slots = [Slot::ReadWrite {
            key: SlotKey {
                owner: Address::SYSTEM_CODE,
                contract: Address::SYSTEM_CODE,
            },
            buffer: SharedAlignedBuffer::from_bytes(Code::code().get_initialized()),
        }];

        let address_allocator_address = Address::system_address_allocator(self.shard_index);
        let mut slots = Slots::new(slots);

        let system_contracts: [(Address, &VariableBytes<{ MAX_CODE_SIZE }>); _] = [
            (Address::SYSTEM_STATE, &State::code()),
            (address_allocator_address, &AddressAllocator::code()),
            (Address::SYSTEM_BLOCK, &Block::code()),
            (Address::SYSTEM_NATIVE_TOKEN, &NativeToken::code()),
            (
                Address::SYSTEM_SIMPLE_WALLET_BASE,
                &SimpleWalletBase::code(),
            ),
        ];

        {
            let mut nested_slots = slots.new_nested_rw();
            // Allow deployment of system contracts
            for (address, _code) in system_contracts {
                assert!(nested_slots.add_new_contract(address));
            }
        }

        // Deploy and initialize other system contacts
        self.transaction_emulate(Address::NULL, &mut slots, |env| {
            for (address, code) in system_contracts {
                env.code_store(MethodContext::Reset, Address::SYSTEM_CODE, &address, code)?;
            }

            env.address_allocator_new(MethodContext::Reset, address_allocator_address)?;
            env.block_genesis(MethodContext::Reset, Address::SYSTEM_BLOCK)?;
            env.native_token_initialize(
                MethodContext::Reset,
                Address::SYSTEM_NATIVE_TOKEN,
                &Address::SYSTEM_NATIVE_TOKEN,
                // TODO: The balance should not be `max` by default, define rules and use them
                &Balance::MAX,
            )
        })?;

        Ok(slots)
    }

    /// Builder of native executor for specified shard index
    #[must_use]
    pub fn builder(shard_index: ShardIndex) -> NativeExecutorBuilder {
        NativeExecutorBuilder::new(shard_index)
    }

    /// Verify the provided transaction.
    ///
    /// [`Self::transaction_execute()`] can be used for transaction execution if needed.
    /// [`Self::transaction_verify_execute()`] can be used to verify and execute a transaction with
    /// a single call.
    pub fn transaction_verify(
        &self,
        transaction: Transaction<'_>,
        slots: &Slots,
    ) -> Result<(), ContractError> {
        if transaction.header.version != TransactionHeader::TRANSACTION_VERSION {
            return Err(ContractError::BadInput);
        }

        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: [0; _],
            own_address: Address::NULL,
            context: Address::NULL,
            caller: Address::NULL,
        };

        let read_slots_size =
            u32::try_from(size_of_val::<[TransactionSlot]>(transaction.read_slots))
                .map_err(|_error| ContractError::BadInput)?;
        let read_slots = VariableElements::from_buffer(transaction.read_slots, &read_slots_size);

        let write_slots_size =
            u32::try_from(size_of_val::<[TransactionSlot]>(transaction.write_slots))
                .map_err(|_error| ContractError::BadInput)?;
        let write_slots = VariableElements::from_buffer(transaction.write_slots, &write_slots_size);

        let payload_size = u32::try_from(size_of_val::<[u128]>(transaction.payload))
            .map_err(|_error| ContractError::BadInput)?;
        let payload = VariableElements::from_buffer(transaction.payload, &payload_size);

        let seal_size = u32::try_from(size_of_val::<[u8]>(transaction.seal))
            .map_err(|_error| ContractError::BadInput)?;
        let seal = VariableBytes::from_buffer(transaction.seal, &seal_size);

        let mut executor_context = NativeExecutorContext::new(
            self.shard_index,
            &self.methods_by_code,
            slots.new_nested_ro(),
            false,
        );
        let env = Env::with_executor_context(env_state, &mut executor_context);
        env.tx_handler_authorize(
            transaction.header.contract,
            transaction.header,
            &read_slots,
            &write_slots,
            &payload,
            &seal,
        )
    }

    /// Execute the previously verified transaction.
    ///
    /// [`Self::transaction_verify()`] must be used for verification.
    /// [`Self::transaction_verify_execute()`] can be used to verify and execute a transaction with
    /// a single call.
    pub fn transaction_execute(
        &self,
        transaction: Transaction<'_>,
        slots: &mut Slots,
    ) -> Result<(), ContractError> {
        if transaction.header.version != TransactionHeader::TRANSACTION_VERSION {
            return Err(ContractError::BadInput);
        }

        // TODO: This is a pretty large data structure to copy around, try to make it a reference
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: [0; _],
            own_address: Address::NULL,
            context: Address::NULL,
            caller: Address::NULL,
        };

        let read_slots_size =
            u32::try_from(size_of_val::<[TransactionSlot]>(transaction.read_slots))
                .map_err(|_error| ContractError::BadInput)?;
        let read_slots = VariableElements::from_buffer(transaction.read_slots, &read_slots_size);

        let write_slots_size =
            u32::try_from(size_of_val::<[TransactionSlot]>(transaction.write_slots))
                .map_err(|_error| ContractError::BadInput)?;
        let write_slots = VariableElements::from_buffer(transaction.write_slots, &write_slots_size);

        let payload_size = u32::try_from(size_of_val::<[u128]>(transaction.payload))
            .map_err(|_error| ContractError::BadInput)?;
        let payload = VariableElements::from_buffer(transaction.payload, &payload_size);

        let seal_size = u32::try_from(size_of_val::<[u8]>(transaction.seal))
            .map_err(|_error| ContractError::BadInput)?;
        let seal = VariableBytes::from_buffer(transaction.seal, &seal_size);

        let mut executor_context = NativeExecutorContext::new(
            self.shard_index,
            &self.methods_by_code,
            slots.new_nested_rw(),
            true,
        );

        let mut env = Env::with_executor_context(env_state, &mut executor_context);
        env.tx_handler_execute(
            MethodContext::Reset,
            transaction.header.contract,
            transaction.header,
            &read_slots,
            &write_slots,
            &payload,
            &seal,
        )
    }

    /// Verify and execute provided transaction.
    ///
    /// A shortcut for [`Self::transaction_verify()`] + [`Self::transaction_execute()`].
    pub fn transaction_verify_execute(
        &self,
        transaction: Transaction<'_>,
        slots: &mut Slots,
    ) -> Result<(), ContractError> {
        if transaction.header.version != TransactionHeader::TRANSACTION_VERSION {
            return Err(ContractError::BadInput);
        }

        // TODO: This is a pretty large data structure to copy around, try to make it a reference
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: [0; _],
            own_address: Address::NULL,
            context: Address::NULL,
            caller: Address::NULL,
        };

        let read_slots_size =
            u32::try_from(size_of_val::<[TransactionSlot]>(transaction.read_slots))
                .map_err(|_error| ContractError::BadInput)?;
        let read_slots = VariableElements::from_buffer(transaction.read_slots, &read_slots_size);

        let write_slots_size =
            u32::try_from(size_of_val::<[TransactionSlot]>(transaction.write_slots))
                .map_err(|_error| ContractError::BadInput)?;
        let write_slots = VariableElements::from_buffer(transaction.write_slots, &write_slots_size);

        let payload_size = u32::try_from(size_of_val::<[u128]>(transaction.payload))
            .map_err(|_error| ContractError::BadInput)?;
        let payload = VariableElements::from_buffer(transaction.payload, &payload_size);

        let seal_size = u32::try_from(size_of_val::<[u8]>(transaction.seal))
            .map_err(|_error| ContractError::BadInput)?;
        let seal = VariableBytes::from_buffer(transaction.seal, &seal_size);

        // TODO: Make it more efficient by not recreating NativeExecutorContext twice here
        {
            let mut executor_context = NativeExecutorContext::new(
                self.shard_index,
                &self.methods_by_code,
                slots.new_nested_ro(),
                false,
            );
            let env = Env::with_executor_context(env_state, &mut executor_context);
            env.tx_handler_authorize(
                transaction.header.contract,
                transaction.header,
                &read_slots,
                &write_slots,
                &payload,
                &seal,
            )?;
        }

        {
            let mut executor_context = NativeExecutorContext::new(
                self.shard_index,
                &self.methods_by_code,
                slots.new_nested_rw(),
                true,
            );
            let mut env = Env::with_executor_context(env_state, &mut executor_context);
            env.tx_handler_execute(
                MethodContext::Reset,
                transaction.header.contract,
                transaction.header,
                &read_slots,
                &write_slots,
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
    ///
    /// Returns `None` if read-only [`Slots`] instance was given.
    pub fn transaction_emulate<Calls, T>(
        &self,
        contract: Address,
        slots: &mut Slots,
        calls: Calls,
    ) -> T
    where
        Calls: FnOnce(&mut Env<'_>) -> T,
    {
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: [0; _],
            own_address: contract,
            context: contract,
            caller: Address::NULL,
        };

        let mut executor_context = NativeExecutorContext::new(
            self.shard_index,
            &self.methods_by_code,
            slots.new_nested_rw(),
            true,
        );
        let mut env = Env::with_executor_context(env_state, &mut executor_context);
        calls(&mut env)
    }

    /// Get a read-only `Env` instance for calling `#[view]` methods on it directly.
    ///
    /// For stateful methods, execute a transaction using [`Self::transaction_execute()`] or
    /// emulate one with [`Self::transaction_emulate()`].
    #[must_use]
    pub fn with_env_ro<Callback, T>(&self, slots: &Slots, callback: Callback) -> T
    where
        Callback: FnOnce(&Env<'_>) -> T,
    {
        let env_state = EnvState {
            shard_index: self.shard_index,
            padding_0: [0; _],
            own_address: Address::NULL,
            context: Address::NULL,
            caller: Address::NULL,
        };

        let mut executor_context = NativeExecutorContext::new(
            self.shard_index,
            &self.methods_by_code,
            slots.new_nested_ro(),
            false,
        );
        let env = Env::with_executor_context(env_state, &mut executor_context);
        callback(&env)
    }
}
