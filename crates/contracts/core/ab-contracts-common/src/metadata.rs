mod compact;
pub mod decode;
#[cfg(test)]
mod tests;

use crate::metadata::compact::compact_metadata;
use ab_contracts_io_type::metadata::MAX_METADATA_CAPACITY;

/// Metadata for smart contact methods.
///
/// Metadata encoding consists of this enum variant treated as `u8` followed by optional metadata
/// encoding rules specific to metadata type variant (see variant's description).
///
/// This metadata is enough to fully reconstruct the hierarchy of the type to generate language
/// bindings, auto-generate UI forms, etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ContractMetadataKind {
    /// Main contract metadata.
    ///
    /// Contracts are encoded af follows:
    /// * Encoding of the state type as described in [`IoTypeMetadataKind`]
    /// * Encoding of the `#[slot]` type as described in [`IoTypeMetadataKind`]
    /// * Encoding of the `#[tmp]` type as described in [`IoTypeMetadataKind`]
    /// * Number of methods (u8)
    /// * Recursive metadata of methods as defined in one of:
    ///   * [`Self::Init`]
    ///   * [`Self::UpdateStateless`]
    ///   * [`Self::UpdateStatefulRo`]
    ///   * [`Self::UpdateStatefulRw`]
    ///   * [`Self::ViewStateless`]
    ///   * [`Self::ViewStateful`]
    ///
    /// [`IoTypeMetadataKind`]: ab_contracts_io_type::metadata::IoTypeMetadataKind
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
    ///   * [`Self::SlotRo`]
    ///   * [`Self::SlotRw`]
    ///   * [`Self::Input`]
    ///   * [`Self::Output`]
    /// * Length of the argument name in bytes (u8, except for [`Self::EnvRo`] and [`Self::EnvRw`])
    /// * Argument name as UTF-8 bytes (except for [`Self::EnvRo`] and [`Self::EnvRw`])
    /// * Only for [`Self::Input`] and [`Self::Output`] recursive metadata of argument's type as
    ///   described in [`IoTypeMetadataKind`] with following exception:
    ///   * For last [`Self::Output`] this is skipped if method is [`Self::Init`] (since it is
    ///     statically known to be `Self`) and present otherwise
    ///
    /// [`IoTypeMetadataKind`]: ab_contracts_io_type::metadata::IoTypeMetadataKind
    ///
    /// NOTE: [`Self::Output`], regardless of whether it is a return type or explicit `#[output]`
    /// argument is encoded as a separate argument and counts towards number of arguments. At the
    /// same time, `self` doesn't count towards the number of arguments as it is implicitly defined
    /// by the variant of this struct.
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
    ViewStateful,

    // TODO: `#[env] can be made implicit assuming the name is of the struct is always the same
    /// Read-only `#[env]` argument.
    ///
    /// Example: `#[env] env: &Env,`
    EnvRo,
    /// Read-write `#[env]` argument.
    ///
    /// Example: `#[env] env: &mut Env<'_>,`
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
    SlotRo,
    /// Read-write `#[slot]` argument with an address.
    ///
    /// Example: `#[slot] (from_address, from): (&Address, &mut MaybeData<Slot>),`
    SlotRw,
    /// `#[input]` argument.
    ///
    /// Example: `#[input] balance: &Balance,`
    Input,
    /// Explicit `#[output]` argument or `T` of [`Result<T, ContractError>`] return type or simply
    /// return type if it is not fallible.
    ///
    /// NOTE: Skipped if return type's `T` is `()`.
    ///
    /// Example: `#[output] out: &mut VariableBytes<1024>,`
    Output,
}

impl ContractMetadataKind {
    // TODO: Implement `TryFrom` once it is available in const environment
    /// Try to create an instance from its `u8` representation
    #[inline(always)]
    pub const fn try_from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => Self::Contract,
            1 => Self::Trait,
            2 => Self::Init,
            3 => Self::UpdateStateless,
            4 => Self::UpdateStatefulRo,
            5 => Self::UpdateStatefulRw,
            6 => Self::ViewStateless,
            7 => Self::ViewStateful,
            8 => Self::EnvRo,
            9 => Self::EnvRw,
            10 => Self::TmpRo,
            11 => Self::TmpRw,
            12 => Self::SlotRo,
            13 => Self::SlotRw,
            14 => Self::Input,
            15 => Self::Output,
            _ => {
                return None;
            }
        })
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Produce compact metadata.
    ///
    /// Compact metadata retains the shape, but throws some details. Specifically following
    /// transformations are applied to metadata (crucially, method names are retained!):
    /// * Struct, trait, enum and enum variant names removed (replaced with 0 bytes names)
    /// * Structs and enum variants turned into tuple variants (removing field names)
    /// * Method argument names removed (removing argument names)
    ///
    /// This means that two methods with different argument names or struct field names, but the
    /// same shape otherwise are considered identical, allowing for limited future refactoring
    /// opportunities without changing compact metadata shape, which is important for
    /// [`MethodFingerprint`].
    ///
    /// [`MethodFingerprint`]: crate::method::MethodFingerprint
    ///
    /// Returns `None` if input is invalid or too long.
    pub const fn compact(metadata: &[u8]) -> Option<([u8; MAX_METADATA_CAPACITY], usize)> {
        compact_metadata(metadata, false)
    }

    // TODO: Create wrapper type for metadata bytes and move this method there
    /// Produce compact metadata for `ExternalArgs`.
    ///
    /// Similar to [`Self::compact()`] arguments that are not reflected in `ExternalArgs` will be
    /// skipped since they don't impact `ExternalArgs` API. This is used for `MethodFingerprint`
    /// derivation.
    ///
    /// Returns `None` if input is invalid or too long.
    pub const fn compact_external_args(
        metadata: &[u8],
    ) -> Option<([u8; MAX_METADATA_CAPACITY], usize)> {
        compact_metadata(metadata, true)
    }
}
