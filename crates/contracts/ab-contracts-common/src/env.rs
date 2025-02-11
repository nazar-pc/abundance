#[cfg(any(unix, windows))]
extern crate alloc;

use crate::method::{ExternalArgs, MethodFingerprint};
use crate::{Address, ContractError, ShardIndex};
use ab_contracts_io_type::trivial_type::TrivialType;
#[cfg(any(unix, windows))]
use alloc::sync::Arc;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

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
    _phantom: PhantomData<&'a ()>,
}

/// Environment state
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct EnvState {
    /// Shard index where execution is happening
    pub shard_index: ShardIndex,
    /// Own address of the contract
    pub own_address: Address,
    /// Context of the execution
    pub context: Address,
    /// Caller of this contract
    pub caller: Address,
}

/// Executor context that can be used to interact with executor
#[cfg(any(unix, windows))]
pub trait ExecutorContext: alloc::fmt::Debug {
    /// Call multiple methods
    fn call_many(
        &self,
        previous_env_state: &EnvState,
        prepared_methods: &[PreparedMethod],
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
    #[cfg(any(unix, windows))]
    executor_context: Arc<dyn ExecutorContext>,
    phantom_data: PhantomData<&'a ()>,
}

// TODO: API to "attach" data structures to the environment to make sure pointers to it can be
//  returned safely, will likely require `Pin` and return some reference from which pointer is to
//  be created
impl Env<'_> {
    /// Instantiate environment with executor context
    #[cfg(any(unix, windows))]
    #[inline]
    pub fn with_executor_context(
        state: EnvState,
        executor_context: Arc<dyn ExecutorContext>,
    ) -> Self {
        Self {
            state,
            executor_context,
            phantom_data: PhantomData,
        }
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
    pub fn context<'a>(self: &'a &'a mut Self) -> Address {
        self.state.context
    }

    /// Caller of this contract
    #[inline]
    pub fn caller<'a>(self: &'a &'a mut Self) -> Address {
        self.state.caller
    }

    /// Call a single method at specified address and with specified arguments.
    ///
    /// This is a shortcut for [`Self::prepare_method_call()`] + [`Self::call_many()`].
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
        self.call_many([prepared_method])
    }

    /// Prepare a single method for calling at specified address and with specified arguments.
    ///
    /// The result is to be used with [`Self::call_many()`] afterward.
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
            external_args: NonNull::from_mut(args).cast::<NonNull<c_void>>(),
            method_context,
            _phantom: PhantomData,
        }
    }

    /// Invoke provided methods and wait for the result.
    ///
    /// The remaining gas will be split equally between all individual invocations.
    #[inline]
    pub fn call_many<const N: usize>(
        &self,
        methods: [PreparedMethod<'_>; N],
    ) -> Result<(), ContractError> {
        #[cfg(any(unix, windows))]
        {
            self.executor_context.call_many(&self.state, &methods)
        }
        #[cfg(all(feature = "guest", not(any(unix, windows))))]
        {
            let _ = methods;
            todo!()
        }
        #[cfg(not(any(unix, windows, feature = "guest")))]
        compile_error!(
            "Contracts support either native environment with Unix or Windows target OS or guest \
            environment with `guest` feature, but neither is configured"
        )
    }
}
