//! Block-related primitives

/// Block number in Subspace network
pub type BlockNumber = u64;
/// Block hash in Subspace network
pub type BlockHash = [u8; 32];
/// BlockWeight type for fork choice rule.
///
/// The smaller the solution range is, the heavier is the block.
pub type BlockWeight = u128;
