use ab_client_api::{BlockDetails, ChainInfo, ReadBlockError};
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::header::{
    BlockHeaderConsensusInfo, BlockHeaderConsensusParameters, BlockHeaderEd25519Seal,
    BlockHeaderFixedConsensusParameters, BlockHeaderPrefix, BlockHeaderResult, BlockHeaderSeal,
};
use ab_core_primitives::block::owned::OwnedBeaconChainBlock;
use ab_core_primitives::block::{BlockNumber, BlockRoot, BlockTimestamp};
use ab_core_primitives::ed25519::{Ed25519PublicKey, Ed25519Signature};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pot::{PotOutput, SlotNumber};
use ab_core_primitives::segments::{LocalSegmentIndex, SegmentHeader};
use ab_core_primitives::shard::{NumShards, ShardIndex};
use ab_core_primitives::solutions::{Solution, SolutionRange};
use std::collections::HashMap;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::mpsc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;
use std::{panic, thread};

/// Minimal beacon chain header with the block number as the slot and proof of time `[salt; _]`,
/// `salt` makes headers with the same number and parent unique
pub(crate) fn beacon_chain_header(
    number: u64,
    parent_root: BlockRoot,
    salt: u8,
) -> OwnedBeaconChainHeader {
    OwnedBeaconChainHeader::from_parts(
        &BlockHeaderPrefix {
            number: BlockNumber::from(number),
            shard_index: ShardIndex::BEACON_CHAIN,
            padding_0: [0; _],
            timestamp: BlockTimestamp::default(),
            parent_root,
            mmr_root: Blake3Hash::default(),
        },
        &BlockHeaderResult {
            body_root: Blake3Hash::default(),
            state_root: Blake3Hash::default(),
        },
        &BlockHeaderConsensusInfo {
            slot: SlotNumber::from(number),
            proof_of_time: PotOutput::from([salt; _]),
            future_proof_of_time: PotOutput::default(),
            solution: Solution::genesis_solution(),
        },
        &Blake3Hash::default(),
        &BlockHeaderConsensusParameters {
            fixed_parameters: BlockHeaderFixedConsensusParameters {
                solution_range: SolutionRange::MAX,
                slot_iterations: NonZeroU32::MIN,
                num_shards: NumShards::new(NonZeroU16::MIN, NonZeroU16::MIN).unwrap(),
            },
            super_segment_root: None,
            next_solution_range: None,
            pot_parameters_change: None,
        },
    )
    .with_seal(BlockHeaderSeal::Ed25519(&BlockHeaderEd25519Seal {
        public_key: Ed25519PublicKey::default(),
        signature: Ed25519Signature::default(),
    }))
}

/// Mock of [`ChainInfo`] with headers of persisted blocks.
///
/// Methods that are not needed by tests are not implemented.
#[derive(Debug, Clone, Default)]
pub(crate) struct MockChainInfo {
    headers: HashMap<BlockRoot, OwnedBeaconChainHeader>,
}

impl ChainInfo<OwnedBeaconChainBlock> for MockChainInfo {
    fn best_root(&self) -> BlockRoot {
        unimplemented!()
    }

    fn best_header(&self) -> OwnedBeaconChainHeader {
        unimplemented!()
    }

    fn best_header_with_details(&self) -> (OwnedBeaconChainHeader, BlockDetails) {
        unimplemented!()
    }

    fn ancestor_header(
        &self,
        ancestor_block_number: BlockNumber,
        descendant_block_root: &BlockRoot,
    ) -> Option<OwnedBeaconChainHeader> {
        let mut block_root = *descendant_block_root;
        while let Some(header) = self.headers.get(&block_root)
            && header.header().prefix.number >= ancestor_block_number
        {
            if header.header().prefix.number == ancestor_block_number {
                return Some(header.clone());
            }
            block_root = header.header().prefix.parent_root;
        }

        None
    }

    fn header(&self, _block_root: &BlockRoot) -> Option<OwnedBeaconChainHeader> {
        unimplemented!()
    }

    fn header_with_details(
        &self,
        _block_root: &BlockRoot,
    ) -> Option<(OwnedBeaconChainHeader, BlockDetails)> {
        unimplemented!()
    }

    async fn block(
        &self,
        _block_root: &BlockRoot,
    ) -> Result<OwnedBeaconChainBlock, ReadBlockError> {
        unimplemented!()
    }

    fn last_segment_header(&self) -> Option<SegmentHeader> {
        unimplemented!()
    }

    fn get_segment_header(&self, _segment_index: LocalSegmentIndex) -> Option<SegmentHeader> {
        unimplemented!()
    }

    fn segment_headers_for_block(&self, _block_number: BlockNumber) -> Vec<SegmentHeader> {
        unimplemented!()
    }
}

impl MockChainInfo {
    /// Add a header of a persisted block and return its root
    pub(crate) fn add_header(&mut self, header: OwnedBeaconChainHeader) -> BlockRoot {
        let block_root = *header.header().root();
        self.headers.insert(block_root, header);
        block_root
    }
}

/// Run `function` on another thread and panic if it doesn't finish within 10 seconds, so a test of
/// code that may loop forever fails instead of hanging
pub(crate) fn with_timeout<F, T>(function: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let (result_sender, result_receiver) = mpsc::channel();
    let join_handle = thread::spawn(move || {
        // The receiver is gone only after the timeout, which already fails the test
        let _: Result<(), _> = result_sender.send(function());
    });

    match result_receiver.recv_timeout(Duration::from_secs(10)) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("Didn't finish within the timeout, probably loops forever")
        }
        // Report the panic of `function` instead of a timeout
        Err(RecvTimeoutError::Disconnected) => panic::resume_unwind(
            join_handle
                .join()
                .expect_err("The sender is only dropped without sending when `function` panicked"),
        ),
    }
}
