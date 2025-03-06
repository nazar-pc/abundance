extern crate alloc;

use ab_contracts_common::Address;
use ab_contracts_common::env::{Blake3Hash, Gas, Transaction, TransactionHeader};
use ab_contracts_common::method::ExternalArgs;
use ab_system_contract_simple_wallet_base::payload::TransactionMethodContext;
use ab_system_contract_simple_wallet_base::payload::builder::{
    TransactionPayloadBuilder, TransactionPayloadBuilderError,
};
use alloc::vec::Vec;

#[derive(Debug, Clone)]
pub struct OwnedTransaction {
    pub header: TransactionHeader,
    pub payload: Vec<u128>,
    pub seal: Vec<u8>,
}

impl OwnedTransaction {
    /// Get [`Transaction`] out of owned transaction
    pub fn as_ref(&self) -> Transaction<'_> {
        Transaction {
            header: &self.header,
            payload: &self.payload,
            seal: &self.seal,
        }
    }
}

pub struct TransactionBuilder {
    contract: Address,
    transaction_payload_builder: TransactionPayloadBuilder,
}

impl TransactionBuilder {
    /// Create a transaction for `contract`
    pub fn new(contract: Address) -> Self {
        Self {
            contract,
            transaction_payload_builder: TransactionPayloadBuilder::default(),
        }
    }

    /// Add method call to the transaction.
    ///
    /// See [`TransactionPayloadBuilder::with_method_call()`] for details of this API.
    pub fn with_method_call<Args>(
        &mut self,
        contract: &Address,
        external_args: &Args,
        method_context: TransactionMethodContext,
        input_output_index: &[Option<u8>],
    ) -> Result<(), TransactionPayloadBuilderError<'static>>
    where
        Args: ExternalArgs,
    {
        self.transaction_payload_builder.with_method_call(
            contract,
            external_args,
            method_context,
            input_output_index,
        )
    }

    pub fn build(self) -> OwnedTransaction {
        OwnedTransaction {
            header: TransactionHeader {
                genesis_hash: Blake3Hash::default(),
                block_hash: Blake3Hash::default(),
                gas_limit: Gas::default(),
                contract: self.contract,
            },
            payload: self.transaction_payload_builder.into_aligned_bytes(),
            seal: Vec::new(),
        }
    }
}
