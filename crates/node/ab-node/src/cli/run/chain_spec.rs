use ab_client_consensus_common::{ConsensusConstants, PotConsensusConstants};
use ab_core_primitives::block::header::{
    BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderEd25519Seal,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotOutput, SlotDuration, SlotNumber};
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::shard::{NumShards, ShardIndex};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

const CONSENSUS_CONSTANTS: ConsensusConstants = ConsensusConstants {
    confirmation_depth_k: BlockNumber::new(100),
    block_authoring_delay: SlotNumber::new(4),
    pot: PotConsensusConstants {
        entropy_injection_interval: BlockNumber::new(50),
        entropy_injection_lookback_depth: 2,
        entropy_injection_delay: SlotNumber::new(15),
    },
    retarget_interval: BlockNumber::new(300),
    slot_probability: (1, 6),
    slot_duration: SlotDuration::from_millis(1000),
    recent_segments: HistorySize::new(NonZeroU64::new(5).expect("Not zero; qed")),
    recent_history_fraction: (
        HistorySize::new(NonZeroU64::new(1).expect("Not zero; qed")),
        HistorySize::new(NonZeroU64::new(10).expect("Not zero; qed")),
    ),
    min_sector_lifetime: HistorySize::new(NonZeroU64::new(4).expect("Not zero; qed")),
    max_block_timestamp_drift: BlockTimestamp::from_millis(30_000),
    // TODO: Reduced values just for testing to hit potential bugs sooner
    // shard_rotation_interval: SlotNumber::new(3600),
    shard_rotation_interval: SlotNumber::new(36),
    // TODO: Reduced values just for testing to hit potential bugs sooner
    // shard_rotation_delay: SlotNumber::new(1800),
    shard_rotation_delay: SlotNumber::new(18),
};

const _: () = {
    assert!(CONSENSUS_CONSTANTS.shard_rotation_interval.as_u64() > 0);
};

// TODO: Placeholder data structure, should probably be replaced with something else
pub(super) struct ChainSpec {
    // TODO
}

// TODO: Think harder about API here
impl ChainSpec {
    pub(super) fn new() -> Self {
        Self {}
    }

    pub(super) fn name(&self) -> &str {
        // TODO: Proper name
        "dev"
    }

    pub(super) fn consensus_constants(&self) -> &ConsensusConstants {
        &CONSENSUS_CONSTANTS
    }

    // TODO: Should PoT external entropy be in consensus constants?
    pub(super) fn pot_external_entropy(&self) -> Option<&[u8]> {
        // TODO: Proper value
        None
    }

    pub(super) fn genesis_block(&self) -> OwnedBeaconChainBlock {
        // TODO: Constants need to be mixed into the genesis block somehow, such that they impact
        //  genesis hash
        OwnedBeaconChainBlock::init([].into_iter(), [].into_iter(), &[])
            .expect("Values of the genesis block are valid; qed")
            .with_header(
                &BlockHeaderPrefix {
                    number: BlockNumber::ZERO,
                    shard_index: ShardIndex::BEACON_CHAIN,
                    padding_0: [0; _],
                    timestamp: BlockTimestamp::default(),
                    parent_root: BlockRoot::default(),
                    mmr_root: Blake3Hash::default(),
                },
                // TODO: Genesis state root must be the result of genesis block execution
                Blake3Hash::default(),
                &BlockHeaderConsensusInfo {
                    slot: SlotNumber::ZERO,
                    proof_of_time: PotOutput::default(),
                    future_proof_of_time: PotOutput::default(),
                    solution: Solution::genesis_solution(),
                },
                &BlockHeaderConsensusParameters {
                    fixed_parameters: BlockHeaderFixedConsensusParameters {
                        // TODO: Genesis solution range should come from the chain spec
                        solution_range: SolutionRange::from_pieces(
                            1000,
                            CONSENSUS_CONSTANTS.slot_probability,
                        ),
                        // TODO: Genesis slot iterations should come from the chain spec
                        // About 1s on 6.2 GHz Raptor Lake CPU (14900KS)
                        // slot_iterations: NonZeroU32::new(206_557_520).expect("Not zero; qed"),
                        slot_iterations: NonZeroU32::new(256).expect("Not zero; qed"),
                        // TODO: Initial number of shards should come from the chain spec
                        num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN)
                            .expect("Values are statically known to be valid; qed"),
                    },
                    super_segment_root: None,
                    next_solution_range: None,
                    pot_parameters_change: None,
                },
            )
            .expect("Values of the genesis block are valid; qed")
            .with_seal(BlockHeaderSeal::Ed25519(&BlockHeaderEd25519Seal {
                public_key: Default::default(),
                signature: Default::default(),
            }))
    }
}
