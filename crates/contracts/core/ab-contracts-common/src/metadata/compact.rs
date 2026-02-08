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
    let contract_metadata_kind_input = *input.split_off_first()?;
    let contract_metadata_kind_output = output.split_off_first_mut()?;
    let contract_metadata_kind =
        ContractMetadataKind::try_from(contract_metadata_kind_input).ok()?;

    match contract_metadata_kind {
        ContractMetadataKind::Contract => {
            *contract_metadata_kind_output = contract_metadata_kind_input;

            // Compact contract state type
            (input, output) = IoTypeMetadataKind::compact(input, output)?;
            // Compact contract `#[slot]` type
            (input, output) = IoTypeMetadataKind::compact(input, output)?;
            // Compact contract `#[tmp]` type
            (input, output) = IoTypeMetadataKind::compact(input, output)?;

            let mut num_methods = *input.split_off_first()?;
            *output.split_off_first_mut()? = num_methods;

            // Compact methods
            while num_methods > 0 {
                (input, output) = compact_metadata_inner(input, output, for_external_args)?;

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Trait => {
            *contract_metadata_kind_output = contract_metadata_kind_input;

            // Remove trait name
            let trait_name_length = *input.split_off_first()?;
            *output.split_off_first_mut()? = 0;
            // TODO: `split_off()` is not `const fn` yet, even unstably
            input = input.get(usize::from(trait_name_length)..)?;

            let mut num_methods = *input.split_off_first()?;
            *output.split_off_first_mut()? = num_methods;

            // Compact methods
            while num_methods > 0 {
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
            *contract_metadata_kind_output = match contract_metadata_kind {
                ContractMetadataKind::Init => contract_metadata_kind_input,
                ContractMetadataKind::UpdateStateless
                | ContractMetadataKind::UpdateStatefulRo
                | ContractMetadataKind::UpdateStatefulRw => {
                    if for_external_args {
                        // For `ExternalArgs` the kind of `#[update]` doesn't matter
                        ContractMetadataKind::UpdateStateless as u8
                    } else {
                        contract_metadata_kind_input
                    }
                }
                ContractMetadataKind::ViewStateless | ContractMetadataKind::ViewStateful => {
                    if for_external_args {
                        // For `ExternalArgs` the kind of `#[view]` doesn't matter
                        ContractMetadataKind::ViewStateless as u8
                    } else {
                        contract_metadata_kind_input
                    }
                }
                _ => {
                    // Just matched above
                    unreachable!();
                }
            };

            // Copy method name
            let method_name_length = *input.first()?;
            (input, output) = copy_n_bytes(input, output, 1 + usize::from(method_name_length))?;

            let mut num_arguments = *input.split_off_first()?;
            *output.split_off_first_mut()? = num_arguments;

            // Compact arguments
            while num_arguments > 0 {
                num_arguments -= 1;

                (input, output) = compact_method_argument(
                    input,
                    output,
                    contract_metadata_kind,
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
        | ContractMetadataKind::Output
        | ContractMetadataKind::Return => {
            // Can't start with an argument
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
    let contract_metadata_kind_input = *input.split_off_first()?;
    let contract_metadata_kind =
        ContractMetadataKind::try_from(contract_metadata_kind_input).ok()?;

    match contract_metadata_kind {
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
            } else {
                let contract_metadata_kind_output = output.split_off_first_mut()?;
                *contract_metadata_kind_output = contract_metadata_kind_input;
            }
            // Nothing else to do here, `#[env]` doesn't include metadata of its type
        }
        ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw => {
            match contract_metadata_kind {
                ContractMetadataKind::TmpRo | ContractMetadataKind::TmpRw => {
                    if for_external_args {
                        // For `ExternalArgs` `#[tmp]` doesn't matter
                    } else {
                        let contract_metadata_kind_output = output.split_off_first_mut()?;
                        *contract_metadata_kind_output = contract_metadata_kind_input;
                    }
                }
                ContractMetadataKind::SlotRo | ContractMetadataKind::SlotRw => {
                    let contract_metadata_kind_output = output.split_off_first_mut()?;
                    *contract_metadata_kind_output = if for_external_args {
                        // For `ExternalArgs` the kind of `#[slot]` doesn't matter
                        ContractMetadataKind::SlotRo as u8
                    } else {
                        contract_metadata_kind_input
                    };
                }
                _ => {
                    // Just matched above
                    unreachable!()
                }
            }

            // Remove argument name
            let argument_name_length = *input.split_off_first()?;
            *output.split_off_first_mut()? = 0;
            // TODO: `split_off()` is not `const fn` yet, even unstably
            input = input.get(usize::from(argument_name_length)..)?;
        }
        ContractMetadataKind::Input
        | ContractMetadataKind::Output
        | ContractMetadataKind::Return => {
            let contract_metadata_kind_output = output.split_off_first_mut()?;
            *contract_metadata_kind_output = contract_metadata_kind_input;

            // Remove argument name
            let argument_name_length = *input.split_off_first()?;
            *output.split_off_first_mut()? = 0;
            // TODO: `split_off()` is not `const fn` yet, even unstably
            input = input.get(usize::from(argument_name_length)..)?;

            // May be skipped for `#[init]`, see `ContractMetadataKind::Init` for details
            let skip_argument_type = matches!(
                (method_kind, contract_metadata_kind, last_argument),
                (
                    ContractMetadataKind::Init,
                    ContractMetadataKind::Output | ContractMetadataKind::Return,
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
