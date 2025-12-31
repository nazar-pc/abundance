//! Macros for contracts
#![no_std]

#[doc(hidden)]
pub mod __private;

/// `#[contract]` macro to derive contract implementation.
///
/// This macro is supposed to be applied to an implementation of the struct that in turn
/// implements [`IoType`] trait. [`IoType`] is most commonly obtained by deriving
/// [`TrivialType`] ([`IoType`] is implemented for all types that implement [`TrivialType`]).
///
/// `#[contract]` macro will process *public* methods annotated with the following attributes:
/// * `#[init]` - method that can be called to produce an initial state of the contract, called
///   once during contacts lifetime
/// * `#[update]` - method that can read and/or modify state and/or slots of the contact, may
///   be called by user transaction directly or by another contract
/// * `#[view]` - method that can only read blockchain data, can read state or slots of the
///   contract, but can't modify their contents
///
/// Each argument (except `self`) of these methods has to be annotated with one of the
/// following attributes (must be in this order):
/// * `#[env]` - environment variable, used to access ephemeral execution environment, call
///   methods on other contracts, etc.
/// * `#[tmp]` - temporary ephemeral value to store auxiliary data while processing a
///   transaction
/// * `#[slot]` - slot corresponding to this contract
/// * `#[input]` - method input coming from user transaction or invocation from another
///   contract
/// * `#[output]` - method output, may serve as an alternative to returning values from a
///   function directly, useful to reduce stack usage
///
/// # For struct implementation
///
/// ## #\[init]
///
/// Initializer's purpose is to produce the initial state of the contract.
///
/// The following arguments are supported by this method (must be in this order):
/// * `#[env]` read-only and read-write
/// * `#[tmp]` read-only and read-write
/// * `#[slot]` read-only and read-write
/// * `#[input]`
/// * `#[output]`
///
/// `self` argument is not supported in any way in this context since the state of the contract
/// is just being created.
///
/// ## #\[update]
///
/// Generic method contract that can both update contract's own state and contents of slots.
///
/// The following arguments are supported by this method (must be in this order):
/// * `&self` or `&mut self` depending on whether state reads and/or modification are required
/// * `#[env]` read-only and read-write
/// * `#[tmp]` read-only and read-write
/// * `#[slot]` read-only and read-write
/// * `#[input]`
/// * `#[output]`
///
/// ## #\[view]
///
/// Similar to `#[update]`, but can only access read-only view of the state and slots, can be
/// called outside of the block context and can only call other `#[view]` methods.
///
/// The following arguments are supported by this method (must be in this order):
/// * `&self`
/// * `#[env]` read-only
/// * `#[slot]` read-only
/// * `#[input]`
/// * `#[output]`
///
/// # For trait definition and trait implementation
///
/// ## #\[update]
///
/// Generic method contract that can (in case of trait indirectly) both update contract's own
/// state and contents of slots.
///
/// The following arguments are supported by this method in trait context (must be in this
/// order):
/// * `#[env]` read-only and read-write
/// * `#[input]`
/// * `#[output]`
///
/// ## #\[view]
///
/// Similar to `#[update]`, but can only access (in case of trait indirectly) read-only view of
/// the state and slots, can be called outside of block context and can only call other
/// `#[view]` methods.
///
/// The following arguments are supported by this method in trait context (must be in this
/// order):
/// * `#[env]` read-only
/// * `#[input]`
/// * `#[output]`
///
/// # Generated code
///
/// This macro will produce several key outputs:
/// * [`Contract`] trait implementation (for struct implementation)
/// * FFI function for every method, which can be used by the host to call into the guest
///   environment (for struct and trait implementation)
/// * `InternalArgs` struct corresponding to each method, used as its sole input for FFI
///   function
/// * Struct implementing [`ExternalArgs`] trait for each method, usable by other contracts to
///   call into this contract through the host, host will interpret it based on metadata and
///   generate `InternalArgs` (for struct and trait implementation, trait definitions)
/// * Extension trait (for struct and trait implementation) that simplifies interaction with
///   host and removes the need to construct [`ExternalArgs`] manually, providing nice strongly
///   typed methods instead, implemented for [`Env`] struct (for struct and trait
///   implementation, trait definitions)
/// * Metadata as defined in [`ContractMetadataKind`] stored in the `ab-contract-metadata` link
///   section when compiled with `guest` feature enabled (for method, struct, and trait
///   implementation)
///
/// ## [`Contract`] trait implementation
///
/// [`Contract`] trait is required by a few other components in the system, so it is
/// automatically implemented by the macro, see trait details.
///
/// ## FFI function
///
/// Macro generates FFI function with C ABI that looks like this:
/// ```ignore
/// #[cfg_attr(feature = "guest", unsafe(no_mangle))]
/// pub unsafe extern "C" fn {prefix}_{method}(
///     args: NonNull<InternalArgs>,
/// ) -> ExitCode {
///     // ...
/// }
/// ```
///
/// Where `{prefix}` is derived from struct or trait name and `{method}` is the original method
/// name from struct or trait implementation.
///
/// Example with struct implementation:
/// ```ignore
/// // This
/// #[contract]
/// impl Example {
///     #[view]
///     pub fn hello() {}
/// }
///
/// // Will generate this
/// #[cfg_attr(feature = "guest", unsafe(no_mangle))]
/// pub unsafe extern "C" fn example_hello(
///     args: NonNull<InternalArgs>,
/// ) -> ExitCode {
///     // ...
/// }
/// ```
///
/// Example with trait implementation:
/// ```ignore
/// // This
/// #[contract]
/// impl Fungible for Token {
///     #[view]
///     pub fn balance(#[slot] address: &Address) -> Balance {}
/// }
///
/// // Will generate this
/// #[cfg_attr(feature = "guest", unsafe(no_mangle))]
/// pub unsafe extern "C" fn fungible_balance(
///     args: NonNull<InternalArgs>,
/// ) -> ExitCode {
///     // ...
/// }
/// ```
///
/// These generated functions are public and available in generated submodules, but there
/// should generally be no need to call them directly.
///
/// ## `InternalArgs` struct
///
/// `InternalArgs` is generated for each method and is used as input to the above FFI
/// functions. Its fields are generated based on function arguments, processing them in the
/// same order as in function signature. It is possible for the host to build this data
/// structure dynamically using available contact metadata.
///
/// All fields in the data structure are pointers, some are read-only, some can be written to
/// if changes need to be communicated back to the host. `read-only` here means that the host
/// will not read the value, even if the contract modifies it.
///
/// ### `&self`
///
/// `&self` is a read-only state of the contract and generates two fields, both of which are
/// read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     pub state_ptr: NonNull<<StructName as IoType>::PointerType>,
///     pub state_size: NonNull<u32>,
///     // ...
/// }
/// ```
///
/// This allows a contract to read the current state of the contract.
///
/// ### `&mut self`
///
/// `&mut self` is a read-write state of the contract and generates three fields, `state_ptr`
/// and `state_size` can be written to, while `state_capacity` is read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     pub state_ptr: NonNull<<StructName as IoType>::PointerType>,
///     pub state_size: *mut u32,
///     pub state_capacity: NonNull<u32>,
///     // ...
/// }
/// ```
///
/// This allows a contract to not only read, but also change the current state of the contract.
/// `state_capacity` is defined by both the type used and the size of the value used (whichever
/// is bigger in case of a variable-sized types) and corresponds to the amount of memory that
/// host allocated for the guest behind `state_ptr`. In the case of a variable-sized types,
/// guest can replace`state_ptr` with a pointer to a guest-allocated region of memory that host
/// must read updated value from. This is helpful in case increase of the value size beyond
/// allocated capacity is needed.
///
/// ### `#[env] env: &Env`
///
/// `#[env] env: &Env` is for accessing ephemeral environment with method calls restricted to
/// `#[view]`. Since this is a system-provided data structure with known layout, only read-only
/// pointer field is generated:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs<'internal_args> {
///     // ...
///     pub env_ptr: NonNull<Env<'internal_args>>,
///     // ...
/// }
/// ```
///
/// ### `#[env] env: &mut Env`
///
/// `#[env] env: &Env` is for accessing ephemeral environment without method calls
/// restrictions. Since this is a system-provided data structure with known layout, only
/// read-write pointer field is generated:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs<'internal_args> {
///     // ...
///     pub env_ptr: NonNull<Env<'internal_args>>,
///     // ...
/// }
/// ```
///
/// ### `#[tmp] tmp: &MaybeData<Tmp>`
///
/// `#[tmp] tmp: &MaybeData<Tmp>` is for accessing ephemeral value with auxiliary data and
/// generates two fields, both of which are read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     // ...
///     pub tmp_ptr: NonNull<
///         <
///             <StructName as Contract>::Tmp as IoType
///         >::PointerType,
///     >,
///     pub tmp_size: NonNull<u32>,
///     // ...
/// }
/// ```
///
/// This allows a contract to read the current ephemeral value of the contract.
///
/// ### `#[tmp] tmp: &mut MaybeData<Tmp>`
///
/// `#[tmp] tmp: &MaybeData<Tmp>` is for accessing ephemeral value with auxiliary data and
/// generates three fields, `tmp_ptr` and `tmp_size` can be written to, while `tmp_capacity` is
/// read-only: ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     // ...
///     pub tmp_ptr: NonNull<
///         <
///             <StructName as Contract>::Tmp as IoType
///         >::PointerType,
///     >,
///     pub tmp_size: *mut u32,
///     pub tmp_capacity: NonNull<u32>,
///     // ...
/// }
/// ```
/// 
/// This allows a contract to not only read, but also change the ephemeral value of the
/// contract. `tmp_capacity` is defined by both the type used and the size of the value used
/// (whichever is bigger in case of a variable-sized types) and corresponds to the amount of
/// memory that host allocated for the guest behind `tmp_ptr`. In the case of a variable-sized
/// types, guest can replace`tmp_ptr` with a pointer to a guest-allocated region of memory that
/// host must read updated value from. This is helpful in case increase of the value size
/// beyond allocated capacity is needed.
///
/// ### `#[slot] slot: &MaybeData<Slot>` and `#[slot] (address, slot): (&Address, &MaybeData<Slot>)`
///
/// `#[slot] slot: &MaybeData<Slot>` and its variant with explicit address argument are for
/// accessing slot data (that corresponds to optional `address` argument) and generates 3
/// fields, all of which are read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     // ...
///     pub slot_address_ptr: NonNull<Address>,
///     pub slot_ptr: NonNull<
///         <
///             <StructName as Contract>::Slot as IoType
///         >::PointerType,
///     >,
///     pub slot_size: NonNull<u32>,
///     // ...
/// }
/// ```
/// 
/// This allows a contract to read slot data.
///
/// ### `#[slot] slot: &mut MaybeData<Slot>` and `#[slot] (address, slot): (&Address, &mut MaybeData<Slot>)`
///
/// `#[slot] slot: &mut MaybeData<Slot>` and its variant with explicit address argument are for
/// accessing slot data (that corresponds to optional `address` argument) and generates 4
/// fields, `slot_ptr` and `slot_size` can be written to, while `slot_address_ptr` and
/// `slot_capacity` are read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     // ...
///     pub slot_address_ptr: NonNull<Address>,
///     pub slot_ptr: NonNull<
///         <
///             <StructName as Contract>::Slot as IoType
///         >::PointerType,
///     >,
///     pub slot_size: *mut u32,
///     pub slot_capacity: NonNull<u32>,
///     // ...
/// }
/// ```
/// 
/// This allows a contract to not only read, but also change slot data.
/// `slot_capacity` is defined by both the type used and the size of the value used (whichever
/// is bigger in case of a variable-sized types) and corresponds to the amount of memory that
/// host allocated for the guest behind `slot_ptr`. In the case of a variable-sized types,
/// guest can replace`slot_ptr` with a pointer to a guest-allocated region of memory that host
/// must read updated value from. This is helpful in case increase of the value size beyond
/// allocated capacity is needed.
///
/// Slot changes done by the method call will not be persisted if it returns an error.
///
/// ### `#[input] input: &InputValue`
///
/// `#[input] input: &InputValue` is a read-only input to the contract call and generates two
/// fields, both of which are read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     // ...
///     pub input_ptr: NonNull<<InputValue as IoType>::PointerType>,
///     pub input_size: NonNull<u32>,
///     // ...
/// }
/// ```
/// 
/// ### `#[output] output: &mut MaybeData<OutputValue>` and `-> ReturnValue`/`-> Result<ReturnValue, ContractError>`
///
/// `#[output] output: &mut MaybeData<OutputValue>` and regular return value is a read-write
/// output to the contract call and generates tree fields, `output_ptr` and `output_size` can
/// be written to, while `output_capacity` is read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct InternalArgs {
///     // ...
///     pub output_ptr: NonNull<<OutputValue as IoType>::PointerType>,
///     pub output_size: *mut u32,
///     pub output_capacity: NonNull<u32>,
///     // ...
/// }
/// ```
/// 
/// Initially output is initialized by the caller (typically empty), but contract can write
/// something useful there and written value will be propagated back to the caller to observe.
/// `output_ptr` pointer *must not be changed* as the host will not follow it to the new
/// address, the output size is fully constrained by capacity specified in `output_capacity`.
/// The only exception is the last `#[output]` of `#[init]` method (or `ReturnValue` if
/// present), which is the contract's initial state. In this case, its pointer can be changed
/// to point to a different data structure and not being limited by `result_capacity`
/// allocation from the host.
///
/// `#[output]` may be used as an alternative to `-> ReturnValue` and
/// `-> Result<ReturnValue, ContractError>` in case the data structure is large and allocation
/// on the stack is undesirable, which is especially helpful in case of a variable-sized
/// contract state.
///
/// *`output_size` might be a null pointer if the output type is [`TrivialType`]!*
///
/// NOTE: In case `ReturnValue` in `-> ReturnValue` or `-> Result<ReturnValue, ContractError>`
/// is `()`, it will be skipped in `InternalArgs`.
///
///
/// ## [`ExternalArgs`] implementation
///
/// Macro generates a struct that implements [`ExternalArgs`] for each method that other
/// contracts give to the host when they want to call into another (or even the same) contract.
///
/// Here is an example with struct implementation, but it works the same way with trait
/// definition and implementation too:
/// ```ignore
/// // This
/// #[contract]
/// impl Example {
///     #[view]
///     pub fn hello() {}
/// }
///
/// #[repr(C)]
/// pub struct ExampleHelloArgs {
///     // ...
/// }
///
/// #[automatically_derived]
/// unsafe impl ExternalArgs for ExampleHelloArgs {
///     // ...
/// }
///
/// impl ExternalArgs {
///     pub fn new(
///         // ...
///     ) -> Self {
///         // ...
///     }
/// }
/// ```
/// 
/// Struct name if generated by concatenating struct or trait name on which name was generated,
/// method name, and `Args` suffix, which is done to make it more convenient to use externally.
///
/// `&self`, `&mut self`, `#[env]` and `#[tmp]` arguments of the method are controlled fully by
/// the host and not present in `ExternalArgs`.
///
/// `ExternalArgs::new()` method is generated for convenient construction of the instance, though in
/// most cases [Extension trait] is used with a more convenient API.
///
/// [Extension trait]: #extension-trait
///
/// ### `#[slot]`
///
/// Each `#[slot]` argument in `ExternalArgs` is represented by a single read-only address
/// pointer: ```ignore
/// #[repr(C)]
/// pub struct ExternalArgs {
///     // ...
///     pub slot_ptr: NonNull<Address>,
///     // ...
/// }
/// ```
///
/// ### `#[input]`
///
/// Each `#[input]` argument in `ExternalArgs` is represented by two read-only fields, pointer
/// to data and its size:
/// ```ignore
/// #[repr(C)]
/// pub struct ExternalArgs {
///     // ...
///     pub input_ptr: NonNull<<InputValue as IoType>::PointerType>,
///     pub input_size: NonNull<u32>,
///     // ...
/// }
/// ```
///
/// ### `#[output]` and `-> ReturnValue`/`-> Result<ReturnValue, ContractError>`
///
/// Each `#[output]` argument in `ExternalArgs` is represented by three fields, `output_ptr`
/// and `output_size` can be written to, while `output_capacity` is read-only:
/// ```ignore
/// #[repr(C)]
/// pub struct ExternalArgs {
///     // ...
///     pub output_ptr: NonNull<<OutputValue as IoType>::PointerType>,
///     pub output_size: *mut u32,
///     pub output_capacity: NonNull<u32>,
///     // ...
/// }
/// ```
///
/// The arguments are skipped in `ExternalArgs` for the last `#[output]` or `ReturnValue` when
/// method is `#[init]` or when `ReturnValue` is `()` in other cases. For `#[init]` method's
/// return value is the contract's initial state and is processed by the execution environment
/// itself. When `ReturnValue` is `()` then there is no point in having a pointer for it.
///
/// The host will propagate the current value that `output_size` points to to the caller, so
/// that the callee can both read and write to it.
///
/// *`output_size` might be a null pointer if the output type is [`TrivialType`]!*
///
/// ## Extension trait
///
/// Extension trait is just a convenient wrapper, whose safe methods take strongly typed
/// arguments, construct `ExternalArgs` while respecting Rust safety invariants, and calls
/// [`Env::call()`] with it. Extension trait usage is not mandatory, but it does make method
/// calls much more convenient in most simple cases.
///
/// Generated methods reflect `ExternalArgs` fields with just context (except when calling
/// `#[view]` method where context is not applicable) and the address of the contract being
/// called added at the beginning:
/// ```ignore
/// // This
/// impl Token {
///     // ...
///
///     #[view]
///     pub fn balance(#[slot] target: &MaybeData<Slot>) -> Balance {
///         // ...
///     }
///
///     #[update]
///     pub fn transfer(
///         #[env] env: &mut Env<'_>,
///         #[slot] (from_address, from): (&Address, &mut MaybeData<Slot>),
///         #[slot] to: &mut MaybeData<Slot>,
///         #[input] &amount: &Balance,
///     ) -> Result<(), ContractError> {
///         // ...
///     }
/// }
///
/// // Will generate this
/// pub trait TokenExt {
///     fn balance(
///         &self,
///         contract: &Address,
///         target: &Address,
///     ) -> Result<Balance, ContractError>;
///
///     fn transfer(
///         self: &&mut Self,
///         method_context: &MethodContext,
///         contract: &Address,
///         from: &Address,
///         to: &Address,
///         amount: &Balance,
///     ) -> Result<(), ContractError>;
/// }
///
/// impl TokenExt for Env {
///     fn balance(
///         &self,
///         contract: &Address,
///         target: &Address,
///     ) -> Result<Balance, ContractError> {
///         // ...
///     }
///
///     fn transfer(
///         self: &&mut Self,
///         method_context: &MethodContext,
///         contract: &Address,
///         from: &Address,
///         to: &Address,
///         amount: &Balance,
///     ) -> Result<(), ContractError> {
///         // ...
///     }
/// }
/// ```
///
/// The name of the extension trait is created as struct or trait name followed by `Ext`
/// suffix.
///
/// ## Metadata
///
/// There are several places where metadata is being generated, see [`ContractMetadataKind`]
/// for details.
///
/// First, the `#[contract]` macro generates a public `METADATA` constant for each method
/// individually.
///
/// Second, for each trait that contract can implement `#[contract]` macro generates an
/// associated constant `METADATA` that essentially aggregates metadata of all annotated
/// methods.
///
/// Third, [`Contract`] trait implementation generated by `#[contract]` macro contains
/// `MAIN_CONTRACT_METADATA` associated constant, which is similar in nature to `METADATA`
/// constant for traits described above.
///
/// Lastly, for the whole contract as a project, both trait and contract metadata is
/// concatenated and stored in the `ab-contract-metadata` link section that can later be
/// inspected externally to understand everything about the contract's interfaces,
/// auto-generate UI, etc.
///
/// [`Contract`]: ab_contracts_common::Contract
/// [`TrivialType`]: ab_io_type::trivial_type::TrivialType
/// [`IoType`]: ab_io_type::IoType
/// [`ExternalArgs`]: ab_contracts_common::method::ExternalArgs
/// [`Env`]: ab_contracts_common::env::Env
/// [`Env::call()`]: ab_contracts_common::env::Env::call()
/// [`ContractMetadataKind`]: ab_contracts_common::metadata::ContractMetadataKind
pub use ab_contracts_macros_impl::contract;
