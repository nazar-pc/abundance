use crate::ContractError;
use crate::method::{ExternalArgs, MethodFingerprint};
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

/// Context for method call.
///
/// The correct mental model for context is "user of the child process," where "process" is a method
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
    /// Explicit padding, contents must be all zeroes
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
        prepared_methods: &mut PreparedMethod<'_>,
    ) -> Result<(), ContractError>;
}

#[cfg(all(feature = "executor", feature = "guest", not(any(doc, unix, windows))))]
compile_error!(
    "`executor` and `guest` features are mutually exclusive due to it affecting `Env` layout"
);

/// Ephemeral execution environment.
///
/// In guest environment equivalent to just [`EnvState`], while on Unix and Windows an executor
/// context is also present
#[derive(Debug)]
#[repr(C)]
pub struct Env<'a> {
    state: EnvState,
    #[cfg(feature = "executor")]
    executor_context: &'a mut dyn ExecutorContext,
    phantom_data: PhantomData<&'a ()>,
}

// TODO: API to "attach" data structures to the environment to make sure pointers to it can be
//  returned safely, will likely require `Pin` and return some reference from which pointer is to
//  be created
impl<'a> Env<'a> {
    /// Instantiate environment with executor context
    #[cfg(feature = "executor")]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn with_executor_context(
        state: EnvState,
        executor_context: &'a mut dyn ExecutorContext,
    ) -> Self {
        Self {
            state,
            executor_context,
            phantom_data: PhantomData,
        }
    }

    /// Instantiate environment with executor context
    #[cfg(feature = "executor")]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn get_mut_executor_context(&mut self) -> &mut dyn ExecutorContext {
        self.executor_context
    }

    /// Shard index where execution is happening
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn shard_index(&self) -> ShardIndex {
        self.state.shard_index
    }

    /// Own address of the contract
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn own_address(&self) -> Address {
        self.state.own_address
    }

    /// Context of the execution
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn context<'b>(self: &'b &'b mut Self) -> Address {
        self.state.context
    }

    /// Caller of this contract
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn caller<'b>(self: &'b &'b mut Self) -> Address {
        self.state.caller
    }

    /// Call a method at specified address and with specified arguments.
    ///
    /// This is a shortcut for [`Self::prepare_method_call()`] + [`Self::call_prepared()`].
    #[inline(always)]
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
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
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
