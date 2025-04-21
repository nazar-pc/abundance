//! Benchmarks for Subspace extension

use crate::extensions::SubspaceExtension;
use crate::pallet::{
    BlockSlots, CurrentBlockAuthorInfo, SegmentRoot as SubspaceSegmentRoot, SolutionRanges,
};
use crate::{Config, Pallet as Subspace};
use frame_benchmarking::v2::*;
use frame_support::dispatch::{DispatchInfo, PostDispatchInfo};
use frame_system::pallet_prelude::RuntimeCallFor;
use parity_scale_codec::{Decode, Encode};
use scale_info::prelude::fmt;
use sp_consensus_slots::Slot;
use sp_runtime::traits::{AsSystemOriginSigner, Dispatchable, NumberFor};
use sp_runtime::transaction_validity::TransactionSource;
use sp_std::collections::btree_map::BTreeMap;
use subspace_core_primitives::pieces::{PieceOffset, RecordChunk};
use subspace_core_primitives::sectors::SectorIndex;
use subspace_core_primitives::segments::{SegmentIndex, SegmentRoot};
use subspace_core_primitives::PublicKey;

pub struct Pallet<T: Config>(Subspace<T>);

#[allow(clippy::multiple_bound_locations)]
#[benchmarks(where
	T: Send + Sync + scale_info::TypeInfo + fmt::Debug,
    RuntimeCallFor<T>: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
	<RuntimeCallFor<T> as Dispatchable>::RuntimeOrigin: AsSystemOriginSigner<<T as frame_system::Config>::AccountId> + Clone)
]
mod benchmarks {
    use super::*;

    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
