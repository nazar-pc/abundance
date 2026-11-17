//! Infos of child shard blocks can't be created from separate parts, and their fields can't be
//! modified after they are created by one of their parsers, which check that the parts correspond
//! to each other

use ab_core_primitives::block::body::{IntermediateShardBlockInfo, LeafShardBlockInfo, OwnSegments};
use ab_core_primitives::block::header::{IntermediateShardHeader, LeafShardHeader};
use ab_core_primitives::hashes::Blake3Hash;

fn create_leaf_shard_block_info<'a>(
    header: LeafShardHeader<'a>,
    transactions_root: &'a Blake3Hash,
    own_segments: Option<OwnSegments<'a>>,
) -> LeafShardBlockInfo<'a> {
    LeafShardBlockInfo {
        header,
        transactions_root,
        own_segments,
    }
}

fn modify_intermediate_shard_block_info<'a>(
    info: &mut IntermediateShardBlockInfo<'a>,
    header: IntermediateShardHeader<'a>,
    own_segments: Option<OwnSegments<'a>>,
) {
    info.header = header;
    info.own_segments = own_segments;
}

fn modify_leaf_shard_block_info<'a>(
    info: &mut LeafShardBlockInfo<'a>,
    header: LeafShardHeader<'a>,
    transactions_root: &'a Blake3Hash,
    own_segments: Option<OwnSegments<'a>>,
) {
    info.header = header;
    info.transactions_root = transactions_root;
    info.own_segments = own_segments;
}

fn main() {}
