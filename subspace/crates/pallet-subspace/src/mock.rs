//! Test utilities

use crate::{self as pallet_subspace, AllowAuthoringBy, Config, ConsensusConstants};
use frame_support::traits::{ConstU128, OnInitialize};
use frame_support::{derive_impl, parameter_types};
use schnorrkel::Keypair;
use sp_consensus_slots::Slot;
use sp_consensus_subspace::digests::{CompatibleDigestItem, PreDigest, PreDigestPotInfo};
use sp_io::TestExternalities;
use sp_runtime::testing::{Digest, DigestItem, TestXt};
use sp_runtime::BuildStorage;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::Once;
use subspace_core_primitives::hashes::Blake3Hash;
use subspace_core_primitives::pieces::PieceOffset;
use subspace_core_primitives::segments::{
    ArchivedBlockProgress, HistorySize, LastArchivedBlock, SegmentHeader, SegmentIndex, SegmentRoot,
};
use subspace_core_primitives::solutions::{Solution, SolutionRange};
use subspace_runtime_primitives::ConsensusEventSegmentSize;
use subspace_verification::sr25519::PublicKey;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

frame_support::construct_runtime!(
    pub struct Test {
        System: frame_system,
        Balances: pallet_balances,
        Subspace: pallet_subspace,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
    type EventSegmentSize = ConsensusEventSegmentSize;
}

impl<C> frame_system::offchain::CreateTransactionBase<C> for Test
where
    RuntimeCall: From<C>,
{
    type RuntimeCall = RuntimeCall;
    type Extrinsic = TestXt<RuntimeCall, ()>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = Balance;
    type ExistentialDeposit = ConstU128<1>;
    type AccountStore = System;
    type RuntimeHoldReason = ();
    type DustRemoval = ();
}

/// 1 in 6 slots (on average, not counting collisions) will have a block.
pub const SLOT_PROBABILITY: (u64, u64) = (3, 10);

// 1GiB
pub const INITIAL_SOLUTION_RANGE: SolutionRange =
    SolutionRange::from_pieces(1024, SLOT_PROBABILITY);

parameter_types! {
    pub const MockConsensusConstants: ConsensusConstants<u64> = ConsensusConstants {
        pot_entropy_injection_interval: 5,
        pot_entropy_injection_lookback_depth: 2,
        pot_entropy_injection_delay: 4,
        era_duration: 4,
        slot_probability: SLOT_PROBABILITY,
    };
}

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type SubspaceOrigin = pallet_subspace::EnsureSubspaceOrigin;
    type ConsensusConstants = MockConsensusConstants;
    type WeightInfo = ();
    type ExtensionWeightInfo = crate::extensions::weights::SubstrateWeight<Test>;
}

pub fn go_to_block(keypair: &Keypair, block: u64, slot: u64) {
    use frame_support::traits::OnFinalize;

    Subspace::on_finalize(System::block_number());

    let parent_hash = if System::block_number() > 1 {
        let header = System::finalize();
        header.hash()
    } else {
        System::parent_hash()
    };

    let chunk = Default::default();

    let pre_digest = make_pre_digest(
        slot.into(),
        Solution {
            public_key_hash: PublicKey::from(keypair.public.to_bytes()).hash(),
            sector_index: 0,
            history_size: HistorySize::from(SegmentIndex::ZERO),
            piece_offset: PieceOffset::default(),
            record_root: Default::default(),
            record_proof: Default::default(),
            chunk,
            chunk_proof: Default::default(),
            proof_of_space: Default::default(),
        },
    );

    System::reset_events();
    System::initialize(&block, &parent_hash, &pre_digest);

    Subspace::on_initialize(block);
}

/// Slots will grow accordingly to blocks
pub fn progress_to_block(keypair: &Keypair, n: u64) {
    let mut slot = u64::from(Subspace::current_slot()) + 1;
    for i in System::block_number() + 1..=n {
        go_to_block(keypair, i, slot);
        slot += 1;
    }
}

pub fn make_pre_digest(slot: Slot, solution: Solution) -> Digest {
    let log = DigestItem::subspace_pre_digest(&PreDigest::V0 {
        slot,
        solution,
        pot_info: PreDigestPotInfo::V0 {
            proof_of_time: Default::default(),
            future_proof_of_time: Default::default(),
        },
    });
    Digest { logs: vec![log] }
}

pub fn new_test_ext() -> TestExternalities {
    static INITIALIZE_LOGGER: Once = Once::new();
    INITIALIZE_LOGGER.call_once(|| {
        let _ = env_logger::try_init_from_env(env_logger::Env::new().default_filter_or("error"));
    });

    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    pallet_subspace::GenesisConfig::<Test> {
        allow_authoring_by: AllowAuthoringBy::Anyone,
        pot_slot_iterations: NonZeroU32::new(100_000).unwrap(),
        initial_solution_range: INITIAL_SOLUTION_RANGE,
        phantom: PhantomData,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    TestExternalities::from(storage)
}

pub fn create_segment_header(segment_index: SegmentIndex) -> SegmentHeader {
    SegmentHeader::V0 {
        segment_index,
        segment_root: SegmentRoot::default(),
        prev_segment_header_hash: Blake3Hash::default(),
        last_archived_block: LastArchivedBlock {
            number: 0,
            archived_progress: ArchivedBlockProgress::Complete,
        },
    }
}
