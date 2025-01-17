use crate::method::{ExternalArgs, MethodFingerprint};
use crate::{Address, ContractError, ShardIndex};
use ab_contracts_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

/// Context for method call.
///
/// Initially context is [`Address::NULL`]. For each call into another contract, context of the
/// current method can be either preserved, reset to [`Address::NULL`] or replaced with current
/// contract's address.
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

/// Method to be called by the host
#[repr(C)]
#[must_use]
// TODO: Once solidified, replace some pointers with inline data
pub struct PreparedMethod<'a> {
    /// Address of the contract that contains a function to below fingerprint
    address: NonNull<Address>,
    /// Fingerprint of the method being called
    fingerprint: NonNull<MethodFingerprint>,
    /// Anonymous pointer to the arguments of the method with above fingerprint
    args: NonNull<c_void>,
    /// Context for method call
    method_context: NonNull<MethodContext>,
    // TODO: Some flags that allow re-origin and other things or those will be separate host fns?
    _phantom: PhantomData<&'a ()>,
}

// TODO: More APIs
/// Ephemeral execution environment
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
#[non_exhaustive]
pub struct Env {
    shard_index: ShardIndex,
    own_address: Address,
    context: Address,
    caller: Address,
}

impl Env {
    /// Context of the execution
    pub fn shard_index(&self) -> ShardIndex {
        self.shard_index
    }

    /// Address of this contract itself
    pub fn own_address(&self) -> &Address {
        &self.own_address
    }

    /// Context of the execution
    pub fn context<'a>(self: &'a &'a mut Self) -> &'a Address {
        &self.context
    }

    /// Caller of this contract
    pub fn caller<'a>(self: &'a &'a mut Self) -> &'a Address {
        &self.caller
    }

    /// Call a single method at specified address and with specified arguments.
    ///
    /// This is a shortcut for [`Self::prepare_call_method`] + [`Self::call_many`].
    pub fn call<Args>(
        &self,
        contract: &Address,
        args: &mut Args,
        method_context: &MethodContext,
    ) -> Result<(), ContractError>
    where
        Args: ExternalArgs,
    {
        let invoke_method = Self::prepare_call_method(contract, args, method_context);
        let [result] = self.call_many([invoke_method]);
        result
    }

    /// Prepare a single method for invocation at specified address and with specified arguments
    pub fn prepare_call_method<'a, Args>(
        contract: &'a Address,
        args: &'a mut Args,
        method_context: &'a MethodContext,
    ) -> PreparedMethod<'a>
    where
        Args: ExternalArgs,
    {
        PreparedMethod {
            // TODO: Use `NonNull::from_ref()` once stable
            address: NonNull::from(contract),
            // TODO: Use `NonNull::from_ref()` once stable
            fingerprint: NonNull::from(Args::FINGERPRINT),
            // TODO: Use `NonNull::from_ref()` once stable
            args: NonNull::from(args).cast(),
            // TODO: Use `NonNull::from_ref()` once stable
            method_context: NonNull::from(method_context).cast(),
            _phantom: PhantomData,
        }
    }

    /// Invoke provided methods and wait for results.
    ///
    /// Remaining gas will be split equally between all individual invocations.
    pub fn call_many<const N: usize>(
        &self,
        methods: [PreparedMethod<'_>; N],
    ) -> [Result<(), ContractError>; N] {
        let _ = methods;
        todo!()
    }
}
