use crate::ContractError;
use crate::method::{ExternalArgs, MethodFingerprint};
use crate::output::Lent;
use ab_core_primitives::address::Address;
use ab_core_primitives::shard::ShardIndex;
use ab_io_type::IoType;
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
/// Initially, the context is [`Address::NULL`]. For each call into another contract, the context of
/// the current method can be either preserved, reset to [`Address::NULL`] or replaced with the
/// current contract's address. Those are the only options. Contracts do not have privileges to
/// change context to the address of an arbitrary contract.
#[derive(Debug, Copy, Clone, Eq, PartialEq, TrivialType)]
#[repr(u8)]
pub enum MethodContext {
    /// Keep current context
    Keep,
    /// Reset context to [`Address::NULL`]
    Reset,
    /// Replace context with the current contract's address
    Replace,
}

/// Method to be called by the executor.
///
/// It only describes a method call and nothing in it is checked: the executor reads arguments
/// behind `external_args` according to metadata of the method with `fingerprint`, and either checks
/// them against that metadata or trusts the caller (like the native executor does). The arguments
/// meet the safety requirements of [`Self::new_untyped()`] for the lifetime `'a`, which
/// [`Self::new()`] ensures with typed arguments. Fields can't be modified after creation, so that a
/// method prepared with [`Self::new()`] can't be retargeted to a different method or arguments by
/// accident.
#[derive(Debug)]
#[repr(C)]
#[must_use]
#[expect(
    clippy::partial_pub_fields,
    reason = "Lifetime marker is an implementation detail"
)]
pub struct PreparedMethod<'a> {
    /// Address of the contract that contains a function to the below fingerprint
    pub mut(self) contract: Address,
    /// Fingerprint of the method being called
    pub mut(self) fingerprint: MethodFingerprint,
    /// Anonymous pointer to a struct that implements `ExternalArgs` of the method with the above
    /// `fingerprint`
    pub mut(self) external_args: NonNull<c_void>,
    /// Context for method call
    pub mut(self) method_context: MethodContext,
    /// Used to tie the lifetime to `ExternalArgs`
    phantom: PhantomData<&'a ()>,
}

impl<'a> PreparedMethod<'a> {
    /// Prepare a single method for calling at specified address and with specified arguments.
    ///
    /// The result is to be used with [`Env::call_prepared()`] afterward.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new<Args>(contract: Address, args: &'a mut Args, method_context: MethodContext) -> Self
    where
        Args: ExternalArgs,
    {
        Self {
            contract,
            fingerprint: Args::FINGERPRINT,
            external_args: NonNull::from_mut(args).cast::<c_void>(),
            method_context,
            phantom: PhantomData,
        }
    }

    /// Prepare a single method for calling with arguments laid out dynamically, like in a
    /// transaction payload.
    ///
    /// Prefer [`Self::new()`] when the type of arguments is known.
    ///
    /// # Safety
    /// `external_args` must start with arguments laid out like [`ExternalArgs`] of the method with
    /// `fingerprint`, and for the lifetime `'a`:
    /// * the pointer of each slot must be aligned and point to an [`Address`] valid for reads
    /// * the pointer of each input must be valid for reads of its size, with that many bytes
    ///   initialized
    /// * the pointer of each output must be valid for writes of its capacity and for reads of its
    ///   size, which must not exceed the capacity, with that many bytes initialized
    /// * the pointer of the return value (if the method has one) must be aligned and valid for
    ///   writes of a value of the return type, while its contents don't need to be initialized
    /// * outputs and the return value must not overlap each other, slot addresses, inputs or
    ///   `external_args`, and must not be accessed other than through `external_args`, while slot
    ///   addresses and inputs must not be written to
    ///
    /// A call can leave contents behind in outputs that are not valid values of their types, even
    /// when it succeeds, and it only initializes the return value. Afterward, neither may be
    /// accessed as a value of its type without being parsed first, unless every bit pattern is a
    /// valid value.
    ///
    /// Whether sizes, capacities and alignment of inputs and outputs match the method's metadata
    /// is not part of this contract, since only the executor has the metadata and guarantees it:
    /// the RISC-V VM checks them, while the native executor trusts callers and only catches
    /// mismatches with debug assertions. Neither are their contents, which the called method
    /// parses.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub unsafe fn new_untyped<Args>(
        contract: Address,
        fingerprint: MethodFingerprint,
        external_args: &'a mut Args,
        method_context: MethodContext,
    ) -> Self {
        Self {
            contract,
            fingerprint,
            external_args: NonNull::from_mut(external_args).cast::<c_void>(),
            method_context,
            phantom: PhantomData,
        }
    }
}

#[cfg(feature = "guest")]
unsafe extern "C" {
    /// Host-level API
    fn __ab_host_call_import(method: &PreparedMethod<'_>) -> crate::ExitCode;
}

/// Internal wrapper around [`__ab_host_call_import()`] that will always have a single copy (never
/// inlined) in the binary and will be possible to locate in the final binary for inspection and
/// rewriting if necessary while completely ignoring relocation of `__ab_host_call_import()`
#[cfg(feature = "guest")]
#[unsafe(no_mangle)]
#[inline(never)]
#[doc(hidden)]
pub extern "C" fn __ab_host_call(method: &PreparedMethod<'_>) -> crate::ExitCode {
    // SAFETY: FFI call with correct argument, there are no other safety requirements
    unsafe { __ab_host_call_import(method) }
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

/// Executor context that can be used to interact with executor.
///
/// # Safety
/// [`Self::call()`] gets pointers to arguments of a method call in [`PreparedMethod`], and callers
/// like methods generated by `#[contract]` rely on implementations to:
/// * write only initialized bytes, including when returning an error
/// * write nothing but the contents of outputs within their capacity, the sizes of outputs in
///   [`ExternalArgs`] and the return value (if there is one)
/// * not read the return value, or bytes of outputs beyond their size, before writing them, since
///   they may be uninitialized
/// * keep the size of each output in [`ExternalArgs`] valid for its type and within its capacity,
///   with that many bytes of the output initialized, since extension methods set it on the output
///   they were given, as [`OutputArg::set_output_size()`] requires
/// * initialize the return value (if there is one) before returning `Ok(())`
/// * not access memory behind any of these pointers after [`Self::call()`] returns
///
/// Contents don't need to be valid values of their types: extension methods parse return values,
/// and contents of outputs are only accessed after being parsed, unless every bit pattern is a
/// valid value (see [`OutputArg`]).
///
/// [`OutputArg`]: crate::output::OutputArg
/// [`OutputArg::set_output_size()`]: crate::output::OutputArg::set_output_size
#[cfg(feature = "executor")]
pub unsafe trait ExecutorContext: core::fmt::Debug {
    /// Call prepared method
    fn call(
        &self,
        previous_env_state: &EnvState,
        prepared_method: &PreparedMethod<'_>,
    ) -> Result<(), ContractError>;
}

#[cfg(all(feature = "executor", feature = "guest", not(any(doc, unix, windows))))]
compile_error!(
    "`executor` and `guest` features are mutually exclusive due to it affecting `Env` layout"
);

// TODO: Make `Env` immovable once the `Move` trait is available in nightly, see
//  https://github.com/rust-lang/rust/issues/149607 and
//  https://goals.rust-lang.org/2026/move-trait.html. Code that gets `&mut Env` couldn't replace it
//  with another `Env` then, so the type system would guarantee that an `Env` given to code by
//  reference is the same afterward, and the second requirement in the `# Safety` of
//  `Env::with_executor_context()` could be removed
/// Ephemeral execution environment.
///
/// In guest environment equivalent to just [`EnvState`], while on Unix and Windows an executor
/// context is also present
#[derive(Debug)]
#[repr(C)]
pub struct Env<'a> {
    state: EnvState,
    #[cfg(feature = "executor")]
    executor_context: &'a dyn ExecutorContext,
    /// Makes `Env` invariant over `'a`.
    ///
    /// Code that gets `Env<'a>` can lend memory to calls with [`Env::lend_output()`], which keeps
    /// it borrowed for `'a`, and a call can leave contents behind in that memory that are not a
    /// valid value. The borrow stops that code from accessing the memory again before `'a` ends,
    /// and afterward the memory is only accessed by parsing it first (see
    /// `Env::with_executor_context()`). If `Env<'a>` could be shortened to `Env<'short>`, the code
    /// could lend memory borrowed only for `'short` and then access it directly once `'short`
    /// ends, while it may still hold an invalid value.
    invariant_lifetime: PhantomData<fn(&'a ()) -> &'a ()>,
}

// TODO: API to "attach" data structures to the environment to make sure pointers to it can be
//  returned safely, will likely require `Pin` and return some reference from which pointer is to
//  be created
impl<'a> Env<'a> {
    /// Instantiate environment with executor context.
    ///
    /// # Safety
    /// Code that uses the returned `Env` may rely on its lifetime `'a`: memory it borrows for `'a`
    /// (see [`Self::lend_output()`]) stays inaccessible to others until `'a` ends, and contents it
    /// leaves behind in that memory may not be a valid value. Hence:
    /// * memory borrowed for `'a` by code that uses the `Env` must not be accessed after `'a` ends
    ///   other than by parsing it, unless its contents accept every bit pattern. For example, a
    ///   method that creates an `Env` to lend memory it received must give it a lifetime that lasts
    ///   until the method returns, like the lifetime of the `Env` it received.
    /// * once the `Env` was given to code by reference, it must not be used afterward other than
    ///   being dropped, since that code may have replaced it with another `Env` whose lifetime only
    ///   lasts until it returns
    // TODO: Remove the second requirement once `Env` is immovable with the `Move` trait
    //  (https://github.com/rust-lang/rust/issues/149607): code given the `Env` by reference
    //  couldn't replace it then, so the type system would guarantee that it is the same `Env`
    //  afterward
    #[cfg(feature = "executor")]
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub unsafe fn with_executor_context(
        state: EnvState,
        executor_context: &'a dyn ExecutorContext,
    ) -> Self {
        Self {
            state,
            executor_context,
            invariant_lifetime: PhantomData,
        }
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

    /// Lend an `#[output]` the method received, or a field of one, to calls as an `#[output]`.
    ///
    /// The memory stays borrowed until the method returns, so after a call its contents are only
    /// accessible through the returned [`Lent`], which parses them first. The state of the
    /// contract (including the state that an `#[init]` method returns through its last
    /// `#[output]`) and `#[slot]` and `#[tmp]` arguments are trusted without being parsed, so they
    /// can't be lent this way: lend a copy made with [`Lent::new()`] instead. Local memory is lent
    /// with [`Lent::new()`], `Lent::<MaybeData<_>>::from_uninit()` or
    /// `Lent::<VariableElements<_>>::from_uninit()`.
    ///
    /// Code generated by `#[contract]` gives a method `Env` with a lifetime that covers the call of
    /// the method, so the method must declare its `#[output]` with that lifetime to lend it:
    /// ```ignore
    /// #[update]
    /// pub fn forward<'a>(
    ///     #[env] env: &mut Env<'a>,
    ///     #[output] mode: &'a mut Mode,
    /// ) -> Result<(), ContractError> {
    ///     let mut mode = env.lend_output(mode);
    ///     env.other_set_mode(MethodContext::Keep, OTHER_ADDRESS, &mut mode)?;
    ///     *mode.get_mut()? = Mode::B;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// With elided lifetimes (`&mut Env<'_>` and `&mut Mode`) compilation will fail with "lifetime
    /// may not live long enough", and rustc will suggest the signature above.
    #[inline(always)]
    pub fn lend_output<'b, T>(&self, value: &'b mut T) -> Lent<'b, T>
    where
        T: IoType,
        'b: 'a,
    {
        // SAFETY: `value` stays borrowed for at least `'a`. Memory that code using an `Env`
        // borrows for its lifetime is only accessed afterward by parsing it, unless its contents
        // accept every bit pattern, as `Self::with_executor_context()` requires (the host upholds
        // the same for an `Env` it gives to a guest).
        unsafe { Lent::from_mut_unchecked(value) }
    }

    /// Call a method at specified address and with specified arguments.
    ///
    /// This is a shortcut for [`PreparedMethod::new()`] + [`Self::call_prepared()`].
    ///
    /// A call can leave contents behind in outputs that are not valid values of their types, even
    /// when it succeeds, and it only initializes the return value, see [`ExternalArgs`].
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
        let prepared_method = PreparedMethod::new(contract, args, method_context);
        self.call_prepared(prepared_method)
    }

    /// Call prepared method.
    ///
    /// In most cases, this doesn't need to be called directly. Extension traits provide a more
    /// convenient way to make method calls and are enough in most cases.
    ///
    /// A call can leave contents behind in outputs that are not valid values of their types, even
    /// when it succeeds, and it only initializes the return value, see [`ExternalArgs`] and
    /// [`PreparedMethod::new_untyped()`].
    #[inline]
    pub fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError> {
        cfg_select! {
            feature = "executor" => self.executor_context.call(&self.state, &method),
            feature = "guest" => __ab_host_call(&method).into(),
            _ => {
                let _: PreparedMethod<'_> = method;
                Err(ContractError::InternalError)
            }
        }
    }
}
