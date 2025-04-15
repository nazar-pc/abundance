#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use blake3::OUT_LEN;
use core::iter::TrustedLen;
use core::mem::MaybeUninit;

/// Number of hashes, including root and excluding already provided leaf hashes
#[inline(always)]
pub const fn num_hashes(num_leaves_log_2: u32) -> usize {
    2_usize.pow(num_leaves_log_2) - 1
}

/// Number of leaves in a tree
#[inline(always)]
pub const fn num_leaves(num_leaves_log_2: u32) -> usize {
    2_usize.pow(num_leaves_log_2)
}

/// Merkle Tree variant that has hash-sized leaves and is fully balanced according to configured
/// generic parameter.
///
/// This Merkle Tree implementation is best suited for use cases when proofs for all (or most) of
/// the elements need to be generated and the whole tree easily fits into memory. It can also be
/// constructed and proofs can be generated efficiently without heap allocations.
///
/// With all parameters of the tree known statically, it results in the most efficient version of
/// the code being generated for a given set of parameters.
///
/// `NUM_LEAVES_LOG_2` is base-2 logarithm of the number of leaves in a tree.
#[derive(Debug)]
pub struct BalancedHashedMerkleTree<'a, const NUM_LEAVES_LOG_2: u32>
where
    [(); num_hashes(NUM_LEAVES_LOG_2)]:,
{
    leaf_hashes: &'a [[u8; OUT_LEN]],
    // This tree doesn't include leaves because we know the size
    tree: [[u8; OUT_LEN]; num_hashes(NUM_LEAVES_LOG_2)],
}

// TODO: Replace hashing individual records with blake3 and building tree manually with building the
//  tree using blake3 itself, such that the root is the same as hashing data with blake3, see
//  https://github.com/BLAKE3-team/BLAKE3/issues/470 for details. Two options are:
//  expand values to 1024 bytes or modify blake3 to use 32-byte chunk size (at which point it'll
//  unfortunately stop being blake3)
impl<'a, const NUM_LEAVES_LOG_2: u32> BalancedHashedMerkleTree<'a, NUM_LEAVES_LOG_2>
where
    [(); num_hashes(NUM_LEAVES_LOG_2)]:,
{
    /// Create a new tree from a fixed set of elements.
    ///
    /// The data structure is statically allocated and might be too large to fit on the stack!
    /// If that is the case, use `new_boxed()` method.
    pub fn new(leaf_hashes: &'a [[u8; OUT_LEN]; num_leaves(NUM_LEAVES_LOG_2)]) -> Self {
        let mut tree = [MaybeUninit::<[u8; OUT_LEN]>::uninit(); num_hashes(NUM_LEAVES_LOG_2)];

        Self::init_internal(leaf_hashes, &mut tree);

        Self {
            leaf_hashes,
            // SAFETY: Statically guaranteed for all elements to be initialized
            tree: unsafe { tree.transpose().assume_init() },
        }
    }

    /// Like [`Self::new()`], but used pre-allocated memory for instantiation
    pub fn new_in<'b>(
        instance: &'b mut MaybeUninit<Self>,
        leaf_hashes: &'a [[u8; OUT_LEN]; num_leaves(NUM_LEAVES_LOG_2)],
    ) -> &'b mut Self {
        let instance_ptr = instance.as_mut_ptr();
        // SAFETY: Valid and correctly aligned non-null pointer
        unsafe {
            (&raw mut (*instance_ptr).leaf_hashes).write(leaf_hashes);
        }
        let tree = {
            // SAFETY: Valid and correctly aligned non-null pointer
            let tree_ptr = unsafe { &raw mut (*instance_ptr).tree };
            // SAFETY: Allocated and correctly aligned uninitialized data
            unsafe {
                tree_ptr
                    .cast::<[MaybeUninit<[u8; OUT_LEN]>; num_hashes(NUM_LEAVES_LOG_2)]>()
                    .as_mut_unchecked()
            }
        };

        Self::init_internal(leaf_hashes, tree);

        // SAFETY: Initialized field by field above
        unsafe { instance.assume_init_mut() }
    }

    /// Like [`Self::new()`], but creates heap-allocated instance, avoiding excessive stack usage
    /// for large trees
    #[cfg(feature = "alloc")]
    pub fn new_boxed(leaf_hashes: &'a [[u8; OUT_LEN]; num_leaves(NUM_LEAVES_LOG_2)]) -> Box<Self> {
        let mut instance = Box::<Self>::new_uninit();

        Self::new_in(&mut instance, leaf_hashes);

        // SAFETY: Initialized by constructor above
        unsafe { instance.assume_init() }
    }

    fn init_internal(
        leaf_hashes: &[[u8; OUT_LEN]; num_leaves(NUM_LEAVES_LOG_2)],
        tree: &mut [MaybeUninit<[u8; OUT_LEN]>; num_hashes(NUM_LEAVES_LOG_2)],
    ) {
        let mut tree_hashes = tree.as_mut_slice();
        let mut level_hashes = leaf_hashes.as_slice();

        let mut pair = [0u8; OUT_LEN * 2];
        while level_hashes.len() > 1 {
            let num_pairs = level_hashes.len() / 2;
            let parent_hashes;
            // SAFETY: The size of the tree is statically known to match the number of leaves and
            // levels of hashes
            (parent_hashes, tree_hashes) = unsafe { tree_hashes.split_at_mut_unchecked(num_pairs) };

            for pair_index in 0..num_pairs {
                // SAFETY: Entry is statically known to be present
                let left_hash = unsafe { level_hashes.get_unchecked(pair_index * 2) };
                // SAFETY: Entry is statically known to be present
                let right_hash = unsafe { level_hashes.get_unchecked(pair_index * 2 + 1) };
                // SAFETY: Entry is statically known to be present
                let parent_hash = unsafe { parent_hashes.get_unchecked_mut(pair_index) };

                pair[..OUT_LEN].copy_from_slice(left_hash);
                pair[OUT_LEN..].copy_from_slice(right_hash);

                parent_hash.write(*blake3::hash(&pair).as_bytes());
            }

            // SAFETY: Just initialized
            level_hashes = unsafe { parent_hashes.assume_init_ref() };
        }
    }

    /// Get the root of Merkle Tree.
    ///
    /// In case a tree contains a single leaf hash, that leaf hash is returned.
    pub fn root(&self) -> [u8; OUT_LEN] {
        *self
            .tree
            .last()
            .or(self.leaf_hashes.last())
            .expect("There is always at least one leaf hash; qed")
    }

    /// Iterator over proofs in the same order as provided leaf hashes
    pub fn all_proofs(
        &self,
    ) -> impl ExactSizeIterator<Item = [[u8; OUT_LEN]; NUM_LEAVES_LOG_2 as usize]> + TrustedLen
    where
        [(); NUM_LEAVES_LOG_2 as usize]:,
    {
        let iter = self.leaf_hashes.array_chunks().enumerate().flat_map(
            |(pair_index, &[left_hash, right_hash])| {
                let mut left_proof =
                    [MaybeUninit::<[u8; OUT_LEN]>::uninit(); NUM_LEAVES_LOG_2 as usize];
                left_proof[0].write(right_hash);

                let left_proof = {
                    let (_, shared_proof) = left_proof.split_at_mut(1);

                    let mut tree_hashes = self.tree.as_slice();
                    let mut parent_position = pair_index;
                    let mut parent_level_size = num_leaves(NUM_LEAVES_LOG_2) / 2;

                    for hash in shared_proof {
                        let parent_other_position = if parent_position % 2 == 0 {
                            parent_position + 1
                        } else {
                            parent_position - 1
                        };
                        // SAFETY: Statically guaranteed to be present by constructor
                        let other_hash =
                            unsafe { tree_hashes.get_unchecked(parent_other_position) };
                        hash.write(*other_hash);
                        (_, tree_hashes) = tree_hashes.split_at(parent_level_size);

                        parent_position /= 2;
                        parent_level_size /= 2;
                    }

                    // SAFETY: Just initialized
                    unsafe { left_proof.transpose().assume_init() }
                };

                let mut right_proof = left_proof;
                right_proof[0] = left_hash;

                [left_proof, right_proof]
            },
        );

        ProofsIterator {
            iter,
            len: num_leaves(NUM_LEAVES_LOG_2),
        }
    }

    /// Verify previously generated proof
    pub fn verify(
        root: &[u8; OUT_LEN],
        proof: &[[u8; OUT_LEN]; NUM_LEAVES_LOG_2 as usize],
        leaf_index: usize,
        leaf_hash: [u8; OUT_LEN],
    ) -> bool
    where
        [(); NUM_LEAVES_LOG_2 as usize]:,
    {
        if leaf_index >= num_leaves(NUM_LEAVES_LOG_2) {
            return false;
        }

        let mut computed_root = leaf_hash;

        let mut position = leaf_index;
        let mut pair = [0u8; OUT_LEN * 2];
        for hash in proof {
            if position % 2 == 0 {
                pair[..OUT_LEN].copy_from_slice(&computed_root);
                pair[OUT_LEN..].copy_from_slice(hash);
            } else {
                pair[..OUT_LEN].copy_from_slice(hash);
                pair[OUT_LEN..].copy_from_slice(&computed_root);
            }

            position /= 2;
            computed_root = *blake3::hash(&pair).as_bytes();
        }

        root == &computed_root
    }
}

struct ProofsIterator<Iter> {
    iter: Iter,
    len: usize,
}

impl<Iter> Iterator for ProofsIterator<Iter>
where
    Iter: Iterator,
{
    type Item = Iter::Item;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }

    #[inline(always)]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.len
    }
}

impl<Iter> ExactSizeIterator for ProofsIterator<Iter>
where
    Iter: Iterator,
{
    #[inline(always)]
    fn len(&self) -> usize {
        self.len
    }
}

unsafe impl<Iter> TrustedLen for ProofsIterator<Iter> where Iter: Iterator {}
