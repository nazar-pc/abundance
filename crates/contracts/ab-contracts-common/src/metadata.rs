/// Metadata for smart contact methods.
///
/// Metadata encoding consists of this enum variant treated as `u8` followed by optional metadata
/// encoding rules specific to metadata type variant (see variant's description).
///
/// This metadata is sufficient to fully reconstruct hierarchy of the type in order to generate
/// language bindings, auto-generate UI forms, etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ContractMetadataKind {
    /// Main contract metadata.
    ///
    /// Contracts are encoded af follows:
    /// * Encoding of the state struct as described in [`IoTypeMetadataKind`]
    /// * Number of methods (u8)
    /// * Recursive metadata of methods as defined in one of:
    ///   * [`Self::Init`]
    ///   * [`Self::UpdateStateless`]
    ///   * [`Self::UpdateStatefulRo`]
    ///   * [`Self::UpdateStatefulRw`]
    ///   * [`Self::ViewStateless`]
    ///   * [`Self::ViewStatefulRo`]
    ///
    /// [`IoTypeMetadataKind`]: ab_contracts_io_type::IoTypeMetadataKind
    Contract,
    /// Trait metadata.
    ///
    /// Traits are encoded af follows:
    /// * Length of trait name in bytes (u8)
    /// * Trait name as UTF-8 bytes
    /// * Number of methods (u8)
    /// * Recursive metadata of methods as defined in one of:
    ///   * [`Self::UpdateStateless`]
    ///   * [`Self::ViewStateless`]
    Trait,

    /// `#[init]` method.
    ///
    /// Initializers are encoded af follows:
    /// * Length of method name in bytes (u8)
    /// * Method name as UTF-8 bytes
    /// * Number of named arguments (u8, excluding state argument `&self` or `&mut self`)
    ///
    /// Each argument is encoded as follows:
    /// * Argument type as u8, one of:
    ///   * [`Self::EnvRo`]
    ///   * [`Self::EnvRw`]
    ///   * [`Self::TmpRo`]
    ///   * [`Self::TmpRw`]
    ///   * [`Self::SlotWithAddressRo`]
    ///   * [`Self::SlotWithAddressRw`]
    ///   * [`Self::SlotWithoutAddressRo`]
    ///   * [`Self::SlotWithoutAddressRw`]
    ///   * [`Self::Input`]
    ///   * [`Self::Output`]
    ///   * [`Self::Result`]
    /// * Length of the argument name in bytes (u8)
    /// * Argument name as UTF-8 bytes
    /// * Recursive metadata of argument's type as described in [`IoTypeMetadataKind`]
    ///
    /// [`IoTypeMetadataKind`]: ab_contracts_io_type::IoTypeMetadataKind
    ///
    /// NOTE: Result, regardless of whether it is a return type or explicit `#[result]` argument, is
    /// encoded as a separate argument and counts towards number of arguments. At the same time
    /// `self` doesn't count towards the number of arguments as it is implicitly defined by the
    /// variant of this struct.
    Init,
    /// Stateless `#[update]` method (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    UpdateStateless,
    /// Stateful read-only `#[update]` method (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    UpdateStatefulRo,
    /// Stateful read-write `#[update]` method (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    UpdateStatefulRw,
    /// Stateless `#[view]` method (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    ViewStateless,
    /// Stateful read-only `#[view]` method (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    ViewStatefulRo,

    // TODO: `#[env] can be made implicit assuming the name is of the struct is always the same
    /// Read-only `#[env]` argument.
    ///
    /// Example: `#[env] env: &Env,`
    EnvRo,
    /// Read-write `#[env]` argument.
    ///
    /// Example: `#[env] env: &mut Env,`
    EnvRw,
    /// Read-only `#[tmp]` argument.
    ///
    /// Example: `#[tmp] tmp: &MaybeData<Tmp>,`
    TmpRo,
    /// Read-write `#[tmp]` argument.
    ///
    /// Example: `#[tmp] tmp: &mut MaybeData<Tmp>,`
    TmpRw,
    // TODO: What if address is mandatory for slots? Then it would be possible to make `#[slot]`
    //  implicit
    /// Read-only `#[slot]` argument with an address.
    ///
    /// Example: `#[slot] (from_address, from): (&Address, &MaybeData<Slot>),`
    SlotWithAddressRo,
    /// Read-write `#[slot]` argument with an address.
    ///
    /// Example: `#[slot] (from_address, from): (&Address, &mut MaybeData<Slot>),`
    SlotWithAddressRw,
    /// Read-only `#[slot]` argument without an address.
    ///
    /// Example: `#[slot] from: &MaybeData<Slot>,`
    SlotWithoutAddressRo,
    /// Read-write `#[slot]` argument without an address.
    ///
    /// Example: `#[slot] from: &mut MaybeData<Slot>,`
    SlotWithoutAddressRw,
    /// `#[input]` argument.
    ///
    /// Example: `#[input] balance: &Balance,`
    Input,
    /// `#[output]` argument.
    ///
    /// Example: `#[output] out: &mut VariableBytes<1024>,`
    Output,
    // TODO: Is explicit result needed? If not then `#[input]` and `#[output]` can be made implicit
    /// Explicit `#[result`] argument or `T` of [`Result<T, ContractError>`] return type or simply
    /// return type if it is not fallible.
    ///
    /// Example: `#[result] result: &mut MaybeData<Balance>,`
    ///
    /// NOTE: There is always exactly one result in a method.
    Result,
}
