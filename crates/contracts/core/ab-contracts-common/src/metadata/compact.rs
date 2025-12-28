use crate::metadata::ContractMetadataKind;
use ab_io_type::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY};

#[inline(always)]
pub(super) const fn compact_metadata(
    metadata: &[u8],
    for_external_args: bool,
) -> Option<([u8; MAX_METADATA_CAPACITY], usize)> {
    let mut metadata_scratch = [0; MAX_METADATA_CAPACITY];

    let Some((metadata, remainder)) =
        compact_metadata_inner(metadata, &mut metadata_scratch, for_external_args)
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

#[inline(always)]
const fn compact_metadata_inner<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    for_external_args: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    let kind = ContractMetadataKind::try_from_u8(input[0])?;

    match kind {
        ContractMetadataKind::Contract => {
            (input, output) = copy_n_bytes(input, output, 1)?;

            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Compact contract state type
            (input, output) = IoTypeMetadataKind::compact(input, output)?;
            // Compact contract `#[slot]` type
            (input, output) = IoTypeMetadataKind::compact(input, output)?;
            // Compact contract `#[tmp]` type
            (input, output) = IoTypeMetadataKind::compact(input, output)?;

            if input.is_empty() {
                return None;
            }

            let mut num_methods = input[0];
            (input, output) = copy_n_bytes(input, output, 1)?;

            // Compact methods
            while num_methods > 0 {
                if input.is_empty() {
                    return None;
                }

                (input, output) = compact_metadata_inner(input, output, for_external_args)?;

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Trait => {
            (input, output) = copy_n_bytes(input, output, 1)?;

            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Remove trait name
            let trait_name_length = input[0] as usize;
            output[0] = 0;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            input = skip_n_bytes(input, trait_name_length)?;

            if input.is_empty() {
                return None;
            }

            let mut num_methods = input[0];
            (input, output) = copy_n_bytes(input, output, 1)?;

            // Compact methods
            while num_methods > 0 {
                if input.is_empty() {
                    return None;
                }

                (input, output) = compact_metadata_inner(input, output, for_external_args)?;

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Init
        | ContractMetadataKind::UpdateStateless
        | ContractMetadataKind::UpdateStatefulRo
        | ContractMetadataKind::UpdateStatefulRw
        | ContractMetadataKind::ViewStateless
        | ContractMetadataKind::ViewStateful => {
            match kind {
                ContractMetadataKind::Init => {
                    (input, output) = copy_n_bytes(input, output, 1)?;
                }
                ContractMetadataKind::UpdateStateless
                | ContractMetadataKind::UpdateStatefulRo
                | ContractMetadataKind::UpdateStatefulRw => {
                    if for_external_args {
                        // For `ExternalArgs` the kind of `#[update]` doesn't matter
                        output[0] = ContractMetadataKind::UpdateStateless as u8;
                        (input, output) = skip_n_bytes_io(input, output, 1)?;
                    } else {
                        (input, output) = copy_n_bytes(input, output, 1)?;
                    }
                }
                ContractMetadataKind::ViewStateless | ContractMetadataKind::ViewStateful => {
                    if for_external_args {
                        // For `ExternalArgs` the kind of `#[view]` doesn't matter
                        output[0] = ContractMetadataKind::ViewStateless as u8;
                        (input, output) = skip_n_bytes_io(input, output, 1)?;
                    } else {
                        (input, output) = copy_n_bytes(input, output, 1)?;
                    }
                }
                _ => {
                    // Just matched above
                    unreachable!();
                }
            }

            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Copy method name
            let method_name_length = input[0] as usize;
            (input, output) = copy_n_bytes(input, output, 1 + method_name_length)?;

            if input.is_empty() {
                return None;
            }

            let mut num_arguments = input[0];
            (input, output) = copy_n_bytes(input, output, 1)?;

            // Compact arguments
            while num_arguments > 0 {
                if input.is_empty() {
                    return None;
                }

                num_arguments -= 1;

                (input, output) = compact_method_argument(
                    input,
                    output,
                    kind,
                    num_arguments == 0,
                    for_external_args,
                )?;
            }
        }
        ContractMetadataKind::EnvRo
        | ContractMetadataKind::EnvRw
        | ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw
        | ContractMetadataKind::Input
        | ContractMetadataKind::Output => {
            // Can't start with argument
            return None;
        }
    }

    Some((input, output))
}

#[inline(always)]
const fn compact_method_argument<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    method_kind: ContractMetadataKind,
    last_argument: bool,
    for_external_args: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    if input.is_empty() || output.is_empty() {
        return None;
    }

    let kind = ContractMetadataKind::try_from_u8(input[0])?;

    match kind {
        ContractMetadataKind::Contract
        | ContractMetadataKind::Trait
        | ContractMetadataKind::Init
        | ContractMetadataKind::UpdateStateless
        | ContractMetadataKind::UpdateStatefulRo
        | ContractMetadataKind::UpdateStatefulRw
        | ContractMetadataKind::ViewStateless
        | ContractMetadataKind::ViewStateful => {
            // Expected argument here
            return None;
        }
        ContractMetadataKind::EnvRo | ContractMetadataKind::EnvRw => {
            if for_external_args {
                // For `ExternalArgs` `#[env]` doesn't matter
                input = skip_n_bytes(input, 1)?;
            } else {
                (input, output) = copy_n_bytes(input, output, 1)?;
            }
            // Nothing else to do here, `#[env]` doesn't include metadata of its type
        }
        ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw => {
            match kind {
                ContractMetadataKind::TmpRo | ContractMetadataKind::TmpRw => {
                    if for_external_args {
                        // For `ExternalArgs` `#[tmp]` doesn't matter
                        input = skip_n_bytes(input, 1)?;
                    } else {
                        (input, output) = copy_n_bytes(input, output, 1)?;
                    }
                }
                ContractMetadataKind::SlotRo | ContractMetadataKind::SlotRw => {
                    if for_external_args {
                        // For `ExternalArgs` the kind of `#[slot]` doesn't matter
                        output[0] = ContractMetadataKind::SlotRo as u8;
                        (input, output) = skip_n_bytes_io(input, output, 1)?;
                    } else {
                        (input, output) = copy_n_bytes(input, output, 1)?;
                    }
                }
                _ => {
                    // Just matched above
                    unreachable!()
                }
            }

            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Remove argument name
            let argument_name_length = input[0] as usize;
            output[0] = 0;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            input = skip_n_bytes(input, argument_name_length)?;
        }
        ContractMetadataKind::Input | ContractMetadataKind::Output => {
            (input, output) = copy_n_bytes(input, output, 1)?;

            if input.is_empty() || output.is_empty() {
                return None;
            }

            // Remove argument name
            let argument_name_length = input[0] as usize;
            output[0] = 0;
            (input, output) = skip_n_bytes_io(input, output, 1)?;
            input = skip_n_bytes(input, argument_name_length)?;

            // May be skipped for `#[init]`, see `ContractMetadataKind::Init` for details
            let skip_argument_type = matches!(
                (method_kind, kind, last_argument),
                (
                    ContractMetadataKind::Init,
                    ContractMetadataKind::Output,
                    true
                )
            );
            if !skip_argument_type {
                // Compact argument type
                (input, output) = IoTypeMetadataKind::compact(input, output)?;
            }
        }
    }

    Some((input, output))
}

/// Copies `n` bytes from input to output and returns both input and output after `n` bytes offset
#[inline(always)]
const fn copy_n_bytes<'i, 'o>(
    input: &'i [u8],
    output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    let (source, input) = input.split_at_checked(n)?;
    let (target, output) = output.split_at_mut_checked(n)?;

    target.copy_from_slice(source);

    Some((input, output))
}

/// Skips `n` bytes and return remainder
#[inline(always)]
const fn skip_n_bytes(input: &[u8], n: usize) -> Option<&[u8]> {
    input.get(n..)
}

/// Skips `n` bytes in input and output
#[inline(always)]
const fn skip_n_bytes_io<'i, 'o>(
    input: &'i [u8],
    output: &'o mut [u8],
    n: usize,
) -> Option<(&'i [u8], &'o mut [u8])> {
    let input = input.get(n..)?;
    let output = output.get_mut(n..)?;

    Some((input, output))
}
