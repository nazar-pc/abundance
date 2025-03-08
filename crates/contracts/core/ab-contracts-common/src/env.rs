#[cfg(feature = "alloc")]
extern crate alloc;

use crate::method::{ExternalArgs, MethodFingerprint};
use crate::{Address, ContractError, ShardIndex};
use ab_contracts_io_type::trivial_type::TrivialType;
#[cfg(feature = "executor")]
use alloc::boxed::Box;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

// TODO: New type
pub type Blake3Hash = [u8; 32];

/// A measure of compute resources, 1 Gas == 1 ns of compute on reference hardware
#[derive(Debug, Default, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Gas(u64);

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct TransactionHeader {
    pub block_hash: Blake3Hash,
    pub gas_limit: Gas,
    /// Contract implementing `TxHandler` trait to use for transaction verification and execution
    pub contract: Address,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, TrivialType)]
#[repr(C)]
pub struct TransactionSlot {
    pub owner: Address,
    pub contract: Address,
}

/// Similar to `Transaction`, but doesn't require `allow` or data ownership.
///
/// Can be created with `Transaction::as_ref()` call.
#[derive(Debug, Copy, Clone)]
pub struct Transaction<'a> {
    pub header: &'a TransactionHeader,
    /// Slots in the form of [`TransactionSlot`] that may be read during transaction processing.
    ///
    /// The code slot of the contract that is being executed is implicitly included and doesn't need
    /// to be repeated. Also slots that may also be written to do not need to be repeated in the
    /// read slots.
    pub read_slots: &'a [TransactionSlot],
    /// Slots in the form of [`TransactionSlot`] that may be written during transaction processing
    pub write_slots: &'a [TransactionSlot],
    pub payload: &'a [u128],
    pub seal: &'a [u8],
}

/// Context for method call.
///
/// The correct mental model for context is "user of the child process", where "process" is a method
/// call. Essentially, something executed with a context of a contract can be thought as done
/// "on behalf" of that contract, which depending on circumstances may or may not be desired.
///
/// Initially, context is [`Address::NULL`]. For each call into another contract, the context of the
/// current method can be either preserved, reset to [`Address::NULL`] or replaced with the current
/// contract's address. Those are the only options. Contracts do not have privileges to change
/// context to the address of an arbitrary contract.
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(u8)]
pub enum MethodContext {
    /// Keep current context
    Keep,
    /// Reset context to [`Address::NULL`]
    Reset,
    /// Replace context with current contract's address
    Replace,
}

/// Method to be called by the executor
#[derive(Debug)]
#[repr(C)]
#[must_use]
pub struct PreparedMethod<'a> {
    /// Address of the contract that contains a function to below fingerprint
    pub contract: Address,
    /// Fingerprint of the method being called
    pub fingerprint: MethodFingerprint,
    /// Anonymous pointer to a struct that implements `ExternalArgs` of the method with above
    /// `fingerprint`
    pub external_args: NonNull<NonNull<c_void>>,
    /// Context for method call
    pub method_context: MethodContext,
    /// Used to tie the lifetime to `ExternalArgs`
    pub phantom: PhantomData<&'a ()>,
}

/// Environment state
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct EnvState {
    /// Shard index where execution is happening
    pub shard_index: ShardIndex,
    /// Explicit padding, contents does not matter
    pub padding_0: [u8; 4],
    /// Own address of the contract
    pub own_address: Address,
    /// Context of the execution
    pub context: Address,
    /// Caller of this contract
    pub caller: Address,
}

/// Executor context that can be used to interact with executor
#[cfg(feature = "executor")]
pub trait ExecutorContext: core::fmt::Debug {
    /// Call prepared method
    fn call(
        &self,
        previous_env_state: &EnvState,
        prepared_methods: &mut PreparedMethod,
    ) -> Result<(), ContractError>;
}

/// Ephemeral execution environment.
///
/// In guest environment equivalent to just [`EnvState`], while on Unix and Windows an executor
/// context is also present
#[derive(Debug)]
#[repr(C)]
pub struct Env<'a> {
    state: EnvState,
    #[cfg(feature = "executor")]
    executor_context: Box<dyn ExecutorContext + 'a>,
    phantom_data: PhantomData<&'a ()>,
}

// TODO: API to "attach" data structures to the environment to make sure pointers to it can be
//  returned safely, will likely require `Pin` and return some reference from which pointer is to
//  be created
impl<'a> Env<'a> {
    /// Instantiate environment with executor context
    #[cfg(feature = "executor")]
    #[inline]
    pub fn with_executor_context(
        state: EnvState,
        executor_context: Box<dyn ExecutorContext + 'a>,
    ) -> Self {
        Self {
            state,
            executor_context,
            phantom_data: PhantomData,
        }
    }

    /// Instantiate environment with executor context
    #[cfg(feature = "executor")]
    #[inline]
    pub fn get_mut_executor_context(&mut self) -> &mut (dyn ExecutorContext + 'a) {
        self.executor_context.as_mut()
    }

    /// Shard index where execution is happening
    #[inline]
    pub fn shard_index(&self) -> ShardIndex {
        self.state.shard_index
    }

    /// Own address of the contract
    #[inline]
    pub fn own_address(&self) -> Address {
        self.state.own_address
    }

    /// Context of the execution
    #[inline]
    pub fn context<'b>(self: &'b &'b mut Self) -> Address {
        self.state.context
    }

    /// Caller of this contract
    #[inline]
    pub fn caller<'b>(self: &'b &'b mut Self) -> Address {
        self.state.caller
    }

    /// Call a method at specified address and with specified arguments.
    ///
    /// This is a shortcut for [`Self::prepare_method_call()`] + [`Self::call_prepared()`].
    #[inline]
    pub fn call<Args>(
        &self,
        contract: Address,
        args: &mut Args,
        method_context: MethodContext,
    ) -> Result<(), ContractError>
    where
        Args: ExternalArgs,
    {
        let prepared_method = Self::prepare_method_call(contract, args, method_context);
        self.call_prepared(prepared_method)
    }

    /// Prepare a single method for calling at specified address and with specified arguments.
    ///
    /// The result is to be used with [`Self::call_prepared()`] afterward.
    #[inline]
    pub fn prepare_method_call<Args>(
        contract: Address,
        args: &mut Args,
        method_context: MethodContext,
    ) -> PreparedMethod<'_>
    where
        Args: ExternalArgs,
    {
        PreparedMethod {
            contract,
            fingerprint: Args::FINGERPRINT,
            // TODO: Method on `ExternalArgs` that returns an iterator over pointers
            external_args: NonNull::from_mut(args).cast::<NonNull<c_void>>(),
            method_context,
            phantom: PhantomData,
        }
    }

    /// Call prepared method.
    ///
    /// In most cases, this doesn't need to be called directly. Extension traits provide a more
    /// convenient way to make method calls and are enough in most cases.
    #[inline]
    pub fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError> {
        #[cfg(feature = "executor")]
        {
            let mut method = method;
            self.executor_context.call(&self.state, &mut method)
        }
        #[cfg(all(feature = "guest", not(feature = "executor")))]
        {
            let _ = method;
            todo!()
        }
        #[cfg(not(any(feature = "executor", feature = "guest")))]
        {
            let _ = method;
            Err(ContractError::InternalError)
        }
    }
}
