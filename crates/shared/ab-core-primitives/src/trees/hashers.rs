use ab_blake3::KEY_LEN;
use ab_merkle_tree::NewMerkleHasherForTreeSize;
use const_fn_specialization::const_fn_specialization;

/// Keyed BLAKE3 hasher of inner nodes with [`Self::KEY`], which doesn't depend on the tree size.
///
/// Used for trees that don't know the number of leaves upfront and for sparse trees.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Blake3StaticMerkleHasher<const KEY: [u8; KEY_LEN]>;

const impl<const KEY: [u8; KEY_LEN]> Default for Blake3StaticMerkleHasher<KEY> {
    #[inline(always)]
    fn default() -> Self {
        Self
    }
}

impl<const KEY: [u8; KEY_LEN]> Blake3StaticMerkleHasher<KEY> {
    /// Key of keyed BLAKE3 hash
    pub const KEY: [u8; KEY_LEN] = KEY;

    #[inline(always)]
    pub(super) fn node_key(&self) -> &[u8; KEY_LEN] {
        &KEY
    }
}

/// Keyed BLAKE3 hasher of inner nodes with [`Self::key()`], which commits to the tree size.
///
/// Used for trees that know the number of leaves upfront.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Blake3SizedMerkleHasher<const BASE_KEY: [u8; KEY_LEN]> {
    key: [u8; KEY_LEN],
}

const impl<const BASE_KEY: [u8; KEY_LEN]> NewMerkleHasherForTreeSize
    for Blake3SizedMerkleHasher<BASE_KEY>
{
    #[inline(always)]
    fn new_for_tree_size(num_leaves: u64) -> Self {
        Self {
            key: Self::key(num_leaves),
        }
    }
}

#[const_fn_specialization]
impl<const BASE_KEY: [u8; KEY_LEN]> Blake3SizedMerkleHasher<BASE_KEY> {
    pub(super) const BASE_KEY: [u8; KEY_LEN] = BASE_KEY;

    /// Key of keyed BLAKE3 hash for a tree with `num_leaves` leaves
    #[inline(always)]
    pub const fn key(num_leaves: u64) -> [u8; KEY_LEN] {
        ab_blake3::const_keyed_hash(&BASE_KEY, &num_leaves.to_le_bytes())
    }

    /// Key of keyed BLAKE3 hash for a tree with `num_leaves` leaves
    #[inline(always)]
    pub fn key(num_leaves: u64) -> [u8; KEY_LEN] {
        ab_blake3::single_block_keyed_hash(&BASE_KEY, &num_leaves.to_le_bytes())
            .expect("Less than a single block worth of bytes; qed")
    }

    #[inline(always)]
    pub(super) fn node_key(&self) -> &[u8; KEY_LEN] {
        &self.key
    }
}
