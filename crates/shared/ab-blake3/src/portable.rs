// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
use crate::platform::{le_bytes_from_words_32, words_from_le_bytes_64};
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
use crate::{BLOCK_LEN, BlockBytes, CVBytes, OUT_LEN};
use crate::{BlockWords, CVWords, IV, MSG_SCHEDULE};
// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
use blake3::IncrementCounter;

#[inline(always)]
const fn g(state: &mut BlockWords, a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(x);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(y);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

#[inline(always)]
const fn round(state: &mut BlockWords, msg: &BlockWords, round: usize) {
    // Select the message schedule based on the round.
    let schedule = MSG_SCHEDULE[round];

    // Mix the columns.
    g(state, 0, 4, 8, 12, msg[schedule[0]], msg[schedule[1]]);
    g(state, 1, 5, 9, 13, msg[schedule[2]], msg[schedule[3]]);
    g(state, 2, 6, 10, 14, msg[schedule[4]], msg[schedule[5]]);
    g(state, 3, 7, 11, 15, msg[schedule[6]], msg[schedule[7]]);

    // Mix the diagonals.
    g(state, 0, 5, 10, 15, msg[schedule[8]], msg[schedule[9]]);
    g(state, 1, 6, 11, 12, msg[schedule[10]], msg[schedule[11]]);
    g(state, 2, 7, 8, 13, msg[schedule[12]], msg[schedule[13]]);
    g(state, 3, 4, 9, 14, msg[schedule[14]], msg[schedule[15]]);
}

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline]
const fn counter_low(counter: u64) -> u32 {
    counter as u32
}

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline]
const fn counter_high(counter: u64) -> u32 {
    (counter >> 32) as u32
}

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[inline(always)]
const fn compress_pre(
    cv: &CVWords,
    block_words: &BlockWords,
    block_len: u32,
    counter: u64,
    flags: u32,
) -> BlockWords {
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter_low(counter),
        counter_high(counter),
        block_len,
        flags,
    ];

    round(&mut state, block_words, 0);
    round(&mut state, block_words, 1);
    round(&mut state, block_words, 2);
    round(&mut state, block_words, 3);
    round(&mut state, block_words, 4);
    round(&mut state, block_words, 5);
    round(&mut state, block_words, 6);

    state
}

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
pub(crate) const fn compress_in_place(
    cv: &mut CVWords,
    block_words: &BlockWords,
    block_len: u32,
    counter: u64,
    flags: u32,
) {
    let state = compress_pre(cv, block_words, block_len, counter, flags);

    cv[0] = state[0] ^ state[8];
    cv[1] = state[1] ^ state[9];
    cv[2] = state[2] ^ state[10];
    cv[3] = state[3] ^ state[11];
    cv[4] = state[4] ^ state[12];
    cv[5] = state[5] ^ state[13];
    cv[6] = state[6] ^ state[14];
    cv[7] = state[7] ^ state[15];
}

///  Like [`compress_pre()`], but `counter` is limited to `u32` for small inputs
#[inline(always)]
const fn compress_pre_u32(
    cv: &CVWords,
    block_words: &BlockWords,
    block_len: u32,
    counter: u32,
    flags: u32,
) -> BlockWords {
    #[rustfmt::skip]
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        // Counter low
        counter,
        // Counter high
        0,
        block_len,
        flags,
    ];

    round(&mut state, block_words, 0);
    round(&mut state, block_words, 1);
    round(&mut state, block_words, 2);
    round(&mut state, block_words, 3);
    round(&mut state, block_words, 4);
    round(&mut state, block_words, 5);
    round(&mut state, block_words, 6);

    state
}

///  Like [`compress_in_place()`], but `counter` is limited to `u32` for small inputs
pub(crate) const fn compress_in_place_u32(
    cv: &mut CVWords,
    block_words: &BlockWords,
    block_len: u32,
    counter: u32,
    flags: u32,
) {
    let state = compress_pre_u32(cv, block_words, block_len, counter, flags);

    cv[0] = state[0] ^ state[8];
    cv[1] = state[1] ^ state[9];
    cv[2] = state[2] ^ state[10];
    cv[3] = state[3] ^ state[11];
    cv[4] = state[4] ^ state[12];
    cv[5] = state[5] ^ state[13];
    cv[6] = state[6] ^ state[14];
    cv[7] = state[7] ^ state[15];
}

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
const fn hash1<const N: usize>(
    input: &[u8; N],
    key: &CVWords,
    counter: u64,
    flags: u8,
    flags_start: u8,
    flags_end: u8,
    out: &mut CVBytes,
) {
    debug_assert!(N.is_multiple_of(BLOCK_LEN), "uneven blocks");
    let mut cv = *key;
    let mut block_flags = flags | flags_start;
    let mut slice = input.as_slice();
    while slice.len() >= BLOCK_LEN {
        let block;
        (block, slice) = slice.split_at(BLOCK_LEN);
        if slice.is_empty() {
            block_flags |= flags_end;
        }
        let block = {
            let ptr = block.as_ptr().cast::<BlockBytes>();
            // SAFETY: Sliced off correct length above
            unsafe { &*ptr }
        };
        let block_words = words_from_le_bytes_64(block);

        compress_in_place(
            &mut cv,
            &block_words,
            BLOCK_LEN as u32,
            counter,
            block_flags as u32,
        );
        block_flags = flags;
    }
    *out = *le_bytes_from_words_32(&cv);
}

// TODO: Workaround for https://github.com/Rust-GPU/rust-gpu/issues/312
#[cfg(not(target_arch = "spirv"))]
#[expect(clippy::too_many_arguments, reason = "Internal")]
pub(crate) const fn hash_many<const N: usize>(
    mut inputs: &[&[u8; N]],
    key: &CVWords,
    mut counter: u64,
    increment_counter: IncrementCounter,
    flags: u8,
    flags_start: u8,
    flags_end: u8,
    mut out: &mut [u8],
) {
    debug_assert!(out.len() >= inputs.len() * OUT_LEN, "out too short");
    while !inputs.is_empty() {
        let input;
        (input, inputs) = inputs.split_first().expect("Not empty; qed");
        let o;
        (o, out) = out.split_at_mut(OUT_LEN);
        let o = {
            let ptr = o.as_mut_ptr().cast::<[u8; OUT_LEN]>();
            // SAFETY: Sliced off correct length above
            unsafe { &mut *ptr }
        };

        hash1(input, key, counter, flags, flags_start, flags_end, o);
        if matches!(increment_counter, IncrementCounter::Yes) {
            counter += 1;
        }
    }
}
