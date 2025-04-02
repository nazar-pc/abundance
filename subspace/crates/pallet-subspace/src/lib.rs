#![doc = include_str!("../README.md")]
#![cfg_attr(not(feature = "std"), no_std)]
#![feature(array_chunks, assert_matches, let_chains, portable_simd)]
#![warn(unused_must_use, unsafe_code, unused_variables)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod extensions;
pub mod weights;

use crate::extensions::weights::WeightInfo as ExtensionWeightInfo;
#[cfg(not(feature = "std"))]
use alloc::string::String;
use core::num::NonZeroU64;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::{EnsureOrigin, RuntimeDebug};
use frame_support::traits::Get;
use frame_system::pallet_prelude::*;
use log::{debug, warn};
pub use pallet::*;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_consensus_slots::Slot;
use sp_consensus_subspace::digests::CompatibleDigestItem;
use sp_consensus_subspace::{PotParameters, PotParametersChange};
use sp_runtime::generic::DigestItem;
use sp_runtime::traits::{BlockNumberProvider, CheckedSub, Hash, One, Zero};
use sp_runtime::transaction_validity::{
    InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
    TransactionValidityError, ValidTransaction,
};
use sp_std::prelude::*;
use subspace_core_primitives::segments::{
    ArchivedHistorySegment, HistorySize, SegmentHeader, SegmentIndex,
};
use subspace_verification::{derive_next_solution_range, derive_pot_entropy};

/// Trigger an era change, if any should take place.
pub trait EraChangeTrigger {
    /// Trigger an era change, if any should take place. This should be called
    /// during every block, after initialization is done.
    fn trigger<T: Config>(block_number: BlockNumberFor<T>);
}

/// A type signifying to Subspace that it should perform era changes with an internal trigger.
pub struct NormalEraChange;

impl EraChangeTrigger for NormalEraChange {
    fn trigger<T: Config>(block_number: BlockNumberFor<T>) {
        if <Pallet<T>>::should_era_change(block_number) {
            <Pallet<T>>::enact_era_change();
        }
    }
}

/// Custom origin for validated unsigned extrinsics.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RawOrigin {
    ValidatedUnsigned,
}

/// Ensure the subspace origin.
pub struct EnsureSubspaceOrigin;
impl<O: Into<Result<RawOrigin, O>> + From<RawOrigin>> EnsureOrigin<O> for EnsureSubspaceOrigin {
    type Success = ();

    fn try_origin(o: O) -> Result<Self::Success, O> {
        o.into().map(|o| match o {
            RawOrigin::ValidatedUnsigned => (),
        })
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<O, ()> {
        Ok(O::from(RawOrigin::ValidatedUnsigned))
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::{EraChangeTrigger, ExtensionWeightInfo};
    use crate::weights::WeightInfo;
    use crate::RawOrigin;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_consensus_slots::Slot;
    use sp_consensus_subspace::digests::CompatibleDigestItem;
    use sp_consensus_subspace::inherents::{InherentError, InherentType, INHERENT_IDENTIFIER};
    use sp_runtime::traits::One;
    use sp_runtime::DigestItem;
    use sp_std::collections::btree_map::BTreeMap;
    use sp_std::num::NonZeroU32;
    use sp_std::prelude::*;
    use subspace_core_primitives::hashes::Blake3Hash;
    use subspace_core_primitives::pieces::PieceOffset;
    use subspace_core_primitives::pot::PotCheckpoints;
    use subspace_core_primitives::sectors::SectorIndex;
    use subspace_core_primitives::segments::{HistorySize, SegmentHeader, SegmentIndex};
    use subspace_core_primitives::solutions::SolutionRange;
    use subspace_core_primitives::{PublicKey, Randomness, ScalarBytes};

    pub(super) struct InitialSolutionRanges<T: Config> {
        _config: T,
    }

    impl<T: Config> Get<sp_consensus_subspace::SolutionRanges> for InitialSolutionRanges<T> {
        fn get() -> sp_consensus_subspace::SolutionRanges {
            sp_consensus_subspace::SolutionRanges {
                current: T::InitialSolutionRange::get(),
                next: None,
            }
        }
    }

    /// Override for next solution range adjustment
    #[derive(Debug, Encode, Decode, TypeInfo)]
    pub(super) struct SolutionRangeOverride {
        /// Value that should be set as solution range
        pub(super) solution_range: SolutionRange,
    }

    /// The Subspace Pallet
    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::config]
    #[pallet::disable_frame_system_supertrait_check]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Origin for subspace call.
        type SubspaceOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = ()>;

        /// Number of slots between slot arrival and when corresponding block can be produced.
        ///
        /// Practically this means future proof of time proof needs to be revealed this many slots
        /// ahead before block can be authored even though solution is available before that.
        #[pallet::constant]
        type BlockAuthoringDelay: Get<Slot>;

        /// Interval, in blocks, between blockchain entropy injection into proof of time chain.
        #[pallet::constant]
        type PotEntropyInjectionInterval: Get<BlockNumberFor<Self>>;

        /// Interval, in entropy injection intervals, where to take entropy for injection from.
        #[pallet::constant]
        type PotEntropyInjectionLookbackDepth: Get<u8>;

        /// Delay after block, in slots, when entropy injection takes effect.
        #[pallet::constant]
        type PotEntropyInjectionDelay: Get<Slot>;

        /// The amount of time, in blocks, that each era should last.
        /// NOTE: Currently it is not possible to change the era duration after
        /// the chain has started. Attempting to do so will brick block production.
        #[pallet::constant]
        type EraDuration: Get<BlockNumberFor<Self>>;

        /// Initial solution range used for challenges during the very first era.
        #[pallet::constant]
        type InitialSolutionRange: Get<SolutionRange>;

        /// How often in slots slots (on average, not counting collisions) will have a block.
        ///
        /// Expressed as a rational where the first member of the tuple is the
        /// numerator and the second is the denominator. The rational should
        /// represent a value between 0 and 1.
        #[pallet::constant]
        type SlotProbability: Get<(u64, u64)>;

        /// Depth `K` after which a block enters the recorded history (a global constant, as opposed
        /// to the client-dependent transaction confirmation depth `k`).
        #[pallet::constant]
        type ConfirmationDepthK: Get<BlockNumberFor<Self>>;

        /// Number of latest archived segments that are considered "recent history".
        #[pallet::constant]
        type RecentSegments: Get<HistorySize>;

        /// Fraction of pieces from the "recent history" (`recent_segments`) in each sector.
        #[pallet::constant]
        type RecentHistoryFraction: Get<(HistorySize, HistorySize)>;

        /// Minimum lifetime of a plotted sector, measured in archived segment.
        #[pallet::constant]
        type MinSectorLifetime: Get<HistorySize>;

        /// How many pieces one sector is supposed to contain (max)
        #[pallet::constant]
        type MaxPiecesInSector: Get<u16>;

        type ShouldAdjustSolutionRange: Get<bool>;
        /// Subspace requires some logic to be triggered on every block to query for whether an era
        /// has ended and to perform the transition to the next era.
        ///
        /// Era is normally used to update solution range used for challenges.
        type EraChangeTrigger: EraChangeTrigger;

        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;

        /// Maximum number of block number to block slot mappings to keep (oldest pruned first).
        #[pallet::constant]
        type BlockSlotCount: Get<u32>;

        /// Extension weight information for the pallet's extensions.
        type ExtensionWeightInfo: ExtensionWeightInfo;
    }

    #[derive(Debug, Default, Encode, Decode, TypeInfo)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub enum AllowAuthoringBy {
        /// Anyone can author new blocks at genesis.
        #[default]
        Anyone,
        /// Author of the first block will be able to author blocks going forward unless unlocked
        /// for everyone.
        FirstFarmer,
        /// Specified root farmer is allowed to author blocks unless unlocked for everyone.
        RootFarmer(PublicKey),
    }

    #[derive(Debug, Copy, Clone, Encode, Decode, TypeInfo)]
    pub(super) struct PotEntropyValue {
        /// Target slot at which entropy should be injected (when known)
        pub(super) target_slot: Option<Slot>,
        pub(super) entropy: Blake3Hash,
    }

    #[derive(Debug, Copy, Clone, Encode, Decode, TypeInfo, PartialEq)]
    pub(super) struct PotSlotIterationsValue {
        pub(super) slot_iterations: NonZeroU32,
        /// Scheduled proof of time slot iterations update
        pub(super) update: Option<PotSlotIterationsUpdate>,
    }

    #[derive(Debug, Copy, Clone, Encode, Decode, TypeInfo, PartialEq)]
    pub(super) struct PotSlotIterationsUpdate {
        /// Target slot at which entropy should be injected (when known)
        pub(super) target_slot: Option<Slot>,
        pub(super) slot_iterations: NonZeroU32,
    }

    /// When to enable block rewards
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Encode, Decode, TypeInfo)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub enum EnableRewardsAt<BlockNumber> {
        /// At specified height or next block if `None`
        Height(BlockNumber),
        /// When solution range is below specified threshold
        SolutionRange(u64),
        /// Manually with an explicit extrinsic
        Manually,
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T>
    where
        T: Config,
    {
        /// When rewards should be enabled.
        pub enable_rewards_at: EnableRewardsAt<BlockNumberFor<T>>,
        /// Who can author blocks at genesis.
        pub allow_authoring_by: AllowAuthoringBy,
        /// Number of iterations for proof of time per slot
        pub pot_slot_iterations: NonZeroU32,
        #[serde(skip)]
        pub phantom: PhantomData<T>,
    }

    impl<T> Default for GenesisConfig<T>
    where
        T: Config,
    {
        #[inline]
        fn default() -> Self {
            Self {
                enable_rewards_at: EnableRewardsAt::Height(BlockNumberFor::<T>::one()),
                allow_authoring_by: AllowAuthoringBy::Anyone,
                pot_slot_iterations: NonZeroU32::MIN,
                phantom: PhantomData,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T> BuildGenesisConfig for GenesisConfig<T>
    where
        T: Config,
    {
        fn build(&self) {
            match self.enable_rewards_at {
                EnableRewardsAt::Height(block_number) => {
                    EnableRewards::<T>::put(block_number);
                }
                EnableRewardsAt::SolutionRange(solution_range) => {
                    EnableRewardsBelowSolutionRange::<T>::put(solution_range);
                }
                EnableRewardsAt::Manually => {
                    // Nothing to do in this case
                }
            }
            match &self.allow_authoring_by {
                AllowAuthoringBy::Anyone => {
                    AllowAuthoringByAnyone::<T>::put(true);
                }
                AllowAuthoringBy::FirstFarmer => {
                    AllowAuthoringByAnyone::<T>::put(false);
                }
                AllowAuthoringBy::RootFarmer(root_farmer) => {
                    AllowAuthoringByAnyone::<T>::put(false);
                    RootPlotPublicKey::<T>::put(root_farmer);
                }
            }
            PotSlotIterations::<T>::put(PotSlotIterationsValue {
                slot_iterations: self.pot_slot_iterations,
                update: None,
            });
        }
    }

    /// Events type.
    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Segment header was stored in blockchain history.
        SegmentHeaderStored { segment_header: SegmentHeader },
    }

    #[pallet::origin]
    pub type Origin = RawOrigin;

    #[pallet::error]
    pub enum Error<T> {
        /// Solution range adjustment already enabled.
        SolutionRangeAdjustmentAlreadyEnabled,
        /// Iterations are not multiple of number of checkpoints times two
        NotMultipleOfCheckpoints,
        /// Proof of time slot iterations must increase as hardware improves
        PotSlotIterationsMustIncrease,
        /// Proof of time slot iterations update already scheduled
        PotSlotIterationsUpdateAlreadyScheduled,
    }

    /// Bounded mapping from block number to slot
    #[pallet::storage]
    #[pallet::getter(fn block_slots)]
    pub(super) type BlockSlots<T: Config> =
        StorageValue<_, BoundedBTreeMap<BlockNumberFor<T>, Slot, T::BlockSlotCount>, ValueQuery>;

    /// Solution ranges used for challenges.
    #[pallet::storage]
    #[pallet::getter(fn solution_ranges)]
    pub(super) type SolutionRanges<T: Config> = StorageValue<
        _,
        sp_consensus_subspace::SolutionRanges,
        ValueQuery,
        InitialSolutionRanges<T>,
    >;

    /// Storage to check if the solution range is to be adjusted for next era
    #[pallet::storage]
    #[pallet::getter(fn should_adjust_solution_range)]
    pub(super) type ShouldAdjustSolutionRange<T: Config> =
        StorageValue<_, bool, ValueQuery, T::ShouldAdjustSolutionRange>;

    /// Override solution range during next update
    #[pallet::storage]
    pub(super) type NextSolutionRangeOverride<T> = StorageValue<_, SolutionRangeOverride>;

    /// Slot at which current era started.
    #[pallet::storage]
    pub(super) type EraStartSlot<T> = StorageValue<_, Slot>;

    /// Mapping from segment index to corresponding segment commitment of contained records.
    #[pallet::storage]
    #[pallet::getter(fn segment_commitment)]
    pub(super) type SegmentCommitment<T> = CountedStorageMap<
        _,
        Twox64Concat,
        SegmentIndex,
        subspace_core_primitives::segments::SegmentCommitment,
    >;

    /// Whether the segment headers inherent has been processed in this block (temporary value).
    ///
    /// This value is updated to `true` when processing `store_segment_headers` by a node.
    /// It is then cleared at the end of each block execution in the `on_finalize` hook.
    #[pallet::storage]
    pub(super) type DidProcessSegmentHeaders<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Parent block author information.
    #[pallet::storage]
    pub(super) type ParentBlockAuthorInfo<T> =
        StorageValue<_, (PublicKey, SectorIndex, PieceOffset, ScalarBytes, Slot)>;

    /// Enable rewards since specified block number.
    #[pallet::storage]
    pub(super) type EnableRewards<T: Config> = StorageValue<_, BlockNumberFor<T>>;

    /// Enable rewards when solution range is below this threshold.
    #[pallet::storage]
    pub(super) type EnableRewardsBelowSolutionRange<T: Config> = StorageValue<_, u64>;

    /// Block author information
    #[pallet::storage]
    pub(super) type CurrentBlockAuthorInfo<T: Config> = StorageValue<
        _,
        (
            PublicKey,
            SectorIndex,
            PieceOffset,
            ScalarBytes,
            Slot,
            Option<T::AccountId>,
        ),
    >;

    /// Number of iterations for proof of time per slot with optional scheduled update
    #[pallet::storage]
    pub(super) type PotSlotIterations<T> = StorageValue<_, PotSlotIterationsValue>;

    /// Entropy that needs to be injected into proof of time chain at specific slot associated with
    /// block number it came from.
    #[pallet::storage]
    pub(super) type PotEntropy<T: Config> =
        StorageValue<_, BTreeMap<BlockNumberFor<T>, PotEntropyValue>, ValueQuery>;

    /// The current block randomness, updated at block initialization. When the proof of time feature
    /// is enabled it derived from PoT otherwise PoR.
    #[pallet::storage]
    pub type BlockRandomness<T> = StorageValue<_, Randomness>;

    /// Allow block authoring by anyone or just root.
    #[pallet::storage]
    pub(super) type AllowAuthoringByAnyone<T> = StorageValue<_, bool, ValueQuery>;

    /// Root plot public key.
    ///
    /// Set just once to make sure no one else can author blocks until allowed for anyone.
    #[pallet::storage]
    #[pallet::getter(fn root_plot_public_key)]
    pub(super) type RootPlotPublicKey<T> = StorageValue<_, PublicKey>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: BlockNumberFor<T>) -> Weight {
            Self::do_initialize(block_number);
            Weight::zero()
        }

        fn on_finalize(block_number: BlockNumberFor<T>) {
            Self::do_finalize(block_number)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Submit new segment header to the blockchain. This is an inherent extrinsic and part of
        /// the Subspace consensus logic.
        #[pallet::call_index(0)]
        #[pallet::weight((< T as Config >::WeightInfo::store_segment_headers(segment_headers.len() as u32), DispatchClass::Mandatory))]
        pub fn store_segment_headers(
            origin: OriginFor<T>,
            segment_headers: Vec<SegmentHeader>,
        ) -> DispatchResult {
            ensure_none(origin)?;
            Self::do_store_segment_headers(segment_headers)
        }

        /// Enable solution range adjustment after every era.
        /// Note: No effect on the solution range for the current era
        #[pallet::call_index(1)]
        #[pallet::weight(< T as Config >::WeightInfo::enable_solution_range_adjustment())]
        pub fn enable_solution_range_adjustment(
            origin: OriginFor<T>,
            solution_range_override: Option<u64>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Self::do_enable_solution_range_adjustment(solution_range_override)?;

            frame_system::Pallet::<T>::deposit_log(
                DigestItem::enable_solution_range_adjustment_and_override(solution_range_override),
            );

            Ok(())
        }

        /// Enable rewards for blocks at specified block height.
        #[pallet::call_index(3)]
        #[pallet::weight(< T as Config >::WeightInfo::enable_rewards_at())]
        pub fn enable_rewards_at(
            origin: OriginFor<T>,
            enable_rewards_at: EnableRewardsAt<BlockNumberFor<T>>,
        ) -> DispatchResult {
            ensure_root(origin)?;

            Self::do_enable_rewards_at(enable_rewards_at)
        }

        /// Enable storage access for all users.
        #[pallet::call_index(4)]
        #[pallet::weight(< T as Config >::WeightInfo::enable_authoring_by_anyone())]
        pub fn enable_authoring_by_anyone(origin: OriginFor<T>) -> DispatchResult {
            ensure_root(origin)?;

            AllowAuthoringByAnyone::<T>::put(true);
            RootPlotPublicKey::<T>::take();
            // Deposit root plot public key update such that light client can validate blocks later.
            frame_system::Pallet::<T>::deposit_log(DigestItem::root_plot_public_key_update(None));

            Ok(())
        }

        /// Update proof of time slot iterations
        #[pallet::call_index(5)]
        #[pallet::weight(< T as Config >::WeightInfo::set_pot_slot_iterations())]
        pub fn set_pot_slot_iterations(
            origin: OriginFor<T>,
            slot_iterations: NonZeroU32,
        ) -> DispatchResult {
            ensure_root(origin)?;

            if slot_iterations.get() % u32::from(PotCheckpoints::NUM_CHECKPOINTS.get() * 2) != 0 {
                return Err(Error::<T>::NotMultipleOfCheckpoints.into());
            }

            let mut pot_slot_iterations =
                PotSlotIterations::<T>::get().expect("Always initialized during genesis; qed");

            if pot_slot_iterations.slot_iterations >= slot_iterations {
                return Err(Error::<T>::PotSlotIterationsMustIncrease.into());
            }

            // Can't update if already scheduled since it will cause verification issues
            if let Some(pot_slot_iterations_update_value) = pot_slot_iterations.update
                && pot_slot_iterations_update_value.target_slot.is_some()
            {
                return Err(Error::<T>::PotSlotIterationsUpdateAlreadyScheduled.into());
            }

            pot_slot_iterations.update.replace(PotSlotIterationsUpdate {
                // Slot will be known later when next entropy injection takes place
                target_slot: None,
                slot_iterations,
            });

            PotSlotIterations::<T>::put(pot_slot_iterations);

            Ok(())
        }
    }

    #[pallet::inherent]
    impl<T: Config> ProvideInherent for Pallet<T> {
        type Call = Call<T>;
        type Error = InherentError;
        const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;

        fn create_inherent(data: &InherentData) -> Option<Self::Call> {
            let inherent_data = data
                .get_data::<InherentType>(&INHERENT_IDENTIFIER)
                .expect("Subspace inherent data not correctly encoded")
                .expect("Subspace inherent data must be provided");

            let segment_headers = inherent_data.segment_headers;
            if segment_headers.is_empty() {
                None
            } else {
                Some(Call::store_segment_headers { segment_headers })
            }
        }

        fn is_inherent_required(data: &InherentData) -> Result<Option<Self::Error>, Self::Error> {
            let inherent_data = data
                .get_data::<InherentType>(&INHERENT_IDENTIFIER)
                .expect("Subspace inherent data not correctly encoded")
                .expect("Subspace inherent data must be provided");

            Ok(if inherent_data.segment_headers.is_empty() {
                None
            } else {
                Some(InherentError::MissingSegmentHeadersList)
            })
        }

        fn check_inherent(call: &Self::Call, data: &InherentData) -> Result<(), Self::Error> {
            if let Call::store_segment_headers { segment_headers } = call {
                let inherent_data = data
                    .get_data::<InherentType>(&INHERENT_IDENTIFIER)
                    .expect("Subspace inherent data not correctly encoded")
                    .expect("Subspace inherent data must be provided");

                if segment_headers != &inherent_data.segment_headers {
                    return Err(InherentError::IncorrectSegmentHeadersList {
                        expected: inherent_data.segment_headers,
                        actual: segment_headers.clone(),
                    });
                }
            }

            Ok(())
        }

        fn is_inherent(call: &Self::Call) -> bool {
            matches!(call, Call::store_segment_headers { .. })
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::store_segment_headers { segment_headers } => {
                    Self::validate_segment_header(source, segment_headers)
                }
                _ => InvalidTransaction::Call.into(),
            }
        }

        fn pre_dispatch(call: &Self::Call) -> Result<(), TransactionValidityError> {
            match call {
                Call::store_segment_headers { segment_headers } => {
                    Self::pre_dispatch_segment_header(segment_headers)
                }
                _ => Err(InvalidTransaction::Call.into()),
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Total number of pieces in the blockchain
    pub fn history_size() -> HistorySize {
        // Chain starts with one segment plotted, even if it is not recorded in the runtime yet
        let number_of_segments = u64::from(SegmentCommitment::<T>::count()).max(1);
        HistorySize::from(NonZeroU64::new(number_of_segments).expect("Not zero; qed"))
    }

    /// Determine whether an era change should take place at this block.
    /// Assumes that initialization has already taken place.
    fn should_era_change(block_number: BlockNumberFor<T>) -> bool {
        block_number % T::EraDuration::get() == Zero::zero()
    }

    /// DANGEROUS: Enact era change. Should be done on every block where `should_era_change` has
    /// returned `true`, and the caller is the only caller of this function.
    ///
    /// This will update solution range used in consensus.
    fn enact_era_change() {
        let slot_probability = T::SlotProbability::get();

        let current_slot = Self::current_slot();

        SolutionRanges::<T>::mutate(|solution_ranges| {
            let next_solution_range;
            // Check if the solution range should be adjusted for next era.
            if !ShouldAdjustSolutionRange::<T>::get() {
                next_solution_range = solution_ranges.current;
            } else if let Some(solution_range_override) = NextSolutionRangeOverride::<T>::take() {
                next_solution_range = solution_range_override.solution_range;
            } else {
                next_solution_range = derive_next_solution_range(
                    // If Era start slot is not found it means we have just finished the first era
                    u64::from(EraStartSlot::<T>::get().unwrap_or_default()),
                    u64::from(current_slot),
                    slot_probability,
                    solution_ranges.current,
                    T::EraDuration::get()
                        .try_into()
                        .unwrap_or_else(|_| panic!("Era duration is always within u64; qed")),
                );
            };
            solution_ranges.next.replace(next_solution_range);

            if let Some(solution_range_for_rewards) = EnableRewardsBelowSolutionRange::<T>::get() {
                if next_solution_range <= solution_range_for_rewards {
                    EnableRewardsBelowSolutionRange::<T>::take();

                    let next_block_number =
                        frame_system::Pallet::<T>::current_block_number() + One::one();
                    EnableRewards::<T>::put(next_block_number);
                }
            }
        });

        EraStartSlot::<T>::put(current_slot);
    }

    fn do_initialize(block_number: BlockNumberFor<T>) {
        let pre_digest = frame_system::Pallet::<T>::digest()
            .logs
            .iter()
            .find_map(|s| s.as_subspace_pre_digest::<T::AccountId>())
            .expect("Block must always have pre-digest");
        let current_slot = pre_digest.slot();

        BlockSlots::<T>::mutate(|block_slots| {
            if let Some(to_remove) = block_number.checked_sub(&T::BlockSlotCount::get().into()) {
                block_slots.remove(&to_remove);
            }
            block_slots
                .try_insert(block_number, current_slot)
                .expect("one entry just removed before inserting; qed");
        });

        {
            // Remove old value
            CurrentBlockAuthorInfo::<T>::take();
            let farmer_public_key = pre_digest.solution().public_key;

            // Optional restriction for block authoring to the root user
            if !AllowAuthoringByAnyone::<T>::get() {
                RootPlotPublicKey::<T>::mutate(|maybe_root_plot_public_key| {
                    if let Some(root_plot_public_key) = maybe_root_plot_public_key {
                        if root_plot_public_key != &farmer_public_key {
                            panic!("Client bug, authoring must be only done by the root user");
                        }
                    } else {
                        maybe_root_plot_public_key.replace(farmer_public_key);
                        // Deposit root plot public key update such that light client can validate
                        // blocks later.
                        frame_system::Pallet::<T>::deposit_log(
                            DigestItem::root_plot_public_key_update(Some(farmer_public_key)),
                        );
                    }
                });
            }

            let key = (
                farmer_public_key,
                pre_digest.solution().sector_index,
                pre_digest.solution().piece_offset,
                pre_digest.solution().chunk,
                current_slot,
            );
            let (public_key, sector_index, piece_offset, chunk, slot) = key;

            CurrentBlockAuthorInfo::<T>::put((
                public_key,
                sector_index,
                piece_offset,
                chunk,
                slot,
                Some(pre_digest.solution().reward_address.clone()),
            ));
        }

        // If solution range was updated in previous block, set it as current.
        if let sp_consensus_subspace::SolutionRanges {
            next: Some(next), ..
        } = SolutionRanges::<T>::get()
        {
            SolutionRanges::<T>::put(sp_consensus_subspace::SolutionRanges {
                current: next,
                next: None,
            });
        }

        let block_randomness = pre_digest
            .pot_info()
            .proof_of_time()
            .derive_global_randomness();

        // Update the block randomness.
        BlockRandomness::<T>::put(block_randomness);

        // Deposit solution range data such that light client can validate blocks later.
        frame_system::Pallet::<T>::deposit_log(DigestItem::solution_range(
            SolutionRanges::<T>::get().current,
        ));

        // Enact era change, if necessary.
        T::EraChangeTrigger::trigger::<T>(block_number);

        {
            let mut pot_slot_iterations =
                PotSlotIterations::<T>::get().expect("Always initialized during genesis; qed");
            // This is what we had after previous block
            frame_system::Pallet::<T>::deposit_log(DigestItem::pot_slot_iterations(
                pot_slot_iterations.slot_iterations,
            ));

            // Check PoT slot iterations update and apply it if it is time to do so, while also
            // removing corresponding storage item
            if let Some(update) = pot_slot_iterations.update
                && let Some(target_slot) = update.target_slot
                && target_slot <= current_slot
            {
                debug!(
                    target: "runtime::subspace",
                    "Applying PoT slots update, changing to {} at block #{:?}",
                    update.slot_iterations,
                    block_number
                );
                pot_slot_iterations = PotSlotIterationsValue {
                    slot_iterations: update.slot_iterations,
                    update: None,
                };
                PotSlotIterations::<T>::put(pot_slot_iterations);
            }
            let pot_entropy_injection_interval = T::PotEntropyInjectionInterval::get();
            let pot_entropy_injection_delay = T::PotEntropyInjectionDelay::get();

            let mut entropy = PotEntropy::<T>::get();
            let lookback_in_blocks = pot_entropy_injection_interval
                * BlockNumberFor::<T>::from(T::PotEntropyInjectionLookbackDepth::get());
            let last_entropy_injection_block =
                block_number / pot_entropy_injection_interval * pot_entropy_injection_interval;
            let maybe_entropy_source_block_number =
                last_entropy_injection_block.checked_sub(&lookback_in_blocks);

            if (block_number % pot_entropy_injection_interval).is_zero() {
                let current_block_entropy = derive_pot_entropy(
                    &pre_digest.solution().chunk,
                    pre_digest.pot_info().proof_of_time(),
                );
                // Collect entropy every `T::PotEntropyInjectionInterval` blocks
                entropy.insert(
                    block_number,
                    PotEntropyValue {
                        target_slot: None,
                        entropy: current_block_entropy,
                    },
                );

                // Update target slot for entropy injection once we know it
                if let Some(entropy_source_block_number) = maybe_entropy_source_block_number {
                    if let Some(entropy_value) = entropy.get_mut(&entropy_source_block_number) {
                        let target_slot = pre_digest
                            .slot()
                            .saturating_add(pot_entropy_injection_delay);
                        debug!(
                            target: "runtime::subspace",
                            "Pot entropy injection will happen at slot {target_slot:?}",
                        );
                        entropy_value.target_slot.replace(target_slot);

                        // Schedule PoT slot iterations update at the same slot as entropy
                        if let Some(update) = &mut pot_slot_iterations.update
                            && update.target_slot.is_none()
                        {
                            debug!(
                                target: "runtime::subspace",
                                "Scheduling PoT slots update to happen at slot {target_slot:?}"
                            );
                            update.target_slot.replace(target_slot);
                            PotSlotIterations::<T>::put(pot_slot_iterations);
                        }
                    }
                }

                PotEntropy::<T>::put(entropy.clone());
            }

            // Deposit consensus log item with parameters change in case corresponding entropy is
            // available
            if let Some(entropy_source_block_number) = maybe_entropy_source_block_number {
                let maybe_entropy_value = entropy.get(&entropy_source_block_number).copied();
                if let Some(PotEntropyValue {
                    target_slot,
                    entropy,
                }) = maybe_entropy_value
                {
                    let target_slot = target_slot
                        .expect("Target slot is guaranteed to be present due to logic above; qed");
                    // Check if there was a PoT slot iterations update at the same exact slot
                    let slot_iterations = if let Some(update) = pot_slot_iterations.update
                        && let Some(update_target_slot) = update.target_slot
                        && update_target_slot == target_slot
                    {
                        debug!(
                            target: "runtime::subspace",
                            "Applying PoT slots update to the next PoT parameters change"
                        );
                        update.slot_iterations
                    } else {
                        pot_slot_iterations.slot_iterations
                    };

                    frame_system::Pallet::<T>::deposit_log(DigestItem::pot_parameters_change(
                        PotParametersChange {
                            slot: target_slot,
                            slot_iterations,
                            entropy,
                        },
                    ));
                }
            }

            // Clean up old values we'll no longer need
            if let Some(entry) = entropy.first_entry() {
                if let Some(target_slot) = entry.get().target_slot
                    && target_slot < current_slot
                {
                    entry.remove();
                    PotEntropy::<T>::put(entropy);
                }
            }
        }
    }

    fn do_finalize(_block_number: BlockNumberFor<T>) {
        // Deposit the next solution range in the block finalization to account for solution range override extrinsic and
        // era change happens in the same block.
        if let Some(next_solution_range) = SolutionRanges::<T>::get().next {
            // Deposit next solution range data such that light client can validate blocks later.
            frame_system::Pallet::<T>::deposit_log(DigestItem::next_solution_range(
                next_solution_range,
            ));
        }

        if let Some((public_key, sector_index, piece_offset, scalar, slot, _reward_address)) =
            CurrentBlockAuthorInfo::<T>::get()
        {
            ParentBlockAuthorInfo::<T>::put((public_key, sector_index, piece_offset, scalar, slot));
        } else {
            ParentBlockAuthorInfo::<T>::take();
        }

        DidProcessSegmentHeaders::<T>::take();
    }

    fn do_store_segment_headers(segment_headers: Vec<SegmentHeader>) -> DispatchResult {
        assert!(
            !DidProcessSegmentHeaders::<T>::exists(),
            "Segment headers must be updated only once in the block"
        );

        for segment_header in segment_headers {
            SegmentCommitment::<T>::insert(
                segment_header.segment_index(),
                segment_header.segment_commitment(),
            );
            // Deposit global randomness data such that light client can validate blocks later.
            frame_system::Pallet::<T>::deposit_log(DigestItem::segment_commitment(
                segment_header.segment_index(),
                segment_header.segment_commitment(),
            ));
            Self::deposit_event(Event::SegmentHeaderStored { segment_header });
        }

        DidProcessSegmentHeaders::<T>::put(true);
        Ok(())
    }

    fn do_enable_solution_range_adjustment(solution_range_override: Option<u64>) -> DispatchResult {
        if ShouldAdjustSolutionRange::<T>::get() {
            return Err(Error::<T>::SolutionRangeAdjustmentAlreadyEnabled.into());
        }

        ShouldAdjustSolutionRange::<T>::put(true);

        if let Some(solution_range) = solution_range_override {
            SolutionRanges::<T>::mutate(|solution_ranges| {
                // If solution range update is already scheduled, just update values
                if solution_ranges.next.is_some() {
                    solution_ranges.next.replace(solution_range);
                } else {
                    solution_ranges.current = solution_range;

                    // Solution range can re-adjust very soon, make sure next re-adjustment is
                    // also overridden
                    NextSolutionRangeOverride::<T>::put(SolutionRangeOverride { solution_range });
                    frame_system::Pallet::<T>::deposit_log(DigestItem::next_solution_range(
                        solution_range,
                    ));
                }
            });
        }

        Ok(())
    }

    fn do_enable_rewards_at(
        enable_rewards_at: EnableRewardsAt<BlockNumberFor<T>>,
    ) -> DispatchResult {
        match enable_rewards_at {
            EnableRewardsAt::Height(block_number) => {
                // Enable rewards at a particular block height (default to the next block after
                // this)
                let next_block_number =
                    frame_system::Pallet::<T>::current_block_number() + One::one();
                EnableRewards::<T>::put(block_number.max(next_block_number));
            }
            EnableRewardsAt::SolutionRange(solution_range) => {
                EnableRewardsBelowSolutionRange::<T>::put(solution_range);
            }
            EnableRewardsAt::Manually => {
                // Nothing to do in this case
            }
        }

        Ok(())
    }

    /// Proof of time parameters
    pub fn pot_parameters() -> PotParameters {
        let block_number = frame_system::Pallet::<T>::block_number();
        let pot_slot_iterations =
            PotSlotIterations::<T>::get().expect("Always initialized during genesis; qed");
        let pot_entropy_injection_interval = T::PotEntropyInjectionInterval::get();

        let entropy = PotEntropy::<T>::get();
        let lookback_in_blocks = pot_entropy_injection_interval
            * BlockNumberFor::<T>::from(T::PotEntropyInjectionLookbackDepth::get());
        let last_entropy_injection_block =
            block_number / pot_entropy_injection_interval * pot_entropy_injection_interval;
        let maybe_entropy_source_block_number =
            last_entropy_injection_block.checked_sub(&lookback_in_blocks);

        let mut next_change = None;

        if let Some(entropy_source_block_number) = maybe_entropy_source_block_number {
            let maybe_entropy_value = entropy.get(&entropy_source_block_number).copied();
            if let Some(PotEntropyValue {
                target_slot,
                entropy,
            }) = maybe_entropy_value
            {
                let target_slot = target_slot.expect(
                    "Always present due to identical check present in block initialization; qed",
                );
                // Check if there was a PoT slot iterations update at the same exact slot
                let slot_iterations = if let Some(update) = pot_slot_iterations.update
                    && let Some(update_target_slot) = update.target_slot
                    && update_target_slot == target_slot
                {
                    update.slot_iterations
                } else {
                    pot_slot_iterations.slot_iterations
                };

                next_change.replace(PotParametersChange {
                    slot: target_slot,
                    slot_iterations,
                    entropy,
                });
            }
        }

        PotParameters::V0 {
            slot_iterations: pot_slot_iterations.slot_iterations,
            next_change,
        }
    }

    /// Current slot number
    pub fn current_slot() -> Slot {
        BlockSlots::<T>::get()
            .last_key_value()
            .map(|(_block, slot)| *slot)
            .unwrap_or_default()
    }

    /// Size of the archived history of the blockchain in bytes
    pub fn archived_history_size() -> u64 {
        let archived_segments = SegmentCommitment::<T>::count();

        u64::from(archived_segments) * ArchivedHistorySegment::SIZE as u64
    }
}

/// Methods for the `ValidateUnsigned` implementation:
/// It restricts calls to `store_segment_header` to local calls (i.e. extrinsics generated on this
/// node) or that already in a block. This guarantees that only block authors can include root
/// blocks.
impl<T: Config> Pallet<T> {
    fn validate_segment_header(
        source: TransactionSource,
        segment_headers: &[SegmentHeader],
    ) -> TransactionValidity {
        // Discard segment header not coming from the local node
        if !matches!(
            source,
            TransactionSource::Local | TransactionSource::InBlock,
        ) {
            warn!(
                target: "runtime::subspace",
                "Rejecting segment header extrinsic because it is not local/in-block.",
            );

            return InvalidTransaction::Call.into();
        }

        check_segment_headers::<T>(segment_headers)?;

        ValidTransaction::with_tag_prefix("SubspaceSegmentHeader")
            // We assign the maximum priority for any segment header.
            .priority(TransactionPriority::MAX)
            // Should be included immediately into the current block (this is an inherent
            // extrinsic) with no exceptions.
            .longevity(0)
            // We don't propagate this. This can never be included on a remote node.
            .propagate(false)
            .build()
    }

    fn pre_dispatch_segment_header(
        segment_headers: &[SegmentHeader],
    ) -> Result<(), TransactionValidityError> {
        check_segment_headers::<T>(segment_headers)
    }
}

fn check_segment_headers<T: Config>(
    segment_headers: &[SegmentHeader],
) -> Result<(), TransactionValidityError> {
    let mut segment_headers_iter = segment_headers.iter();

    // There should be some segment headers
    let first_segment_header = match segment_headers_iter.next() {
        Some(first_segment_header) => first_segment_header,
        None => {
            return Err(InvalidTransaction::BadMandatory.into());
        }
    };

    // Segment in segment headers should monotonically increase
    if first_segment_header.segment_index() > SegmentIndex::ZERO
        && !SegmentCommitment::<T>::contains_key(
            first_segment_header.segment_index() - SegmentIndex::ONE,
        )
    {
        return Err(InvalidTransaction::BadMandatory.into());
    }

    // Segment headers should never repeat
    if SegmentCommitment::<T>::contains_key(first_segment_header.segment_index()) {
        return Err(InvalidTransaction::BadMandatory.into());
    }

    let mut last_segment_index = first_segment_header.segment_index();

    for segment_header in segment_headers_iter {
        let segment_index = segment_header.segment_index();

        // Segment in segment headers should monotonically increase
        if segment_index != last_segment_index + SegmentIndex::ONE {
            return Err(InvalidTransaction::BadMandatory.into());
        }

        // Segment headers should never repeat
        if SegmentCommitment::<T>::contains_key(segment_index) {
            return Err(InvalidTransaction::BadMandatory.into());
        }

        last_segment_index = segment_index;
    }

    Ok(())
}

impl<T: Config> subspace_runtime_primitives::RewardsEnabled for Pallet<T> {
    fn rewards_enabled() -> bool {
        if let Some(height) = EnableRewards::<T>::get() {
            frame_system::Pallet::<T>::current_block_number() >= height
        } else {
            false
        }
    }
}

impl<T: Config> subspace_runtime_primitives::FindBlockRewardAddress<T::AccountId> for Pallet<T> {
    fn find_block_reward_address() -> Option<T::AccountId> {
        CurrentBlockAuthorInfo::<T>::get().and_then(
            |(_public_key, _sector_index, _piece_offset, _chunk, _slot, reward_address)| {
                // Rewards might be disabled, in which case no block reward
                if let Some(height) = EnableRewards::<T>::get() {
                    if frame_system::Pallet::<T>::current_block_number() >= height {
                        return reward_address;
                    }
                }

                None
            },
        )
    }
}

impl<T: Config> frame_support::traits::Randomness<T::Hash, BlockNumberFor<T>> for Pallet<T> {
    fn random(subject: &[u8]) -> (T::Hash, BlockNumberFor<T>) {
        let mut subject = subject.to_vec();
        subject.extend_from_slice(
            BlockRandomness::<T>::get()
                .expect("Block randomness is always set in block initialization; qed")
                .as_ref(),
        );

        (
            T::Hashing::hash(&subject),
            frame_system::Pallet::<T>::current_block_number(),
        )
    }

    fn random_seed() -> (T::Hash, BlockNumberFor<T>) {
        (
            T::Hashing::hash(
                BlockRandomness::<T>::get()
                    .expect("Block randomness is always set in block initialization; qed")
                    .as_ref(),
            ),
            frame_system::Pallet::<T>::current_block_number(),
        )
    }
}
