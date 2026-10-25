use crate::metadata::{ContractMetadataKind, MAX_METADATA_DEPTH};
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
    let contract_metadata_kind = ContractMetadataKind::try_from(*input.first()?).ok()?;

    match contract_metadata_kind {
        ContractMetadataKind::Contract => {
            *output.split_off_first_mut()? = *input.split_off_first()?;

            // Compact contract state type
            (input, output) = IoTypeMetadataKind::compact::<MAX_METADATA_DEPTH>(input, output)?;
            // Compact contract `#[slot]` type
            (input, output) = IoTypeMetadataKind::compact::<MAX_METADATA_DEPTH>(input, output)?;
            // Compact contract `#[tmp]` type
            (input, output) = IoTypeMetadataKind::compact::<MAX_METADATA_DEPTH>(input, output)?;

            let mut num_methods = *input.split_off_first()?;
            *output.split_off_first_mut()? = num_methods;

            // Compact methods
            while num_methods > 0 {
                (input, output) = compact_method(input, output, for_external_args)?;

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Trait => {
            *output.split_off_first_mut()? = *input.split_off_first()?;

            // Remove trait name
            let trait_name_length = *input.split_off_first()?;
            *output.split_off_first_mut()? = 0;
            // TODO: `split_off()` is not `const fn` yet, even unstably
            input = input.get(usize::from(trait_name_length)..)?;

            let mut num_methods = *input.split_off_first()?;
            *output.split_off_first_mut()? = num_methods;

            // Compact methods
            while num_methods > 0 {
                (input, output) = compact_method(input, output, for_external_args)?;

                num_methods -= 1;
            }
        }
        ContractMetadataKind::Init
        | ContractMetadataKind::UpdateStateless
        | ContractMetadataKind::UpdateStatefulRo
        | ContractMetadataKind::UpdateStatefulRw
        | ContractMetadataKind::ViewStateless
        | ContractMetadataKind::ViewStateful => {
            (input, output) = compact_method(input, output, for_external_args)?;
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

/// Compact a method, methods of contracts and traits are compacted with this function rather than
/// with [`compact_metadata_inner()`], so they can't be contracts or traits themselves
#[inline(always)]
const fn compact_method<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    for_external_args: bool,
) -> Option<(&'i [u8], &'o mut [u8])> {
    let contract_metadata_kind_input = *input.split_off_first()?;
    let contract_metadata_kind_output = output.split_off_first_mut()?;
    let contract_metadata_kind =
        ContractMetadataKind::try_from(contract_metadata_kind_input).ok()?;

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
        ContractMetadataKind::Contract
        | ContractMetadataKind::Trait
        | ContractMetadataKind::EnvRo
        | ContractMetadataKind::EnvRw
        | ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw
        | ContractMetadataKind::Input
        | ContractMetadataKind::Output
        | ContractMetadataKind::Return => {
            // Not a method
            return None;
        }
    };

    // Copy method name
    let method_name_length = *input.first()?;
    (input, output) = copy_n_bytes(input, output, 1 + usize::from(method_name_length))?;

    let mut num_arguments = *input.split_off_first()?;
    // Some arguments are skipped for `ExternalArgs`, so the number of arguments is written after
    // they are compacted
    let num_arguments_output = output.split_off_first_mut()?;
    let mut num_compact_arguments = 0;

    // Compact arguments
    while num_arguments > 0 {
        num_arguments -= 1;

        let kept;
        (input, output, kept) = compact_method_argument(
            input,
            output,
            contract_metadata_kind,
            num_arguments == 0,
            for_external_args,
        )?;
        if kept {
            num_compact_arguments += 1;
        }
    }

    *num_arguments_output = num_compact_arguments;

    Some((input, output))
}

/// Compact a method argument and return whether it was kept in the output.
///
/// `#[env]` and `#[tmp]` are not part of `ExternalArgs`, so they are skipped for it.
#[inline(always)]
const fn compact_method_argument<'i, 'o>(
    mut input: &'i [u8],
    mut output: &'o mut [u8],
    method_kind: ContractMetadataKind,
    last_argument: bool,
    for_external_args: bool,
) -> Option<(&'i [u8], &'o mut [u8], bool)> {
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
                return Some((input, output, false));
            }

            let contract_metadata_kind_output = output.split_off_first_mut()?;
            *contract_metadata_kind_output = contract_metadata_kind_input;
            // Nothing else to do here, `#[env]` doesn't include metadata of its type
        }
        ContractMetadataKind::TmpRo | ContractMetadataKind::TmpRw => {
            // Skip argument name
            let argument_name_length = *input.split_off_first()?;
            // TODO: `split_off()` is not `const fn` yet, even unstably
            input = input.get(usize::from(argument_name_length)..)?;

            if for_external_args {
                // For `ExternalArgs` `#[tmp]` doesn't matter
                return Some((input, output, false));
            }

            let contract_metadata_kind_output = output.split_off_first_mut()?;
            *contract_metadata_kind_output = contract_metadata_kind_input;
            // Remove argument name
            *output.split_off_first_mut()? = 0;
        }
        ContractMetadataKind::SlotRo | ContractMetadataKind::SlotRw => {
            let contract_metadata_kind_output = output.split_off_first_mut()?;
            *contract_metadata_kind_output = if for_external_args {
                // For `ExternalArgs` the kind of `#[slot]` doesn't matter
                ContractMetadataKind::SlotRo as u8
            } else {
                contract_metadata_kind_input
            };

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
                (input, output) = IoTypeMetadataKind::compact::<MAX_METADATA_DEPTH>(input, output)?;
            }
        }
    }

    Some((input, output, true))
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
