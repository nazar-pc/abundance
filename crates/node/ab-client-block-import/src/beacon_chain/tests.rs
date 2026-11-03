use crate::beacon_chain::VerificationChainInfo;
use crate::importing_blocks::{ImportingBlockHandle, ImportingBlocks};
use crate::test_utils::{MockChainInfo, beacon_chain_header, with_timeout};
use ab_client_consensus_common::consensus_parameters::{
    DeriveConsensusParametersChainInfo, ShardMembershipEntropySourceChainInfo,
};
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::block::{BlockNumber, BlockRoot};
use ab_core_primitives::pot::{PotOutput, SlotNumber};
use ab_core_primitives::trees::BlockMerkleMountainRange;
use triomphe::Arc;

type ImportingHandles = Vec<ImportingBlockHandle<OwnedBeaconChainHeader>>;

/// Start importing a block with `header` and return its root, the block stays in
/// `importing_blocks` while the handle in `importing_handles` exists
fn start_importing(
    importing_blocks: &ImportingBlocks<OwnedBeaconChainHeader>,
    importing_handles: &mut ImportingHandles,
    header: OwnedBeaconChainHeader,
) -> BlockRoot {
    let block_root = *header.header().root();
    importing_handles.push(
        importing_blocks
            .insert(header, Arc::new(BlockMerkleMountainRange::new()))
            .unwrap(),
    );

    block_root
}

/// `genesis - c1 - c2 (persisted) - c3 - c4 - c5 (being imported)`, the proof of time of each
/// block is `[block_number; _]`, returns roots of all blocks
fn chain() -> (
    MockChainInfo,
    ImportingBlocks<OwnedBeaconChainHeader>,
    ImportingHandles,
    [BlockRoot; 6],
) {
    let mut chain_info = MockChainInfo::default();
    let importing_blocks = ImportingBlocks::new();
    let mut importing_handles = Vec::new();

    let mut parent_root = BlockRoot::default();
    let block_roots = [0, 1, 2, 3, 4, 5].map(|number| {
        let header = beacon_chain_header(u64::from(number), parent_root, number);
        parent_root = if number <= 2 {
            chain_info.add_header(header)
        } else {
            start_importing(&importing_blocks, &mut importing_handles, header)
        };
        parent_root
    });

    (chain_info, importing_blocks, importing_handles, block_roots)
}

/// Proof of time of the block at `ancestor_block_number` on the chain ending at
/// `descendant_block_root`, looked up with both methods of [`VerificationChainInfo`], which must
/// agree
fn ancestor_proof_of_time(
    chain_info: &MockChainInfo,
    importing_blocks: &ImportingBlocks<OwnedBeaconChainHeader>,
    ancestor_block_number: u64,
    descendant_block_root: BlockRoot,
) -> Option<PotOutput> {
    let chain_info = chain_info.clone();
    let importing_blocks = importing_blocks.clone();

    with_timeout(move || {
        let verification_chain_info = VerificationChainInfo {
            chain_info: &chain_info,
            importing_blocks: &importing_blocks,
        };
        let ancestor_block_number = BlockNumber::from(ancestor_block_number);

        let consensus_info = verification_chain_info
            .ancestor_header_consensus_info(ancestor_block_number, &descendant_block_root);
        let proof_of_time = verification_chain_info
            .ancestor_header_proof_of_time(ancestor_block_number, &descendant_block_root);

        // Test headers have the block number as the slot
        assert_eq!(
            consensus_info.map(|consensus_info| consensus_info.slot),
            proof_of_time.map(|_| SlotNumber::from(u64::from(ancestor_block_number)))
        );
        assert_eq!(
            consensus_info.map(|consensus_info| consensus_info.proof_of_time),
            proof_of_time
        );

        proof_of_time
    })
}

#[test]
fn ancestor_lookup_finds_blocks_being_imported() {
    // genesis - c1 - c2 (persisted) - c3 - c4 - c5 (being imported)
    let (chain_info, importing_blocks, _importing_handles, [.., c5]) = chain();

    // Through two blocks being imported
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 3, c5),
        Some(PotOutput::from([3; _]))
    );
    // The descendant itself
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 5, c5),
        Some(PotOutput::from([5; _]))
    );
}

#[test]
fn ancestor_lookup_finds_persisted_blocks_below_blocks_being_imported() {
    // genesis - c1 - c2 (persisted) - c3 - c4 - c5 (being imported)
    let (chain_info, importing_blocks, _importing_handles, [.., c5]) = chain();

    // The parent of the deepest block being imported
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 2, c5),
        Some(PotOutput::from([2; _]))
    );
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 0, c5),
        Some(PotOutput::from([0; _]))
    );
}

#[test]
fn ancestor_lookup_misses() {
    // genesis - c1 - c2 (persisted) - c3 - c4 - c5 (being imported)
    //
    // a3 (unknown) - a4 (being imported)
    let (chain_info, importing_blocks, mut importing_handles, [.., c5]) = chain();
    let a3 = *beacon_chain_header(3, BlockRoot::default(), 103)
        .header()
        .root();
    let a4 = start_importing(
        &importing_blocks,
        &mut importing_handles,
        beacon_chain_header(4, a3, 104),
    );

    // Above the descendant
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 6, c5),
        None
    );
    // Below a block being imported with an unknown parent
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 2, a4),
        None
    );
    // Unknown descendant
    assert_eq!(
        ancestor_proof_of_time(&chain_info, &importing_blocks, 2, a3),
        None
    );
}
