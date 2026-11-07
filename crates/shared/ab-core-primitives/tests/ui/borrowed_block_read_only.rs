//! Borrowed blocks can't be created from a separate header and body, and their fields can't be
//! modified after they are created by one of their constructors, which ensure that the header
//! corresponds to the body

use ab_core_primitives::block::body::{BeaconChainBody, IntermediateShardBody, LeafShardBody};
use ab_core_primitives::block::header::{
    BeaconChainHeader, IntermediateShardHeader, LeafShardHeader,
};
use ab_core_primitives::block::{BeaconChainBlock, IntermediateShardBlock, LeafShardBlock};

fn create_beacon_chain_block<'a>(
    header: BeaconChainHeader<'a>,
    body: BeaconChainBody<'a>,
) -> BeaconChainBlock<'a> {
    BeaconChainBlock { header, body }
}

fn create_intermediate_shard_block<'a>(
    header: IntermediateShardHeader<'a>,
    body: IntermediateShardBody<'a>,
) -> IntermediateShardBlock<'a> {
    IntermediateShardBlock { header, body }
}

fn create_leaf_shard_block<'a>(
    header: LeafShardHeader<'a>,
    body: LeafShardBody<'a>,
) -> LeafShardBlock<'a> {
    LeafShardBlock { header, body }
}

fn modify_beacon_chain_block<'a>(
    block: &mut BeaconChainBlock<'a>,
    header: BeaconChainHeader<'a>,
    body: BeaconChainBody<'a>,
) {
    block.header = header;
    block.body = body;
}

fn modify_intermediate_shard_block<'a>(
    block: &mut IntermediateShardBlock<'a>,
    header: IntermediateShardHeader<'a>,
    body: IntermediateShardBody<'a>,
) {
    block.header = header;
    block.body = body;
}

fn modify_leaf_shard_block<'a>(
    block: &mut LeafShardBlock<'a>,
    header: LeafShardHeader<'a>,
    body: LeafShardBody<'a>,
) {
    block.header = header;
    block.body = body;
}

fn main() {}
