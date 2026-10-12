//! Tests of contract metadata and of metadata of types used by contracts

use ab_contracts_common::env::{EnvState, MethodContext};
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
