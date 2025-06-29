#[cfg(test)]
mod tests;

use crate::const_fn::{CHUNK_LEN, IV, KEY_LEN};

/// An alias to distinguish [`const_hash_derive_key_context`] outputs from other keys.
pub(super) type ContextKey = [u8; KEY_LEN];

/// Given the length in bytes of either a complete input or a subtree input, return the number of
/// bytes that belong to its left child subtree. The rest belong to its right child subtree.
///
/// Concretely, this function returns the largest power-of-two number of bytes that's strictly less
/// than `input_len`. This leads to a tree where all left subtrees are "complete" and at least as
/// large as their sibling right subtrees, as specified in section 2.1 of [the BLAKE3
/// paper](https://github.com/BLAKE3-team/BLAKE3-specs/blob/master/blake3.pdf). For example, if an
/// input is exactly two chunks, its left and right subtrees both get one chunk. But if an input is
/// two chunks plus one more byte, then its left subtree gets two chunks, and its right subtree
/// only gets one byte.
///
/// This function isn't meaningful for one chunk of input, because chunks don't have children. It
/// currently panics in debug mode if `input_len <= CHUNK_LEN`.
#[inline(always)]
pub(super) const fn left_subtree_len(input_len: u64) -> u64 {
    debug_assert!(input_len > CHUNK_LEN as u64);
    // Note that .next_power_of_two() is greater than *or equal*.
    input_len.div_ceil(2).next_power_of_two()
}

/// Hash a [`const_derive_key`](crate::const_fn::const_derive_key) context string and return a
/// [`ContextKey`]
pub(super) const fn const_hash_derive_key_context(context: &str) -> ContextKey {
    crate::const_fn::const_hash_all_at_once(
        context.as_bytes(),
        IV,
        crate::const_fn::DERIVE_KEY_CONTEXT,
    )
    .root_hash()
}
