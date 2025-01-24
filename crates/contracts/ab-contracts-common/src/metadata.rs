#[cfg(test)]
mod tests;

use ab_contracts_io_type::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY};
use core::ptr;

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
    /// * Encoding of the state type as described in [`IoTypeMetadataKind`]
    /// * Number of methods (u8)
    /// * Recursive metadata of methods as defined in one of:
    ///   * [`Self::Init`]
    ///   * [`Self::UpdateStateless`]
    ///   * [`Self::UpdateStatefulRo`]
    ///   * [`Self::UpdateStatefulRw`]
    ///   * [`Self::ViewStateless`]
    ///   * [`Self::ViewStatefulRo`]
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
    /// * Recursive metadata of argument's type as described in [`IoTypeMetadataKind`] with
    ///   following exceptions:
    ///   * This is skipped for [`Self::EnvRo`] and [`Self::EnvRw`]
    ///   * For [`Self::SlotWithAddressRo`] and [`Self::SlotWithAddressRw`] only slot's metadata is
    ///     included
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

impl ContractMetadataKind {
    // TODO: Implement `TryFrom` once it is available in const environment
    /// Try to create an instance from its `u8` representation
    pub const fn try_from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => Self::Contract,
            1 => Self::Trait,
            2 => Self::Init,
            3 => Self::UpdateStateless,
            4 => Self::UpdateStatefulRo,
            5 => Self::UpdateStatefulRw,
            6 => Self::ViewStateless,
            7 => Self::ViewStatefulRo,
            8 => Self::EnvRo,
            9 => Self::EnvRw,
            10 => Self::TmpRo,
            11 => Self::TmpRw,
            12 => Self::SlotWithAddressRo,
            13 => Self::SlotWithAddressRw,
            14 => Self::SlotWithoutAddressRo,
            15 => Self::SlotWithoutAddressRw,
            16 => Self::Input,
            17 => Self::Output,
            18 => Self::Result,
            _ => {
                return None;
            }
        })
    }

    /// Produce compact metadata.
    ///
    /// Compact metadata retains the shape, but throws some of the details. Specifically following
    /// transformations are applied to metadata (crucially, method names are retained!):
    /// * Struct, trait, enum and enum variant names are removed (replaced with 0 bytes names)
    /// * Structs and enum variants are turned into tuple variants (removing field names)
    /// * Method argument names are removed (removing argument names)
    ///
    /// This means that two methods with different argument names or struct field names, but the
    /// same shape otherwise are considered identical, allowing for limited future refactoring
    /// opportunities without changing compact metadata shape, which is important for
    /// [`MethodFingerprint`].
    ///
    /// [`MethodFingerprint`]: crate::methods::MethodFingerprint
    ///
    /// Returns `None` if input is invalid or too long.
    pub const fn compact(metadata: &[u8]) -> Option<([u8; MAX_METADATA_CAPACITY], usize)> {
        let mut metadata_scratch = [0; MAX_METADATA_CAPACITY];

        let Some((metadata, remainder)) = compact_metadata(metadata, &mut metadata_scratch) else {
            return None;
        };

        if !metadata.is_empty() {
            return None;
        }

        let remainder_len = remainder.len();
        let size = metadata_scratch.len() - remainder_len;
        Some((metadata_scratch, size))
    }
}

/// This macro is necessary to reduce boilerplate due to lack of `?` in const environment
macro_rules! forward_option {
    ($expr:expr) => {{
        let Some(result) = $expr else {
            return None;
        };
        result
    }};
}

const fn compact_metadata<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    let kind = forward_option!(ContractMetadataKind::try_from_u8(input[0]));
    (input, output) = forward_option!(copy_n_bytes(input, output, 1));

    if input.is_empty() || output.is_empty() {
        return None;
    }

    match kind {
        ContractMetadataKind::Contract => {
            // Compact contract state type
            (input, output) = forward_option!(IoTypeMetadataKind::compact(input, output));

            if input.is_empty() {
                return None;
            }

            let mut num_methods = input[0];
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));

            // Compact methods
            while num_methods > 0 {
                if input.is_empty() {
                    return None;
                }

                (input, output) = forward_option!(compact_metadata(input, output));

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Trait => {
            // Remove trait name
            let trait_name_length = input[0] as usize;
            output[0] = 0;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            input = forward_option!(skip_n_bytes(input, trait_name_length));

            if input.is_empty() {
                return None;
            }

            let mut num_methods = input[0];
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));

            // Compact methods
            while num_methods > 0 {
                if input.is_empty() {
                    return None;
                }

                (input, output) = forward_option!(compact_metadata(input, output));

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Init
        | ContractMetadataKind::UpdateStateless
        | ContractMetadataKind::UpdateStatefulRo
        | ContractMetadataKind::UpdateStatefulRw
        | ContractMetadataKind::ViewStateless
        | ContractMetadataKind::ViewStatefulRo => {
            // Copy method name
            let method_name_length = input[0] as usize;
            (input, output) = forward_option!(copy_n_bytes(input, output, 1 + method_name_length));

            if input.is_empty() {
                return None;
            }

            let mut num_arguments = input[0];
            (input, output) = forward_option!(copy_n_bytes(input, output, 1));

            // Compact arguments
            while num_arguments > 0 {
                if input.is_empty() {
                    return None;
                }

                (input, output) = forward_option!(compact_method_argument(input, output));

                num_arguments -= 1;
            }
        }
        ContractMetadataKind::EnvRo
        | ContractMetadataKind::EnvRw
        | ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotWithAddressRo
        | ContractMetadataKind::SlotWithAddressRw
        | ContractMetadataKind::SlotWithoutAddressRo
        | ContractMetadataKind::SlotWithoutAddressRw
        | ContractMetadataKind::Input
        | ContractMetadataKind::Output
        | ContractMetadataKind::Result => {
            // Can't start with argument
            return None;
        }
    }

    Some((input, output))
}

const fn compact_method_argument<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    let kind = forward_option!(ContractMetadataKind::try_from_u8(input[0]));
    (input, output) = forward_option!(copy_n_bytes(input, output, 1));

    match kind {
        ContractMetadataKind::Contract
        | ContractMetadataKind::Trait
        | ContractMetadataKind::Init
        | ContractMetadataKind::UpdateStateless
        | ContractMetadataKind::UpdateStatefulRo
        | ContractMetadataKind::UpdateStatefulRw
        | ContractMetadataKind::ViewStateless
        | ContractMetadataKind::ViewStatefulRo => {
            // Expected argument here
            return None;
        }
        ContractMetadataKind::EnvRo | ContractMetadataKind::EnvRw => {
            // Nothing else to do here, `#[env]` doesn't include metadata of its type
        }
        ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotWithAddressRo
        | ContractMetadataKind::SlotWithAddressRw
        | ContractMetadataKind::SlotWithoutAddressRo
        | ContractMetadataKind::SlotWithoutAddressRw
        | ContractMetadataKind::Input
        | ContractMetadataKind::Output
        | ContractMetadataKind::Result => {
            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Remove argument name
            let argument_name_length = input[0] as usize;
            output[0] = 0;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            input = forward_option!(skip_n_bytes(input, argument_name_length));

            // Compact argument type
            (input, output) = forward_option!(IoTypeMetadataKind::compact(input, output));
        }
    }

    Some((input, output))
}

/// Copies `n` bytes from input to output and returns both input and output after `n` bytes offset
const fn copy_n_bytes<'i, 'o>(
    input: &'i [u8],
    output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if n > input.len() || n > output.len() {
        return None;
    }

    let (source, input) = input.split_at(n);
    let (target, output) = output.split_at_mut(n);
    // TODO: Switch to `copy_from_slice` once stable:
    //  https://github.com/rust-lang/rust/issues/131415
    // The same as `target.copy_from_slice(&source);`, but it doesn't work in const environment
    // yet
    // SAFETY: Size is correct due to slicing above, pointers are created from valid independent
    // slices of equal length
    unsafe {
        ptr::copy_nonoverlapping(source.as_ptr(), target.as_mut_ptr(), source.len());
    }

    Some((input, output))
}

/// Skips `n` bytes and return remainder
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    if n > input.len() {
        return None;
    }

    // `&input[n..]` not supported in const yet
    Some(input.split_at(n).1)
}

/// Skips `n` bytes in input and output
const fn skip_n_bytes_io<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if n > input.len() || n > output.len() {
        return None;
    }

    // `&input[n..]` not supported in const yet
    input = input.split_at(n).1;
    // `&mut output[n..]` not supported in const yet
    output = output.split_at_mut(n).1;

    Some((input, output))
}
