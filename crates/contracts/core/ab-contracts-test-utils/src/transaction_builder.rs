use ab_contracts_common::method::ExternalArgs;
use ab_core_primitives::address::Address;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::transaction::owned::{OwnedTransaction, OwnedTransactionError};
use ab_core_primitives::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::{
    MethodCallReferences, TransactionPayloadBuilder, TransactionPayloadBuilderError,
};

/// Errors for [`TransactionBuilder::build()`]
#[derive(Debug, thiserror::Error)]
pub enum TransactionBuilderError {
    /// Failed to build the payload
    #[error("Failed to build the payload: {0}")]
    Payload(TransactionPayloadBuilderError<'static>),
    /// Failed to create the transaction
    #[error("Failed to create the transaction: {0}")]
    Transaction(OwnedTransactionError),
}

#[derive(Debug)]
pub struct TransactionBuilder {
    contract: Address,
    transaction_payload_builder: TransactionPayloadBuilder,
}

impl TransactionBuilder {
    /// Create a transaction for `contract` with a payload of up to `max_payload_size` bytes that
    /// declares `num_outputs` outputs.
    ///
    /// See [`TransactionPayloadBuilder::new()`] for details of `max_payload_size` and
    /// `num_outputs`.
    pub fn new(
        contract: Address,
        max_payload_size: u32,
        num_outputs: u8,
    ) -> Result<Self, TransactionPayloadBuilderError<'static>> {
        Ok(Self {
            contract,
            transaction_payload_builder: TransactionPayloadBuilder::new(
                max_payload_size,
                num_outputs,
            )?,
        })
    }

    /// Add method call to the transaction.
    ///
    /// See [`TransactionPayloadBuilder::with_method_call()`] for details of this API.
    pub fn with_method_call<Args>(
        &mut self,
        contract: &Address,
        external_args: &Args,
        method_context: TransactionMethodContext,
        references: MethodCallReferences<'_>,
    ) -> Result<(), TransactionPayloadBuilderError<'static>>
    where
        Args: ExternalArgs,
    {
        self.transaction_payload_builder.with_method_call(
            contract,
            external_args,
            method_context,
            references,
        )
    }

    /// Build the transaction.
    ///
    /// See [`TransactionPayloadBuilder::into_aligned_bytes()`] and
    /// [`OwnedTransaction::from_parts()`] for errors.
    pub fn build(
        self,
        read_slots: &[TransactionSlot],
        write_slots: &[TransactionSlot],
    ) -> Result<OwnedTransaction, TransactionBuilderError> {
        let header = TransactionHeader {
            version: 0,
            block_root: BlockRoot::default(),
            gas_limit: Gas::default(),
            contract: self.contract,
        };
        let payload = self
            .transaction_payload_builder
            .into_aligned_bytes()
            .map_err(TransactionBuilderError::Payload)?;

        OwnedTransaction::from_parts(
            &header,
            read_slots,
            write_slots,
            payload.as_u128_slice(),
            &[],
        )
        .map_err(TransactionBuilderError::Transaction)
    }
}
