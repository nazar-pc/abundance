use ab_merkle_tree::unbalanced::UnbalancedMerkleTree;

// Unbalanced trees are aliased through a projection, because rustdoc crashes on plain aliases of
// `UnbalancedMerkleTree` from another crate: it copies methods of the aliased type into the alias
// docs and fails to evaluate proof lengths that depend on generic parameters of those methods
// TODO: Use plain aliases once https://github.com/rust-lang/rust/issues/165186 is resolved
/// Helper for aliases of [`UnbalancedMerkleTree`]
pub trait UnbalancedMerkleTreeAlias {
    /// Aliased tree
    type Tree;
}

impl<H> UnbalancedMerkleTreeAlias for H {
    type Tree = UnbalancedMerkleTree<H>;
}
