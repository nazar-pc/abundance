# 0.3.0

Breaking changes:

* All trees are generic over the hasher, aliases of trees with the default BLAKE3 hasher are in the new `blake3` module
* BLAKE3 is behind the `blake3` feature, which is not enabled by default
* `UnbalancedMerkleTree` methods that take an iterator of leaves require it to be `TrustedLen`, `*_dynamic_len*()`
  variants accept any iterator
* `MmrPeaks`, `MerkleMountainRangeBytes`, `sparse::Leaf` and `balanced::ProofsIterator` are generic over the hasher too
* `INNER_NODE_DOMAIN_SEPARATOR`, `hash_pair()` and `hash_pair_block()` were removed, the default key is available as
  `blake3::Blake3MerkleHasher::KEY`

New features:

* Customizable hasher with the new `MerkleHasher` trait, which defines the size of nodes (`OUT_LEN`) and hashes one or
  many pairs of nodes at once, such that hash functions with other output sizes and SIMD-accelerated hashing of many
  pairs can be used
* Hashers that don't depend on the tree size are created with `Default`, hashers that depend on it implement the new
  `const` trait `NewMerkleHasherForTreeSize`, which is implemented for all hashers that implement `Default`;
  `MerkleMountainRange`, `SparseMerkleTree` and `UnbalancedMerkleTree::*_dynamic_len*()` only support the former,
  `BalancedMerkleTree` and `UnbalancedMerkleTree::compute_root_only_array()` require a `const` implementation
* `blake3::Blake3MerkleHasher`, which produces the same roots and proofs as 0.2.0, other BLAKE3 keys require a custom
  `MerkleHasher`
* `MerkleHasher::hash_single_leaf()` customizes the root of an `UnbalancedMerkleTree` or `MerkleMountainRange` with a
  single leaf, which is the leaf itself by default, so hashers that depend on the tree size can commit to it in that
  case too
* `UnbalancedMerkleTree::compute_root_only_sparse()` computes the root of a tree whose leaves are mostly zeroes, it
  hashes runs of zero leaves as whole subtrees, so the cost depends on the number of non-zero leaves and the height of
  the tree rather than on the number of leaves

Improvements:

* BLAKE3 is optional, so users of other hash functions don't need to compile it

Fixes:

* `BalancedMerkleTree::compute_root_only()` and `BalancedMerkleTree::verify()` no longer compile with `N` that is not a
  power of two or is smaller than 2, previously they did in builds without debug info and ignored some of the leaves or
  panicked at runtime
* `MerkleMountainRange::from_peaks()` now returns `None` for more than `MAX_N` leaves, previously it accepted them as
  long as stack and peaks had space for them
* `SparseMerkleTree::compute_root_only()` and `SparseMerkleTree::verify()` with unsupported `BITS` and
  `UnbalancedMerkleTree::compute_root_only_array()` with an empty array no longer compile when called from generic code
  in crates with `generic_const_args`, previously they returned wrong results or panicked at runtime

# 0.2.0

Breaking changes:

* Migrate from `generic_const_exprs` to `generic_const_args` family of nightly features

Improvements:

* Faster balanced Merkle Tree proof generation
* Extended MMR/SMT benchmarks/tests

Fixes:

* Fix `MERKLE_MOUNTAIN_RANGE_BYTES_SIZE` not computed correctly for non-power-of-to sizes
* Reject `MAX_N == 1` in MMR to avoid out-of-bounds reads (it is quite useless to justify proper support for this
  special case)

# 0.1.0

Initial release
