use crate::batching::{BATCH_NUM_LEAVES, PARALLEL_PAIRS, hash_pairs, perfect_subtree_root};
use crate::{MerkleHasher, NewMerkleHasherForTreeSize};
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::iter::TrustedLen;
use core::mem::MaybeUninit;
use core::num::NonZero;

const TREE_SIZE_WITHOUT_LEAVES<const N: usize>: usize = {
    // Ensuring only supported `N` can be specified for [`BalancedMerkleTree`]
    assert!(
        N.is_power_of_two(),
        "Balanced Merkle Tree must have a number of leaves that is a power of 2"
    );

    assert!(
        N > 1,
        "This Balanced Merkle Tree must have more than one leaf"
    );

    N - 1
};
/// Number of elements in a proof for a tree with `N` leaves
pub const PROOF_ELEMENTS<const N: usize>: usize = N.ilog2() as usize;
const ROOT_ONLY_LARGE_STACK_SIZE<const N: usize, H: MerkleHasher>: usize = {
    // For small trees the large stack is not used, so the returned value does not matter as long as
    // it compiles
    if N < BATCH_NUM_LEAVES::<H> {
        1
    } else {
        (N / BATCH_NUM_LEAVES::<H>).ilog2() as usize + 1
    }
};

/// Merkle Tree variant that has hash-sized leaves and is fully balanced according to configured
/// generic parameter.
///
/// This can be considered a general case of [`UnbalancedMerkleTree`]. The root and proofs are
/// identical for both in case the number of leaves is a power of two. For the number of leaves that
/// is a power of two [`UnbalancedMerkleTree`] is useful when a single proof needs to be generated
/// and the number of leaves is very large (it can generate proofs with very little RAM usage
/// compared to this version).
///
/// [`UnbalancedMerkleTree`]: crate::unbalanced::UnbalancedMerkleTree
///
/// This Merkle Tree implementation is best suited for use cases when proofs for all (or most) of
/// the elements need to be generated and the whole tree easily fits into memory. It can also be
/// constructed and proofs can be generated efficiently without heap allocations.
///
/// With all parameters of the tree known statically, it results in the most efficient version of
/// the code being generated for a given set of parameters.
///
/// Inner nodes are hashed with `H`.
#[derive(Debug)]
pub struct BalancedMerkleTree<'a, const N: usize, H>
where
    H: MerkleHasher,
{
    leaves: &'a [[u8; H::OUT_LEN]; N],
    // This tree doesn't include leaves because we have them in `leaves` field
    tree: [[u8; H::OUT_LEN]; TREE_SIZE_WITHOUT_LEAVES::<N>],
}

impl<'a, const N: usize, H> BalancedMerkleTree<'a, N, H>
where
    [(); TREE_SIZE_WITHOUT_LEAVES::<N>]:,
    H: MerkleHasher + const NewMerkleHasherForTreeSize,
{
    /// Create a new tree from a fixed set of elements.
    ///
    /// The data structure is statically allocated and might be too large to fit on the stack!
    /// If that is the case, use `new_boxed()` method.
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new(leaves: &'a [[u8; H::OUT_LEN]; N]) -> Self {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { TREE_SIZE_WITHOUT_LEAVES::<N> };

        let mut tree = [MaybeUninit::<[u8; H::OUT_LEN]>::uninit(); _];

        Self::init_internal(leaves, &mut tree);

        Self {
            leaves,
            // SAFETY: Statically guaranteed for all elements to be initialized
            tree: unsafe { tree.transpose().assume_init() },
        }
    }

    /// Like [`Self::new()`], but used pre-allocated memory for instantiation
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn new_in<'b>(
        instance: &'b mut MaybeUninit<Self>,
        leaves: &'a [[u8; H::OUT_LEN]; N],
    ) -> &'b mut Self {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { TREE_SIZE_WITHOUT_LEAVES::<N> };

        let instance_ptr = instance.as_mut_ptr();
        // SAFETY: Valid and correctly aligned non-null pointer
        unsafe {
            (&raw mut (*instance_ptr).leaves).write(leaves);
        }
        let tree = {
            // SAFETY: Valid and correctly aligned non-null pointer
            let tree_ptr = unsafe { &raw mut (*instance_ptr).tree };
            // SAFETY: Allocated and correctly aligned uninitialized data
            unsafe {
                tree_ptr
                    .cast::<[MaybeUninit<[u8; H::OUT_LEN]>; TREE_SIZE_WITHOUT_LEAVES::<N>]>()
                    .as_mut_unchecked()
            }
        };

        Self::init_internal(leaves, tree);

        // SAFETY: Initialized field by field above
        unsafe { instance.assume_init_mut() }
    }

    /// Like [`Self::new()`], but creates heap-allocated instance, avoiding excessive stack usage
    /// for large trees
    #[cfg(feature = "alloc")]
    pub fn new_boxed(leaves: &'a [[u8; H::OUT_LEN]; N]) -> Box<Self> {
        let mut instance = Box::<Self>::new_uninit();

        Self::new_in(&mut instance, leaves);

        // SAFETY: Initialized by constructor above
        unsafe { instance.assume_init() }
    }

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn init_internal(
        leaves: &[[u8; H::OUT_LEN]; N],
        tree: &mut [MaybeUninit<[u8; H::OUT_LEN]>; TREE_SIZE_WITHOUT_LEAVES::<N>],
    ) {
        let hasher = const { H::new_for_tree_size(N as u64) };
        let mut tree_hashes = tree.as_mut_slice();
        let mut level_hashes = leaves.as_slice();

        while level_hashes.len() > 1 {
            let num_pairs = level_hashes.len() / 2;
            let parent_hashes;
            // SAFETY: The size of the tree is statically known to match the number of leaves and
            // levels of hashes
            (parent_hashes, tree_hashes) = unsafe { tree_hashes.split_at_mut_unchecked(num_pairs) };

            let (parent_hashes_chunks, parent_hashes_remainder) =
                parent_hashes.as_chunks_mut::<{ PARALLEL_PAIRS::<H> }>();
            if parent_hashes_remainder.is_empty() {
                for (pairs, hashes) in level_hashes
                    .as_chunks::<{ BATCH_NUM_LEAVES::<H> }>()
                    .0
                    .iter()
                    .zip(parent_hashes_chunks)
                {
                    // TODO: This memory copy is unfortunate, make hashing write into this memory
                    //  directly once blake3 API improves
                    *hashes = MaybeUninit::new(hash_pairs(&hasher, pairs)).into();
                }
            } else {
                for (pair, parent_hash) in level_hashes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .zip(parent_hashes.iter_mut())
                {
                    parent_hash.write(hasher.hash_pair(pair));
                }
            }

            // SAFETY: Just initialized
            level_hashes = unsafe { parent_hashes.assume_init_ref() };
        }
    }

    // TODO: Method that generates not only root, but also proof, like Unbalanced Merkle Tree
    /// Compute Merkle Tree root.
    ///
    /// This is functionally equivalent to creating an instance first and calling [`Self::root()`]
    /// method, but is faster and avoids heap allocation when root is the only thing that is needed.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn compute_root_only(leaves: &[[u8; H::OUT_LEN]; N]) -> [u8; H::OUT_LEN] {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { TREE_SIZE_WITHOUT_LEAVES::<N> };

        let hasher = const { H::new_for_tree_size(N as u64) };

        // Trees smaller than a batch are hashed one level at a time
        if N < BATCH_NUM_LEAVES::<H> {
            let mut nodes = *leaves;
            // SAFETY: `N` is a power of two, which `TREE_SIZE_WITHOUT_LEAVES` checks
            return unsafe { perfect_subtree_root(&hasher, &mut nodes) };
        }

        // Stack of intermediate nodes per tree level. Each level holds a batch of hashes, the last
        // one is collapsed into a single root hash at the very end.
        let mut stack =
            [[[0u8; H::OUT_LEN]; PARALLEL_PAIRS::<H>]; ROOT_ONLY_LARGE_STACK_SIZE::<N, H>];

        // This variable allows reusing and reducing stack usage instead of having a separate
        // `current` variable
        let mut parent_current = [[0u8; H::OUT_LEN]; BATCH_NUM_LEAVES::<H>];
        for (num_chunks, chunk_leaves) in leaves
            .as_chunks::<{ BATCH_NUM_LEAVES::<H> }>()
            .0
            .iter()
            .enumerate()
        {
            let current_half = &mut parent_current[PARALLEL_PAIRS::<H>..];

            let current = hash_pairs::<{ PARALLEL_PAIRS::<H> }, _, _>(&hasher, chunk_leaves);
            current_half.copy_from_slice(&current);

            // Every bit set to `1` corresponds to an active Merkle Tree level
            let lowest_active_levels = num_chunks.trailing_ones() as usize;
            for parent in &mut stack[..lowest_active_levels] {
                let parent_half = &mut parent_current[..PARALLEL_PAIRS::<H>];
                parent_half.copy_from_slice(parent);

                let current = hash_pairs::<{ PARALLEL_PAIRS::<H> }, _, _>(&hasher, &parent_current);

                let current_half = &mut parent_current[PARALLEL_PAIRS::<H>..];
                current_half.copy_from_slice(&current);
            }

            let current_half = &mut parent_current[PARALLEL_PAIRS::<H>..];

            // Place a freshly computed batch of hashes into the first inactive level
            stack[lowest_active_levels].copy_from_slice(current_half);
        }

        // SAFETY: Each level of the stack holds `MerkleHasher::PARALLEL_PAIRS` nodes, which is a
        // power of two
        unsafe { perfect_subtree_root(&hasher, &mut stack[ROOT_ONLY_LARGE_STACK_SIZE::<N, H> - 1]) }
    }

    /// Get the root of Merkle Tree
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn root(&self) -> [u8; H::OUT_LEN] {
        *self
            .tree
            .last()
            .or(self.leaves.last())
            .expect("There is always at least one leaf hash; qed")
    }

    /// Iterator over proofs in the same order as provided leaf hashes
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn all_proofs(&self) -> ProofsIterator<'_, N, H> {
        ProofsIterator {
            leaves: self.leaves,
            tree: &self.tree,
            leaf_index: 0,
        }
    }

    /// Verify previously generated proof
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    pub fn verify(
        root: &[u8; H::OUT_LEN],
        proof: &[[u8; H::OUT_LEN]; PROOF_ELEMENTS::<N>],
        leaf_index: usize,
        leaf: [u8; H::OUT_LEN],
    ) -> bool {
        // TODO: Technically redundant with the where-clause, which is not enforced in generic code
        //  due to https://github.com/rust-lang/rust/issues/164019, remove once fixed
        const { TREE_SIZE_WITHOUT_LEAVES::<N> };

        if leaf_index >= N {
            return false;
        }

        let hasher = const { H::new_for_tree_size(N as u64) };
        let mut computed_root = leaf;

        let mut position = leaf_index;
        for hash in proof {
            computed_root = if position.is_multiple_of(2) {
                hasher.hash_pair(&[computed_root, *hash])
            } else {
                hasher.hash_pair(&[*hash, computed_root])
            };

            position /= 2;
        }

        root == &computed_root
    }
}

/// Iterator over proofs for a balanced Merkle tree
#[derive(Debug)]
pub struct ProofsIterator<'a, const N: usize, H>
where
    H: MerkleHasher,
{
    leaves: &'a [[u8; H::OUT_LEN]; N],
    tree: &'a [[u8; H::OUT_LEN]; TREE_SIZE_WITHOUT_LEAVES::<N>],
    /// Index of the next leaf, the iterator is exhausted once it reaches `N`
    leaf_index: usize,
}

impl<const N: usize, H> ProofsIterator<'_, N, H>
where
    H: MerkleHasher,
{
    #[inline(always)]
    fn remaining(&self) -> usize {
        N - self.leaf_index
    }
}

impl<const N: usize, H> Iterator for ProofsIterator<'_, N, H>
where
    H: MerkleHasher,
{
    type Item = [[u8; H::OUT_LEN]; PROOF_ELEMENTS::<N>];

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.leaf_index;

        // The line below is a more efficient branchless version of this:
        // let sibling_index = if index % 2 == 0 {
        //     index + 1
        // } else {
        //     index - 1
        // };
        let sibling_index = index ^ 1;
        // `N` is an even number, so the sibling is missing exactly when `index >= N`, meaning the
        // iterator is exhausted
        let sibling_hash = *self.leaves.get(sibling_index)?;
        self.leaf_index += 1;

        let mut proof = [MaybeUninit::<[u8; H::OUT_LEN]>::uninit(); _];
        proof[0].write(sibling_hash);

        // Part that is shared between left and right leaf proofs
        let shared_proof = &mut proof[1..];

        let mut tree_hashes = self.tree.as_slice();
        let mut parent_position = index / 2;
        let mut parent_level_size = N / 2;

        for hash in shared_proof {
            let parent_other_position = parent_position ^ 1;

            let other_hash = tree_hashes.get(parent_other_position).expect(
                "`parent_position` is within the current level of the tree, whose size is an \
                    even number; qed",
            );
            hash.write(*other_hash);
            tree_hashes = &tree_hashes[parent_level_size..];

            parent_position /= 2;
            parent_level_size /= 2;
        }

        // SAFETY: Just initialized
        Some(unsafe { proof.transpose().assume_init() })
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining();
        (remaining, Some(remaining))
    }

    #[inline(always)]
    fn count(self) -> usize {
        self.remaining()
    }

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn last(mut self) -> Option<Self::Item> {
        if self.leaf_index >= N {
            return None;
        }
        self.leaf_index = N - 1;
        self.next()
    }

    #[inline(always)]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let advance = n.min(self.remaining());
        self.leaf_index += advance;
        NonZero::new(n - advance).map_or(Ok(()), Err)
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self.advance_by(n) {
            Ok(()) => self.next(),
            Err(_) => None,
        }
    }
}

impl<const N: usize, H> ExactSizeIterator for ProofsIterator<'_, N, H>
where
    H: MerkleHasher,
{
    #[inline(always)]
    fn len(&self) -> usize {
        self.remaining()
    }
}

// SAFETY: size_hint is always exact
unsafe impl<const N: usize, H> TrustedLen for ProofsIterator<'_, N, H> where H: MerkleHasher {}
