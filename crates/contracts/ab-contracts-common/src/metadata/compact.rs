use crate::metadata::ContractMetadataKind;
use ab_contracts_io_type::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY};
use core::ptr;

pub(super) const fn compact_metadata(
    metadata: &[u8],
) -> Option<([u8; MAX_METADATA_CAPACITY], usize)> {
    let mut metadata_scratch = [0; MAX_METADATA_CAPACITY];

    let Some((metadata, remainder)) = compact_metadata_inner(metadata, &mut metadata_scratch)
    else {
        return None;
    };

    if !metadata.is_empty() {
        return None;
    }

    let remainder_len = remainder.len();
    let size = metadata_scratch.len() - remainder_len;
    Some((metadata_scratch, size))
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

const fn compact_metadata_inner<'i, 'o>(
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
            // Compact contract `#[slot]` type
            (input, output) = forward_option!(IoTypeMetadataKind::compact(input, output));
            // Compact contract `#[tmp]` type
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

                (input, output) = forward_option!(compact_metadata_inner(input, output));

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

                (input, output) = forward_option!(compact_metadata_inner(input, output));

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

                (input, output) = forward_option!(compact_method_argument(input, output, kind));

                num_arguments -= 1;
            }
        }
        ContractMetadataKind::EnvRo
        | ContractMetadataKind::EnvRw
        | ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw
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
    method_kind: ContractMetadataKind,
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
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw => {
            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Remove argument name
            let argument_name_length = input[0] as usize;
            output[0] = 0;
            (input, output) = forward_option!(skip_n_bytes_io(input, output, 1));
            input = forward_option!(skip_n_bytes(input, argument_name_length));
        }
        ContractMetadataKind::Input
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

            if !matches!(
                (method_kind, kind),
                (ContractMetadataKind::Init, ContractMetadataKind::Result)
            ) {
                // Compact argument type
                (input, output) = forward_option!(IoTypeMetadataKind::compact(input, output));
            }
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
