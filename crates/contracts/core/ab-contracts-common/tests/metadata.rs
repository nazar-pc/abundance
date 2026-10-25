//! Tests of contract metadata and of metadata of types used by contracts

use ab_contracts_common::MAX_TOTAL_METHOD_ARGS;
use ab_contracts_common::env::{EnvState, MethodContext};
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MetadataDecoder, MetadataDecodingError, MethodKind, MethodMetadataDecoder,
    MethodsContainerKind,
};
use ab_contracts_common::metadata::{ContractMetadataKind, MAX_METADATA_DEPTH};
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_core_primitives::balance::Balance;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_io_type::alignment::Alignment;
use ab_io_type::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY};
use ab_io_type::trivial_type::TrivialType;

/// Compact metadata of `T`
fn compact_type<T>() -> Vec<u8>
where
    T: TrivialType,
{
    let mut compact = [0; MAX_METADATA_CAPACITY];
    let (remainder, compact_remainder) =
        IoTypeMetadataKind::compact::<MAX_METADATA_DEPTH>(T::METADATA, &mut compact).unwrap();
    assert!(remainder.is_empty());
    let compact_length = MAX_METADATA_CAPACITY - compact_remainder.len();
    compact[..compact_length].to_vec()
}

/// Check that compact metadata of `T` decodes into the size and alignment of `T`
#[track_caller]
fn check_compact_metadata<T>()
where
    T: TrivialType,
{
    let compact = compact_type::<T>();
    let (type_details, remainder) =
        IoTypeMetadataKind::type_details::<MAX_METADATA_DEPTH>(&compact).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(
        usize::try_from(type_details.recommended_capacity).unwrap(),
        size_of::<T>()
    );
    assert_eq!(type_details.alignment, Alignment::of::<T>());
}

#[test]
fn compact_metadata_decodes() {
    check_compact_metadata::<Address>();
    check_compact_metadata::<Balance>();
    check_compact_metadata::<BlockNumber>();
    check_compact_metadata::<BlockRoot>();
    check_compact_metadata::<EnvState>();
    check_compact_metadata::<Gas>();
    check_compact_metadata::<MethodContext>();
    check_compact_metadata::<MethodFingerprint>();
    check_compact_metadata::<ShardIndex>();
    check_compact_metadata::<TransactionHeader>();
    check_compact_metadata::<TransactionSlot>();
}

#[test]
fn newtypes_compact_into_wrapped_types() {
    // Structs with a single field compact into the type of the field, recursively
    assert_eq!(compact_type::<BlockNumber>(), compact_type::<u64>());
    assert_eq!(compact_type::<Gas>(), compact_type::<u64>());
    assert_eq!(compact_type::<ShardIndex>(), compact_type::<u32>());
    assert_eq!(compact_type::<BlockRoot>(), compact_type::<[u8; 32]>());
    assert_eq!(
        compact_type::<MethodFingerprint>(),
        compact_type::<[u8; 32]>()
    );
    assert_eq!(
        compact_type::<[BlockNumber; 2]>(),
        compact_type::<[u64; 2]>()
    );

    // Structs with more fields are retained, so `Address` (two `u64`) differs from `u128`, but has
    // the same compact metadata as `Balance`
    assert_eq!(compact_type::<Address>(), compact_type::<Balance>());
    assert_ne!(compact_type::<Address>(), compact_type::<u128>());
}

/// Metadata of a method of `kind` named `method` with a single `#[input] value: u8` argument and
/// its compact metadata with `compact_kind`
fn method_metadata(
    kind: ContractMetadataKind,
    compact_kind: ContractMetadataKind,
) -> (Vec<u8>, Vec<u8>) {
    let metadata = [
        &[kind as u8, 6][..],
        b"method",
        &[1, ContractMetadataKind::Input as u8, 5],
        b"value",
        &[IoTypeMetadataKind::U8 as u8],
    ]
    .concat();
    let compact = [
        &[compact_kind as u8, 6][..],
        b"method",
        &[1, ContractMetadataKind::Input as u8, 0],
        &[IoTypeMetadataKind::U8 as u8],
    ]
    .concat();

    (metadata, compact)
}

/// Metadata of a contract with the given state type, `()` `#[slot]` and `#[tmp]` types and a
/// single method
fn contract_with_method(state: &[u8], method: &[u8]) -> Vec<u8> {
    let unit = IoTypeMetadataKind::Unit as u8;
    [
        &[ContractMetadataKind::Contract as u8][..],
        state,
        &[unit, unit, 1],
        method,
    ]
    .concat()
}

/// Metadata of a trait with a zero-length name and a single method
fn trait_with_method(method: &[u8]) -> Vec<u8> {
    [&[ContractMetadataKind::Trait as u8, 0, 1][..], method].concat()
}

/// Compact contract metadata, see [`ContractMetadataKind::compact()`]
fn compact(metadata: &[u8]) -> Option<Vec<u8>> {
    let (compact, size) = ContractMetadataKind::compact(metadata)?;
    Some(compact[..size].to_vec())
}

/// Compact contract metadata for `ExternalArgs`, see
/// [`ContractMetadataKind::compact_external_args()`]
fn compact_external_args(metadata: &[u8]) -> Option<Vec<u8>> {
    let (compact, size) = ContractMetadataKind::compact_external_args(metadata)?;
    Some(compact[..size].to_vec())
}

#[test]
fn contracts_and_traits_compact() {
    let (update, update_compact) = method_metadata(
        ContractMetadataKind::UpdateStatefulRw,
        ContractMetadataKind::UpdateStatefulRw,
    );
    let (_, update_external_args) = method_metadata(
        ContractMetadataKind::UpdateStatefulRw,
        ContractMetadataKind::UpdateStateless,
    );
    let (view, view_compact) = method_metadata(
        ContractMetadataKind::ViewStateless,
        ContractMetadataKind::ViewStateless,
    );

    // State, `#[slot]` and `#[tmp]` types are `()`
    let unit = IoTypeMetadataKind::Unit as u8;
    let contract = [
        &[ContractMetadataKind::Contract as u8, unit, unit, unit, 2][..],
        &update,
        &view,
    ]
    .concat();
    assert_eq!(
        compact(&contract).unwrap(),
        [
            &[ContractMetadataKind::Contract as u8, unit, unit, unit, 2][..],
            &update_compact,
            &view_compact,
        ]
        .concat()
    );
    assert_eq!(
        compact_external_args(&contract).unwrap(),
        [
            &[ContractMetadataKind::Contract as u8, unit, unit, unit, 2][..],
            &update_external_args,
            &view_compact,
        ]
        .concat()
    );

    let r#trait = [
        &[ContractMetadataKind::Trait as u8, 5][..],
        b"Trait",
        &[1],
        &view,
    ]
    .concat();
    assert_eq!(
        compact(&r#trait).unwrap(),
        [
            &[ContractMetadataKind::Trait as u8, 0, 1][..],
            &view_compact
        ]
        .concat()
    );

    // A single method
    assert_eq!(compact(&update).unwrap(), update_compact);
    assert_eq!(
        compact_external_args(&update).unwrap(),
        update_external_args
    );
}

#[test]
fn methods_must_not_be_contracts_or_traits() {
    let (view, _) = method_metadata(
        ContractMetadataKind::ViewStateless,
        ContractMetadataKind::ViewStateless,
    );
    let unit = [IoTypeMetadataKind::Unit as u8];

    for nested in [
        contract_with_method(&unit, &contract_with_method(&unit, &view)),
        contract_with_method(&unit, &trait_with_method(&view)),
        trait_with_method(&trait_with_method(&view)),
        trait_with_method(&contract_with_method(&unit, &view)),
    ] {
        assert!(compact(&nested).is_none());
        assert!(compact_external_args(&nested).is_none());
    }
}

/// Metadata of `depth` `Unaligned` nested in each other around `u8`
fn nested_unaligned(depth: usize) -> Vec<u8> {
    let mut metadata = vec![IoTypeMetadataKind::Unaligned as u8; depth];
    metadata.push(IoTypeMetadataKind::U8 as u8);
    metadata
}

/// Metadata of a method with `#[input] value` of a type nested `depth` levels deep
fn method_with_nested_input(depth: usize) -> Vec<u8> {
    [
        &[ContractMetadataKind::ViewStateless as u8, 6][..],
        b"method",
        &[1, ContractMetadataKind::Input as u8, 5],
        b"value",
        &nested_unaligned(depth),
    ]
    .concat()
}

/// Decode the first argument of the first method of contract metadata with [`MetadataDecoder`]
fn decode_first_argument(metadata: &[u8]) -> Result<(), MetadataDecodingError<'_>> {
    let mut decoder = MetadataDecoder::new(metadata);
    let item = decoder.decode_next().unwrap()?;
    let mut methods_decoder = item.into_decoder();
    let (mut arguments_decoder, _method) = methods_decoder.decode_next().unwrap().decode_next()?;
    arguments_decoder.decode_next().unwrap().map(|_argument| ())
}

#[test]
fn io_types_nested_too_deep_are_rejected() {
    let method_at_limit = method_with_nested_input(MAX_METADATA_DEPTH);
    let method_too_deep = method_with_nested_input(MAX_METADATA_DEPTH + 1);
    assert!(compact(&method_at_limit).is_some());
    assert!(compact_external_args(&method_at_limit).is_some());
    assert!(MethodFingerprint::new(&method_at_limit).is_some());
    assert!(compact(&method_too_deep).is_none());
    assert!(compact_external_args(&method_too_deep).is_none());
    assert!(MethodFingerprint::new(&method_too_deep).is_none());

    let state_at_limit = nested_unaligned(MAX_METADATA_DEPTH);
    let state_too_deep = nested_unaligned(MAX_METADATA_DEPTH + 1);
    let contract_at_limit = contract_with_method(&state_at_limit, &method_at_limit);
    assert!(compact(&contract_at_limit).is_some());
    assert!(compact(&contract_with_method(&state_too_deep, &method_at_limit)).is_none());
    assert!(compact(&contract_with_method(&state_at_limit, &method_too_deep)).is_none());

    decode_first_argument(&contract_at_limit).unwrap();
    assert!(matches!(
        decode_first_argument(&contract_with_method(&state_too_deep, &method_at_limit)),
        Err(MetadataDecodingError::InvalidStateIoType)
    ));
    assert!(matches!(
        decode_first_argument(&contract_with_method(&state_at_limit, &method_too_deep)),
        Err(MetadataDecodingError::InvalidArgumentIoType {
            argument_name: b"value",
            argument_kind: ArgumentKind::Input,
        })
    ));
}

/// Kinds of all arguments, in the order that `#[contract]` generates them in
const ARGUMENT_KINDS: [ContractMetadataKind; 9] = [
    ContractMetadataKind::EnvRo,
    ContractMetadataKind::EnvRw,
    ContractMetadataKind::TmpRo,
    ContractMetadataKind::TmpRw,
    ContractMetadataKind::SlotRo,
    ContractMetadataKind::SlotRw,
    ContractMetadataKind::Input,
    ContractMetadataKind::Output,
    ContractMetadataKind::Return,
];

/// Position of an argument of `kind` in the order that `#[contract]` generates and whether more
/// than one argument can have it
fn argument_position(kind: ContractMetadataKind) -> (u8, bool) {
    match kind {
        ContractMetadataKind::EnvRo | ContractMetadataKind::EnvRw => (0, false),
        ContractMetadataKind::TmpRo | ContractMetadataKind::TmpRw => (1, false),
        ContractMetadataKind::SlotRo | ContractMetadataKind::SlotRw => (2, true),
        ContractMetadataKind::Input => (3, true),
        ContractMetadataKind::Output => (4, true),
        ContractMetadataKind::Return => (5, false),
        kind => unreachable!("Not an argument: {kind:?}"),
    }
}

/// Metadata of an argument of `kind`, of type `u8` if it has a type
fn argument_metadata(kind: ContractMetadataKind) -> Vec<u8> {
    let u8_type = IoTypeMetadataKind::U8 as u8;
    match kind {
        ContractMetadataKind::EnvRo | ContractMetadataKind::EnvRw => vec![kind as u8],
        ContractMetadataKind::TmpRo
        | ContractMetadataKind::TmpRw
        | ContractMetadataKind::SlotRo
        | ContractMetadataKind::SlotRw => [&[kind as u8, 4][..], b"name"].concat(),
        ContractMetadataKind::Input | ContractMetadataKind::Output => {
            [&[kind as u8, 4][..], b"name", &[u8_type]].concat()
        }
        ContractMetadataKind::Return => vec![kind as u8, 0, u8_type],
        kind => unreachable!("Not an argument: {kind:?}"),
    }
}

/// Metadata of a method of `kind` with arguments of `argument_kinds`
fn method_with_arguments(
    kind: ContractMetadataKind,
    argument_kinds: &[ContractMetadataKind],
) -> Vec<u8> {
    let mut metadata = vec![kind as u8, 6];
    metadata.extend_from_slice(b"method");
    metadata.push(u8::try_from(argument_kinds.len()).unwrap());
    for &argument_kind in argument_kinds {
        metadata.extend_from_slice(&argument_metadata(argument_kind));
    }
    metadata
}

/// Decode all arguments of method metadata in a container of `container_kind`, an error comes with
/// the index of its argument
fn decode_arguments(
    mut metadata: &[u8],
    container_kind: MethodsContainerKind,
) -> Result<(), (usize, MetadataDecodingError<'_>)> {
    let (mut arguments_decoder, _method) =
        MethodMetadataDecoder::new(&mut metadata, container_kind)
            .decode_next()
            .unwrap();
    let mut index = 0;
    while let Some(argument) = arguments_decoder.decode_next() {
        argument.map_err(|error| (index, error))?;
        index += 1;
    }
    Ok(())
}

#[test]
fn arguments_in_order_are_accepted() {
    // Every subset of these arguments is in order, including several `#[slot]`s, `#[input]`s and
    // `#[output]`s
    let in_order = [
        ContractMetadataKind::EnvRw,
        ContractMetadataKind::TmpRw,
        ContractMetadataKind::SlotRw,
        ContractMetadataKind::SlotRo,
        ContractMetadataKind::Input,
        ContractMetadataKind::Input,
        ContractMetadataKind::Output,
        ContractMetadataKind::Output,
        ContractMetadataKind::Return,
    ];
    for subset in 0..1u32 << in_order.len() {
        let argument_kinds = in_order
            .iter()
            .enumerate()
            .filter(|&(index, _kind)| subset & (1 << index) != 0)
            .map(|(_index, &kind)| kind)
            .collect::<Vec<_>>();
        // `&mut self` counts towards `MAX_TOTAL_METHOD_ARGS`, so larger subsets have too many
        // arguments
        if argument_kinds.len() >= usize::from(MAX_TOTAL_METHOD_ARGS) {
            continue;
        }
        let method = method_with_arguments(ContractMetadataKind::UpdateStatefulRw, &argument_kinds);
        assert!(
            decode_arguments(&method, MethodsContainerKind::Contract).is_ok(),
            "Arguments {argument_kinds:?}"
        );
    }

    // The state of `#[init]` is its last argument and has no type: the return value or, without
    // one, the last `#[output]`
    for state in [ContractMetadataKind::Output, ContractMetadataKind::Return] {
        let init = [
            &[ContractMetadataKind::Init as u8, 4][..],
            b"init",
            &[3],
            &argument_metadata(ContractMetadataKind::SlotRw),
            &argument_metadata(ContractMetadataKind::Output),
            &[state as u8, 0],
        ]
        .concat();
        decode_arguments(&init, MethodsContainerKind::Contract).unwrap();
    }
}

#[test]
fn arguments_out_of_order_are_rejected() {
    // For every pair of arguments, the second one is rejected unless its position is later than
    // that of the first one, or the same for `#[slot]`s, `#[input]`s and `#[output]`s
    for first in ARGUMENT_KINDS {
        for second in ARGUMENT_KINDS {
            let (first_position, repeated) = argument_position(first);
            let (second_position, _) = argument_position(second);
            let in_order =
                first_position < second_position || (first_position == second_position && repeated);

            let method =
                method_with_arguments(ContractMetadataKind::UpdateStatefulRw, &[first, second]);
            let result = decode_arguments(&method, MethodsContainerKind::Contract);
            if in_order {
                assert!(result.is_ok(), "Arguments {first:?} and {second:?}");
            } else {
                assert!(
                    matches!(
                        result,
                        Err((
                            1,
                            MetadataDecodingError::ArgumentOutOfOrder { argument_kind: _ }
                        ))
                    ),
                    "Arguments {first:?} and {second:?}: {result:?}"
                );
            }
        }
    }

    // The error names the argument out of order, here a `#[slot]` after an `#[input]` in a
    // `#[view]` method
    let method = method_with_arguments(
        ContractMetadataKind::ViewStateless,
        &[
            ContractMetadataKind::EnvRo,
            ContractMetadataKind::SlotRo,
            ContractMetadataKind::Input,
            ContractMetadataKind::SlotRo,
        ],
    );
    assert!(matches!(
        decode_arguments(&method, MethodsContainerKind::Contract),
        Err((
            3,
            MetadataDecodingError::ArgumentOutOfOrder {
                argument_kind: ArgumentKind::SlotRo
            }
        ))
    ));
}

#[test]
fn trait_methods_have_no_tmp_or_slots() {
    // Argument kinds that a method allows in a contract and in a trait. Traits only have stateless
    // methods, and `#[view]` methods have no `#[tmp]` and only read-only `#[env]` and `#[slot]`s.
    let allowed_argument_kinds = [
        (
            ContractMetadataKind::UpdateStateless,
            &ARGUMENT_KINDS[..],
            &[
                ContractMetadataKind::EnvRo,
                ContractMetadataKind::EnvRw,
                ContractMetadataKind::Input,
                ContractMetadataKind::Output,
                ContractMetadataKind::Return,
            ][..],
        ),
        (
            ContractMetadataKind::ViewStateless,
            &[
                ContractMetadataKind::EnvRo,
                ContractMetadataKind::SlotRo,
                ContractMetadataKind::Input,
                ContractMetadataKind::Output,
                ContractMetadataKind::Return,
            ][..],
            &[
                ContractMetadataKind::EnvRo,
                ContractMetadataKind::Input,
                ContractMetadataKind::Output,
                ContractMetadataKind::Return,
            ][..],
        ),
    ];

    for (method_kind, in_contract, in_trait) in allowed_argument_kinds {
        for argument_kind in ARGUMENT_KINDS {
            let method = method_with_arguments(method_kind, &[argument_kind]);

            for (container_kind, allowed) in [
                (MethodsContainerKind::Contract, in_contract),
                // Without knowing the container, the decoder accepts what a contract allows
                (MethodsContainerKind::Unknown, in_contract),
                (MethodsContainerKind::Trait, in_trait),
            ] {
                let result = decode_arguments(&method, container_kind);
                if allowed.contains(&argument_kind) {
                    assert!(
                        result.is_ok(),
                        "{method_kind:?} with {argument_kind:?} in {container_kind:?}: {result:?}"
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err((
                                0,
                                MetadataDecodingError::UnexpectedArgumentKind {
                                    argument_kind: _,
                                    method_kind: _,
                                    container_kind: _,
                                }
                            ))
                        ),
                        "{method_kind:?} with {argument_kind:?} in {container_kind:?}: {result:?}"
                    );
                }
            }
        }
    }

    // The container kind reaches the arguments decoder through `MetadataDecoder`
    let update = method_with_arguments(
        ContractMetadataKind::UpdateStateless,
        &[ContractMetadataKind::SlotRw],
    );
    decode_first_argument(&contract_with_method(
        &[IoTypeMetadataKind::Unit as u8],
        &update,
    ))
    .unwrap();
    assert!(matches!(
        decode_first_argument(&trait_with_method(&update)),
        Err(MetadataDecodingError::UnexpectedArgumentKind {
            argument_kind: ArgumentKind::SlotRw,
            method_kind: MethodKind::UpdateStateless,
            container_kind: MethodsContainerKind::Trait,
        })
    ));
}

/// Metadata of an `#[init]` method with arguments of `argument_kinds` followed by the state
fn init_with_state(
    argument_kinds: &[ContractMetadataKind],
    state: ContractMetadataKind,
) -> Vec<u8> {
    let mut metadata = vec![ContractMetadataKind::Init as u8, 4];
    metadata.extend_from_slice(b"init");
    metadata.push(u8::try_from(argument_kinds.len() + 1).unwrap());
    for &argument_kind in argument_kinds {
        metadata.extend_from_slice(&argument_metadata(argument_kind));
    }
    // The state has no type
    metadata.extend_from_slice(&[state as u8, 0]);
    metadata
}

#[test]
fn init_must_end_with_state() {
    // The state is the return value or, without one, the last `#[output]`
    for state in [ContractMetadataKind::Output, ContractMetadataKind::Return] {
        for argument_kinds in [&[][..], &[ContractMetadataKind::Input]] {
            let init = init_with_state(argument_kinds, state);
            decode_arguments(&init, MethodsContainerKind::Contract).unwrap();
        }
    }

    // Without arguments there is no state
    let init = method_with_arguments(ContractMetadataKind::Init, &[]);
    let mut init = init.as_slice();
    assert!(matches!(
        MethodMetadataDecoder::new(&mut init, MethodsContainerKind::Contract).decode_next(),
        Err(MetadataDecodingError::InitWithoutState)
    ));

    // Any other last argument is not the state, even with a type
    for argument_kinds in [
        &[ContractMetadataKind::EnvRw][..],
        &[ContractMetadataKind::TmpRw],
        &[ContractMetadataKind::SlotRw],
        &[ContractMetadataKind::Input],
        &[
            ContractMetadataKind::EnvRw,
            ContractMetadataKind::TmpRw,
            ContractMetadataKind::SlotRw,
            ContractMetadataKind::Input,
        ],
    ] {
        let init = method_with_arguments(ContractMetadataKind::Init, argument_kinds);
        let result = decode_arguments(&init, MethodsContainerKind::Contract);
        assert!(
            matches!(
                result,
                Err((index, MetadataDecodingError::InitWithoutState))
                    if index == argument_kinds.len() - 1
            ),
            "Arguments {argument_kinds:?}: {result:?}"
        );
    }
}

#[test]
fn too_many_arguments_are_rejected() {
    // `self` counts towards `MAX_TOTAL_METHOD_ARGS` in methods that have it
    for (method_kind, max_arguments) in [
        (ContractMetadataKind::UpdateStateless, MAX_TOTAL_METHOD_ARGS),
        (
            ContractMetadataKind::UpdateStatefulRw,
            MAX_TOTAL_METHOD_ARGS - 1,
        ),
    ] {
        let arguments = vec![ContractMetadataKind::Input; usize::from(max_arguments)];
        let method = method_with_arguments(method_kind, &arguments);
        decode_arguments(&method, MethodsContainerKind::Contract).unwrap();

        let too_many_arguments = vec![ContractMetadataKind::Input; usize::from(max_arguments) + 1];
        let method = method_with_arguments(method_kind, &too_many_arguments);
        let mut method = method.as_slice();
        assert!(
            matches!(
                MethodMetadataDecoder::new(&mut method, MethodsContainerKind::Contract)
                    .decode_next(),
                Err(MetadataDecodingError::TooManyArguments { number_of_arguments })
                    if number_of_arguments == MAX_TOTAL_METHOD_ARGS + 1
            ),
            "{method_kind:?}"
        );
    }
}
