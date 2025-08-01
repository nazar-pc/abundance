//! `const fn` BLAKE3 functions.
//!
//! This module and submodules are copied with modifications from the official [`blake3`] crate and
//! are expected to be removed once <https://github.com/BLAKE3-team/BLAKE3/pull/439> or similar
//! lands upstream.

mod hazmat;
#[cfg(test)]
mod tests;

use crate::platform::{
    MAX_SIMD_DEGREE, MAX_SIMD_DEGREE_OR_2, le_bytes_from_words_32, words_from_le_bytes_32,
    words_from_le_bytes_64,
};
use crate::{
    BLOCK_LEN, BlockBytes, CHUNK_END, CHUNK_LEN, CHUNK_START, CVBytes, CVWords, DERIVE_KEY_CONTEXT,
    DERIVE_KEY_MATERIAL, IV, KEY_LEN, KEYED_HASH, OUT_LEN, PARENT, ROOT, portable,
};
use blake3::IncrementCounter;
use core::mem::MaybeUninit;
use core::slice;

/// `Output` with `const fn` methods
struct ConstOutput {
    input_chaining_value: CVWords,
    block: BlockBytes,
    block_len: u8,
    counter: u64,
    flags: u8,
}

impl ConstOutput {
    const fn chaining_value(&self) -> CVBytes {
        let mut cv = self.input_chaining_value;
        let block_words = words_from_le_bytes_64(&self.block);
        portable::compress_in_place(
            &mut cv,
            &block_words,
            self.block_len as u32,
            self.counter,
            self.flags as u32,
        );
        *le_bytes_from_words_32(&cv)
    }

    const fn root_hash(&self) -> [u8; OUT_LEN] {
        debug_assert!(self.counter == 0);
        let mut cv = self.input_chaining_value;
        let block_words = words_from_le_bytes_64(&self.block);
        portable::compress_in_place(
            &mut cv,
            &block_words,
            self.block_len as u32,
            0,
            (self.flags | ROOT) as u32,
        );
        *le_bytes_from_words_32(&cv)
    }
}

struct ConstChunkState {
    cv: CVWords,
    chunk_counter: u64,
    buf: BlockBytes,
    buf_len: u8,
    blocks_compressed: u8,
    flags: u8,
}

impl ConstChunkState {
    const fn new(key: &CVWords, chunk_counter: u64, flags: u8) -> Self {
        Self {
            cv: *key,
            chunk_counter,
            buf: [0; BLOCK_LEN],
            buf_len: 0,
            blocks_compressed: 0,
            flags,
        }
    }

    const fn count(&self) -> usize {
        BLOCK_LEN * self.blocks_compressed as usize + self.buf_len as usize
    }

    const fn fill_buf(&mut self, input: &mut &[u8]) {
        let want = BLOCK_LEN - self.buf_len as usize;
        let take = if want < input.len() {
            want
        } else {
            input.len()
        };
        let output = self
            .buf
            .split_at_mut(self.buf_len as usize)
            .1
            .split_at_mut(take)
            .0;
        output.copy_from_slice(input.split_at(take).0);
        self.buf_len += take as u8;
        *input = input.split_at(take).1;
    }

    const fn start_flag(&self) -> u8 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    // Try to avoid buffering as much as possible by compressing directly from
    // the input slice when full blocks are available.
    const fn update(&mut self, mut input: &[u8]) -> &mut Self {
        if self.buf_len > 0 {
            self.fill_buf(&mut input);
            if !input.is_empty() {
                debug_assert!(self.buf_len as usize == BLOCK_LEN);
                let block_flags = self.flags | self.start_flag(); // borrowck
                let block_words = words_from_le_bytes_64(&self.buf);
                portable::compress_in_place(
                    &mut self.cv,
                    &block_words,
                    BLOCK_LEN as u32,
                    self.chunk_counter,
                    block_flags as u32,
                );
                self.buf_len = 0;
                self.buf = [0; BLOCK_LEN];
                self.blocks_compressed += 1;
            }
        }

        while input.len() > BLOCK_LEN {
            debug_assert!(self.buf_len == 0);
            let block_flags = self.flags | self.start_flag(); // borrowck
            let block = input
                .first_chunk::<BLOCK_LEN>()
                .expect("Interation only starts when there is at least `BLOCK_LEN` bytes; qed");
            let block_words = words_from_le_bytes_64(block);
            portable::compress_in_place(
                &mut self.cv,
                &block_words,
                BLOCK_LEN as u32,
                self.chunk_counter,
                block_flags as u32,
            );
            self.blocks_compressed += 1;
            input = input.split_at(BLOCK_LEN).1;
        }

        self.fill_buf(&mut input);
        debug_assert!(input.is_empty());
        debug_assert!(self.count() <= CHUNK_LEN);
        self
    }

    const fn output(&self) -> ConstOutput {
        let block_flags = self.flags | self.start_flag() | CHUNK_END;
        ConstOutput {
            input_chaining_value: self.cv,
            block: self.buf,
            block_len: self.buf_len,
            counter: self.chunk_counter,
            flags: block_flags,
        }
    }
}

// IMPLEMENTATION NOTE
// ===================
// The recursive function compress_subtree_wide(), implemented below, is the
// basis of high-performance BLAKE3. We use it both for all-at-once hashing,
// and for the incremental input with Hasher (though we have to be careful with
// subtree boundaries in the incremental case). compress_subtree_wide() applies
// several optimizations at the same time:
// - Multithreading with Rayon.
// - Parallel chunk hashing with SIMD.
// - Parallel parent hashing with SIMD. Note that while SIMD chunk hashing
//   maxes out at MAX_SIMD_DEGREE*CHUNK_LEN, parallel parent hashing continues
//   to benefit from larger inputs, because more levels of the tree benefit can
//   use full-width SIMD vectors for parent hashing. Without parallel parent
//   hashing, we lose about 10% of overall throughput on AVX2 and AVX-512.

// Use SIMD parallelism to hash up to MAX_SIMD_DEGREE chunks at the same time
// on a single thread. Write out the chunk chaining values and return the
// number of chunks hashed. These chunks are never the root and never empty;
// those cases use a different codepath.
const fn const_compress_chunks_parallel(
    input: &[u8],
    key: &CVWords,
    chunk_counter: u64,
    flags: u8,
    out: &mut [u8],
) -> usize {
    debug_assert!(!input.is_empty(), "empty chunks below the root");
    debug_assert!(input.len() <= MAX_SIMD_DEGREE * CHUNK_LEN);

    let mut chunks = input;
    let mut chunks_so_far = 0;
    let mut chunks_array = [MaybeUninit::<&[u8; CHUNK_LEN]>::uninit(); MAX_SIMD_DEGREE];
    while let Some(chunk) = chunks.first_chunk::<CHUNK_LEN>() {
        chunks = chunks.split_at(CHUNK_LEN).1;
        chunks_array[chunks_so_far].write(chunk);
        chunks_so_far += 1;
    }
    portable::hash_many(
        // SAFETY: Exactly `chunks_so_far` elements of `chunks_array` were initialized above
        unsafe {
            slice::from_raw_parts(
                chunks_array.as_ptr().cast::<&[u8; CHUNK_LEN]>(),
                chunks_so_far,
            )
        },
        key,
        chunk_counter,
        IncrementCounter::Yes,
        flags,
        CHUNK_START,
        CHUNK_END,
        out,
    );

    // Hash the remaining partial chunk, if there is one. Note that the empty
    // chunk (meaning the empty message) is a different codepath.
    if !chunks.is_empty() {
        let counter = chunk_counter + chunks_so_far as u64;
        let mut chunk_state = ConstChunkState::new(key, counter, flags);
        chunk_state.update(chunks);
        let out = out
            .split_at_mut(chunks_so_far * OUT_LEN)
            .1
            .split_at_mut(OUT_LEN)
            .0;
        let chaining_value = chunk_state.output().chaining_value();
        out.copy_from_slice(&chaining_value);
        chunks_so_far + 1
    } else {
        chunks_so_far
    }
}

// Use SIMD parallelism to hash up to MAX_SIMD_DEGREE parents at the same time
// on a single thread. Write out the parent chaining values and return the
// number of parents hashed. (If there's an odd input chaining value left over,
// return it as an additional output.) These parents are never the root and
// never empty; those cases use a different codepath.
const fn const_compress_parents_parallel(
    child_chaining_values: &[u8],
    key: &CVWords,
    flags: u8,
    out: &mut [u8],
) -> usize {
    debug_assert!(
        child_chaining_values.len().is_multiple_of(OUT_LEN),
        "wacky hash bytes"
    );
    let num_children = child_chaining_values.len() / OUT_LEN;
    debug_assert!(num_children >= 2, "not enough children");
    debug_assert!(num_children <= 2 * MAX_SIMD_DEGREE_OR_2, "too many");

    let mut parents = child_chaining_values;
    // Use MAX_SIMD_DEGREE_OR_2 rather than MAX_SIMD_DEGREE here, because of
    // the requirements of compress_subtree_wide().
    let mut parents_so_far = 0;
    let mut parents_array = [MaybeUninit::<&BlockBytes>::uninit(); MAX_SIMD_DEGREE_OR_2];
    while let Some(parent) = parents.first_chunk::<BLOCK_LEN>() {
        parents = parents.split_at(BLOCK_LEN).1;
        parents_array[parents_so_far].write(parent);
        parents_so_far += 1;
    }
    portable::hash_many(
        // SAFETY: Exactly `parents_so_far` elements of `parents_array` were initialized above
        unsafe {
            slice::from_raw_parts(parents_array.as_ptr().cast::<&BlockBytes>(), parents_so_far)
        },
        key,
        0, // Parents always use counter 0.
        IncrementCounter::No,
        flags | PARENT,
        0, // Parents have no start flags.
        0, // Parents have no end flags.
        out,
    );

    // If there's an odd child left over, it becomes an output.
    if !parents.is_empty() {
        let out = out
            .split_at_mut(parents_so_far * OUT_LEN)
            .1
            .split_at_mut(OUT_LEN)
            .0;
        out.copy_from_slice(parents);
        parents_so_far + 1
    } else {
        parents_so_far
    }
}

// The wide helper function returns (writes out) an array of chaining values
// and returns the length of that array. The number of chaining values returned
// is the dynamically detected SIMD degree, at most MAX_SIMD_DEGREE. Or fewer,
// if the input is shorter than that many chunks. The reason for maintaining a
// wide array of chaining values going back up the tree, is to allow the
// implementation to hash as many parents in parallel as possible.
//
// As a special case when the SIMD degree is 1, this function will still return
// at least 2 outputs. This guarantees that this function doesn't perform the
// root compression. (If it did, it would use the wrong flags, and also we
// wouldn't be able to implement extendable output.) Note that this function is
// not used when the whole input is only 1 chunk long; that's a different
// codepath.
//
// Why not just have the caller split the input on the first update(), instead
// of implementing this special rule? Because we don't want to limit SIMD or
// multithreading parallelism for that update().
const fn const_compress_subtree_wide(
    input: &[u8],
    key: &CVWords,
    chunk_counter: u64,
    flags: u8,
    out: &mut [u8],
) -> usize {
    if input.len() <= CHUNK_LEN {
        return const_compress_chunks_parallel(input, key, chunk_counter, flags, out);
    }

    let (left, right) = input.split_at(hazmat::left_subtree_len(input.len() as u64) as usize);
    let right_chunk_counter = chunk_counter + (left.len() / CHUNK_LEN) as u64;

    // Make space for the child outputs. Here we use MAX_SIMD_DEGREE_OR_2 to
    // account for the special case of returning 2 outputs when the SIMD degree
    // is 1.
    let mut cv_array = [0; 2 * MAX_SIMD_DEGREE_OR_2 * OUT_LEN];
    let degree = if left.len() == CHUNK_LEN { 1 } else { 2 };
    let (left_out, right_out) = cv_array.split_at_mut(degree * OUT_LEN);

    // Recurse!
    let left_n = const_compress_subtree_wide(left, key, chunk_counter, flags, left_out);
    let right_n = const_compress_subtree_wide(right, key, right_chunk_counter, flags, right_out);

    // The special case again. If simd_degree=1, then we'll have left_n=1 and
    // right_n=1. Rather than compressing them into a single output, return
    // them directly, to make sure we always have at least two outputs.
    debug_assert!(left_n == degree);
    debug_assert!(right_n >= 1 && right_n <= left_n);
    if left_n == 1 {
        out.split_at_mut(2 * OUT_LEN)
            .0
            .copy_from_slice(cv_array.split_at(2 * OUT_LEN).0);
        return 2;
    }

    // Otherwise, do one layer of parent node compression.
    let num_children = left_n + right_n;
    const_compress_parents_parallel(cv_array.split_at(num_children * OUT_LEN).0, key, flags, out)
}

// Hash a subtree with compress_subtree_wide(), and then condense the resulting
// list of chaining values down to a single parent node. Don't compress that
// last parent node, however. Instead, return its message bytes (the
// concatenated chaining values of its children). This is necessary when the
// first call to update() supplies a complete subtree, because the topmost
// parent node of that subtree could end up being the root. It's also necessary
// for extended output in the general case.
//
// As with compress_subtree_wide(), this function is not used on inputs of 1
// chunk or less. That's a different codepath.
const fn const_compress_subtree_to_parent_node(
    input: &[u8],
    key: &CVWords,
    chunk_counter: u64,
    flags: u8,
) -> BlockBytes {
    debug_assert!(input.len() > CHUNK_LEN);
    let mut cv_array = [0; MAX_SIMD_DEGREE_OR_2 * OUT_LEN];
    let mut num_cvs = const_compress_subtree_wide(input, key, chunk_counter, flags, &mut cv_array);
    debug_assert!(num_cvs >= 2);

    // If MAX_SIMD_DEGREE is greater than 2 and there's enough input,
    // compress_subtree_wide() returns more than 2 chaining values. Condense
    // them into 2 by forming parent nodes repeatedly.
    let mut out_array = [0; MAX_SIMD_DEGREE_OR_2 * OUT_LEN / 2];
    while num_cvs > 2 {
        let cv_slice = cv_array.split_at(num_cvs * OUT_LEN).0;
        num_cvs = const_compress_parents_parallel(cv_slice, key, flags, &mut out_array);
        cv_array
            .split_at_mut(num_cvs * OUT_LEN)
            .0
            .copy_from_slice(out_array.split_at(num_cvs * OUT_LEN).0);
    }
    *cv_array
        .first_chunk::<BLOCK_LEN>()
        .expect("`cv_array` is larger than `BLOCK_LEN`; qed")
}

// Hash a complete input all at once. Unlike compress_subtree_wide() and
// compress_subtree_to_parent_node(), this function handles the 1 chunk case.
const fn const_hash_all_at_once(input: &[u8], key: &CVWords, flags: u8) -> ConstOutput {
    // If the whole subtree is one chunk, hash it directly with a ChunkState.
    if input.len() <= CHUNK_LEN {
        return ConstChunkState::new(key, 0, flags).update(input).output();
    }

    // Otherwise construct a `ConstOutput` object from the parent node returned by
    // compress_subtree_to_parent_node().
    ConstOutput {
        input_chaining_value: *key,
        block: const_compress_subtree_to_parent_node(input, key, 0, flags),
        block_len: BLOCK_LEN as u8,
        counter: 0,
        flags: flags | PARENT,
    }
}

/// Hashing function like [`blake3::hash()`], but `const fn`
pub const fn const_hash(input: &[u8]) -> [u8; OUT_LEN] {
    const_hash_all_at_once(input, IV, 0).root_hash()
}

/// The keyed hash function like [`blake3::keyed_hash()`], but `const fn`
pub const fn const_keyed_hash(key: &[u8; KEY_LEN], input: &[u8]) -> [u8; OUT_LEN] {
    let key_words = words_from_le_bytes_32(key);
    const_hash_all_at_once(input, &key_words, KEYED_HASH).root_hash()
}

// The key derivation function like [`blake3::derive_key()`], but `const fn`
pub const fn const_derive_key(context: &str, key_material: &[u8]) -> [u8; OUT_LEN] {
    let context_key =
        const_hash_all_at_once(context.as_bytes(), IV, DERIVE_KEY_CONTEXT).root_hash();
    let context_key_words = words_from_le_bytes_32(&context_key);
    const_hash_all_at_once(key_material, &context_key_words, DERIVE_KEY_MATERIAL).root_hash()
}
