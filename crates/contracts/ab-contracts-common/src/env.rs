use crate::method::{ExternalArgs, MethodFingerprint};
use crate::{Address, ContractError};
use ab_contracts_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

/// Method to be invoked by the host
#[repr(C)]
#[must_use]
pub struct InvokeMethod<'a> {
    /// Address of the contract that contains a function to below fingerprint
    address: NonNull<Address>,
    /// Fingerprint of the method being called
    fingerprint: NonNull<MethodFingerprint>,
    /// Anonymous pointer to the arguments of the method with above fingerprint
    args: NonNull<c_void>,
    // TODO: Some flags that allow re-origin and other things or those will be separate host fns?
    _phantom: PhantomData<&'a ()>,
}

/// Ephemeral execution environment
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
#[non_exhaustive]
pub struct Env {
    origin: Address,
    own_address: Address,
}

impl Env {
    // TODO: Platform-specific env

    /// Method call origin
    pub fn origin(&mut self) -> &Address {
        &self.origin
    }

    /// Address of this contract itself
    pub fn own_address(&self) -> &Address {
        &self.own_address
    }

    /// Invoke a single method at specified address and with specified arguments.
    ///
    /// This is a shortcut for [`Self::prepare_invoke_method`] + [`Self::invoke_many`].
    pub fn invoke<Args>(&self, contract: &Address, args: &mut Args) -> Result<(), ContractError>
    where
        Args: ExternalArgs,
    {
        let invoke_method = Self::prepare_invoke_method(contract, args);
        let [result] = self.invoke_many([invoke_method]);
        result
    }

    /// Prepare a single method for invocation at specified address and with specified arguments
    pub fn prepare_invoke_method<'a, Args>(
        contract: &'a Address,
        args: &'a mut Args,
    ) -> InvokeMethod<'a>
    where
        Args: ExternalArgs,
    {
        InvokeMethod {
            // TODO: Use `NonNull::from_ref()` once stable
            address: NonNull::from(contract),
            // TODO: Use `NonNull::from_ref()` once stable
            fingerprint: NonNull::from(Args::FINGERPRINT),
            // TODO: Use `NonNull::from_ref()` once stable
            args: NonNull::from(args).cast(),
            _phantom: PhantomData,
        }
    }

    /// Invoke provided methods and wait for results.
    ///
    /// Remaining gas will be split equally between all individual invocations.
    pub fn invoke_many<const N: usize>(
        &self,
        methods: [InvokeMethod<'_>; N],
    ) -> [Result<(), ContractError>; N] {
        let _ = methods;
        todo!()
    }
}
