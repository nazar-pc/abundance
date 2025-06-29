use crate::const_fn::{CVBytes, BLOCK_LEN, CHUNK_LEN};
use crate::{const_derive_key, const_hash, const_keyed_hash};
use blake3::{derive_key, hash, keyed_hash};

// Interesting input lengths to run tests on.
const TEST_CASES: &[usize] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    BLOCK_LEN - 1,
    BLOCK_LEN,
    BLOCK_LEN + 1,
    2 * BLOCK_LEN - 1,
    2 * BLOCK_LEN,
    2 * BLOCK_LEN + 1,
    CHUNK_LEN - 1,
    CHUNK_LEN,
    CHUNK_LEN + 1,
    2 * CHUNK_LEN,
    2 * CHUNK_LEN + 1,
    3 * CHUNK_LEN,
    3 * CHUNK_LEN + 1,
    4 * CHUNK_LEN,
    4 * CHUNK_LEN + 1,
    5 * CHUNK_LEN,
    5 * CHUNK_LEN + 1,
    6 * CHUNK_LEN,
    6 * CHUNK_LEN + 1,
    7 * CHUNK_LEN,
    7 * CHUNK_LEN + 1,
    8 * CHUNK_LEN,
    8 * CHUNK_LEN + 1,
    16 * CHUNK_LEN - 1,
    16 * CHUNK_LEN, // AVX512's bandwidth
    16 * CHUNK_LEN + 1,
    31 * CHUNK_LEN - 1,
    31 * CHUNK_LEN, // 16 + 8 + 4 + 2 + 1
    31 * CHUNK_LEN + 1,
    100 * CHUNK_LEN, // subtrees larger than MAX_SIMD_DEGREE chunks
];

const TEST_CASES_MAX: usize = 100 * CHUNK_LEN;

// There's a test to make sure these two are equal below.
const TEST_KEY: CVBytes = *b"whats the Elvish word for friend";

#[test]
fn test_compare_with_upstream() {
    let mut input_buf = [0; TEST_CASES_MAX];

    // Paint the input with a repeating byte pattern. We use a cycle length of 251,
    // because that's the largest prime number less than 256. This makes it
    // unlikely to swapping any two adjacent input blocks or chunks will give the
    // same answer.
    for (i, b) in input_buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }

    for &case in TEST_CASES {
        let input = &input_buf[..case];

        // regular
        assert_eq!(hash(input), const_hash(input));

        // keyed
        assert_eq!(
            keyed_hash(&TEST_KEY, input),
            const_keyed_hash(&TEST_KEY, input)
        );

        // derive_key
        let context = "BLAKE3 2019-12-27 16:13:59 example context (not the test vector one)";
        assert_eq!(derive_key(context, input), const_derive_key(context, input));
    }
}
