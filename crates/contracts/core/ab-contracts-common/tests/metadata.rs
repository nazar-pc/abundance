//! Tests of contract metadata and of metadata of types used by contracts

use ab_contracts_common::env::{EnvState, MethodContext};
use ab_contracts_common::metadata::ContractMetadataKind;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_core_primitives::balance::Balance;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::shard::ShardIndex;
use ab_core_primitives::transaction::{Gas, TransactionHeader, TransactionSlot};
use ab_io_type::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY};
use ab_io_type::trivial_type::TrivialType;

/// Check that compact metadata of `T` decodes into the size and alignment of `T`
#[track_caller]
fn check_compact_metadata<T>()
where
    T: TrivialType,
{
    let mut compact = [0; MAX_METADATA_CAPACITY];
    let (remainder, compact_remainder) =
        IoTypeMetadataKind::compact(T::METADATA, &mut compact).unwrap();
    assert!(remainder.is_empty());
    let compact_length = MAX_METADATA_CAPACITY - compact_remainder.len();

    let (type_details, remainder) =
        IoTypeMetadataKind::type_details(&compact[..compact_length]).unwrap();
    assert!(remainder.is_empty());
    assert_eq!(
        usize::try_from(type_details.recommended_capacity).unwrap(),
        size_of::<T>()
    );
    assert_eq!(usize::from(type_details.alignment.get()), align_of::<T>());
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
