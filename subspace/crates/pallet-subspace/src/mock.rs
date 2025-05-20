//! Test utilities

use crate::{self as pallet_subspace, AllowAuthoringBy, Config, ConsensusConstants};
use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::PieceOffset;
use ab_core_primitives::pot::SlotNumber;
use ab_core_primitives::sectors::SectorIndex;
use ab_core_primitives::segments::{
    ArchivedBlockProgress, HistorySize, LastArchivedBlock, SegmentHeader, SegmentIndex, SegmentRoot,
};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use frame_support::traits::{ConstU128, OnInitialize};
use frame_support::{derive_impl, parameter_types};
use schnorrkel::Keypair;
use sp_consensus_subspace::digests::{CompatibleDigestItem, PreDigest, PreDigestPotInfo};
use sp_io::TestExternalities;
use sp_runtime::BuildStorage;
use sp_runtime::testing::{Digest, DigestItem, TestXt};
use std::marker::PhantomData;
use std::num::NonZeroU32;
use subspace_runtime_primitives::ConsensusEventSegmentSize;
use subspace_verification::ed25519::Ed25519PublicKey;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

frame_support::construct_runtime!(
    pub struct Test {
        System: frame_system = 0,
        Balances: pallet_balances = 1,
        // TODO: Should have been 3, but runtime thinks "2" is already occupied by `Void` 🤷
        Subspace: pallet_subspace = 3,
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
        pot_entropy_injection_delay: SlotNumber::new(4),
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

pub fn go_to_block(keypair: &Keypair, block: u64, slot: SlotNumber) {
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
        slot,
        Solution {
            public_key_hash: Ed25519PublicKey::from(keypair.public.to_bytes()).hash(),
            record_root: Default::default(),
            record_proof: Default::default(),
            chunk,
            chunk_proof: Default::default(),
            proof_of_space: Default::default(),
            history_size: HistorySize::from(SegmentIndex::ZERO),
            sector_index: SectorIndex::ZERO,
            piece_offset: PieceOffset::default(),
            padding: [0; _],
        },
    );

    System::reset_events();
    System::initialize(&block, &parent_hash, &pre_digest);

    Subspace::on_initialize(block);
}

/// Slots will grow accordingly to blocks
pub fn progress_to_block(keypair: &Keypair, n: u64) {
    let mut slot = Subspace::current_slot() + SlotNumber::ONE;
    for i in System::block_number() + 1..=n {
        go_to_block(keypair, i, slot);
        slot += SlotNumber::ONE;
    }
}

pub fn make_pre_digest(slot: SlotNumber, solution: Solution) -> Digest {
    let log = DigestItem::subspace_pre_digest(&PreDigest {
        slot,
        solution,
        pot_info: PreDigestPotInfo {
            proof_of_time: Default::default(),
            future_proof_of_time: Default::default(),
        },
    });
    Digest { logs: vec![log] }
}

pub fn new_test_ext() -> TestExternalities {
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
    SegmentHeader {
        segment_index: segment_index.into(),
        segment_root: SegmentRoot::default(),
        prev_segment_header_hash: Blake3Hash::default(),
        last_archived_block: LastArchivedBlock {
            number: BlockNumber::ZERO.into(),
            archived_progress: ArchivedBlockProgress::new_complete(),
        },
    }
}
