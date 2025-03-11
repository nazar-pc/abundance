use ab_contracts_common::ContractError;
use ab_contracts_common::env::{Env, TransactionHeader, TransactionSlot};
use ab_contracts_io_type::variable_bytes::VariableBytes;
use ab_contracts_io_type::variable_elements::VariableElements;
use ab_contracts_macros::contract;

pub type TxHandlerPayload = VariableElements<u128>;
pub type TxHandlerSlots = VariableElements<TransactionSlot>;
pub type TxHandlerSeal = VariableBytes;

/// A transaction handler interface prototype
#[contract]
pub trait TxHandler {
    /// Verify a transaction.
    ///
    /// Each transaction consists of a header, payload, read/write slots and a seal.
    ///
    /// Payload contains 16-byte aligned bytes, which typically represent method calls to be
    /// executed, but the serialization format for it is contract-specific.
    ///
    /// Seal typically contains nonce and a signature over transaction header and payload, used for
    /// checking whether to allow execution of methods in the payload argument.
    ///
    /// In the end, it is up to the contract implementing this trait to interpret both payload and
    /// seal in any way desired.
    ///
    /// This method is called by execution environment is used for transaction authorization. It is
    /// expected to do a limited amount of work before deciding whether execution is allowed or not.
    /// Once authorization is granted (by returning a non-error result), execution environment will
    /// deduct gas from the contract's balance and call [`TxHandler::execute()`] for actual
    /// transaction execution.
    ///
    /// It is up to the host environment to decide how much work is allowed here when verifying
    /// transaction in the transaction pool as for DoS protection. As a result, requiring too much
    /// work may prevent transaction from being included in the block at all (unless user authors
    /// the block and include the transaction themselves). Once a transaction is in the block, there
    /// are no limits to the amount of work here except the ability to pay for gas.
    #[view]
    fn authorize(
        #[env] env: &Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError>;

    /// Execute previously verified transaction.
    ///
    /// *Execution environment will call this method with `env.caller()` set to `Address::NULL`,
    /// which is very important to check!* Since there is no code deployed at `Address::NULL`, only
    /// (trusted) execution environment is able to make such a call.
    ///
    /// Getting to this stage means that verification succeeded and except charging for gas, no
    /// other state changes were made since then. If necessary, it is still possible to do
    /// additional checks that would be too expensive or not possible to do in
    /// [`TxHandler::authorize()`]. It is also important to implement a transaction replay
    /// protection mechanism such as nonce increase or similar.
    #[update]
    fn execute(
        #[env] env: &mut Env<'_>,
        #[input] header: &TransactionHeader,
        #[input] read_slots: &TxHandlerSlots,
        #[input] write_slots: &TxHandlerSlots,
        #[input] payload: &TxHandlerPayload,
        #[input] seal: &TxHandlerSeal,
    ) -> Result<(), ContractError>;
}
