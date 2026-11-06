//! Owned blocks can't be created from a separate header and body, and their fields can't be
//! modified after they are created by one of their constructors, which ensure that the header
//! corresponds to the body

use ab_core_primitives::block::body::owned::{
    OwnedBeaconChainBody, OwnedIntermediateShardBody, OwnedLeafShardBody,
};
use ab_core_primitives::block::header::owned::{
    OwnedBeaconChainHeader, OwnedIntermediateShardHeader, OwnedLeafShardHeader,
};
use ab_core_primitives::block::owned::{
    OwnedBeaconChainBlock, OwnedIntermediateShardBlock, OwnedLeafShardBlock,
};

fn create_beacon_chain_block(
    header: OwnedBeaconChainHeader,
    body: OwnedBeaconChainBody,
) -> OwnedBeaconChainBlock {
    OwnedBeaconChainBlock { header, body }
}

fn create_intermediate_shard_block(
    header: OwnedIntermediateShardHeader,
    body: OwnedIntermediateShardBody,
) -> OwnedIntermediateShardBlock {
    OwnedIntermediateShardBlock { header, body }
}

fn create_leaf_shard_block(
    header: OwnedLeafShardHeader,
    body: OwnedLeafShardBody,
) -> OwnedLeafShardBlock {
    OwnedLeafShardBlock { header, body }
}

fn modify_beacon_chain_block(
    block: &mut OwnedBeaconChainBlock,
    header: OwnedBeaconChainHeader,
    body: OwnedBeaconChainBody,
) {
    block.header = header;
    block.body = body;
}

fn modify_intermediate_shard_block(
    block: &mut OwnedIntermediateShardBlock,
    header: OwnedIntermediateShardHeader,
    body: OwnedIntermediateShardBody,
) {
    block.header = header;
    block.body = body;
}

fn modify_leaf_shard_block(
    block: &mut OwnedLeafShardBlock,
    header: OwnedLeafShardHeader,
    body: OwnedLeafShardBody,
) {
    block.header = header;
    block.body = body;
}

fn main() {}
