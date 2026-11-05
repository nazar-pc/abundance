use crate::importing_blocks::ImportingBlocks;
use crate::test_utils::beacon_chain_header;
use ab_core_primitives::block::BlockRoot;
use ab_core_primitives::block::header::owned::OwnedBeaconChainHeader;
use ab_core_primitives::trees::BlockMerkleMountainRange;
use triomphe::Arc;

#[test]
fn duplicate_insert_keeps_original_entry() {
    let importing_blocks = ImportingBlocks::<OwnedBeaconChainHeader>::new();
    let header = beacon_chain_header(1, BlockRoot::default(), 1);
    let block_root = *header.header().root();

    let importing_handle = importing_blocks
        .insert(header.clone(), Arc::new(BlockMerkleMountainRange::new()))
        .unwrap();
    assert!(
        importing_blocks
            .insert(header, Arc::new(BlockMerkleMountainRange::new()))
            .is_none()
    );

    // The original entry is still there while its import is in progress
    let entry = importing_blocks.get(&block_root).unwrap();
    assert!(Arc::ptr_eq(&entry.inner, &importing_handle.inner));

    drop(importing_handle);
    assert!(importing_blocks.get(&block_root).is_none());
}
