use crate::platform::{le_bytes_from_words_32, words_from_le_bytes_64};
use crate::{
    single_block_derive_key, single_block_hash, single_block_hash_portable_words,
    single_block_keyed_hash, CVBytes, BLOCK_LEN, CHUNK_LEN,
};
use blake3::{derive_key, hash, keyed_hash};

// Interesting input lengths to run tests on.
const TEST_CASES: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, BLOCK_LEN - 1, BLOCK_LEN];

// There's a test to make sure these two are equal below.
const TEST_KEY: CVBytes = *b"whats the Elvish word for friend";

#[test]
fn test_compare_with_upstream() {
    let mut input_buf = [0; CHUNK_LEN];

    // Paint the input with a repeating byte pattern. We use a cycle length of 251,
    // because that's the largest prime number less than 256. This makes it
    // unlikely to swapping any two adjacent input blocks or blocks will give the
    // same answer.
    for (i, b) in input_buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }

    for &case in TEST_CASES {
        let input = &input_buf[..case];

        // regular
        assert_eq!(
            hash(input).as_bytes(),
            &single_block_hash(input).unwrap(),
            "{case}"
        );
        // regular words
        assert_eq!(
            hash(input).as_bytes(),
            le_bytes_from_words_32(&single_block_hash_portable_words(
                &{
                    let mut block = [0; BLOCK_LEN];
                    block[..input.len()].copy_from_slice(input);
                    words_from_le_bytes_64(&block)
                },
                input.len() as u32
            )),
            "{case}"
        );

        // keyed
        assert_eq!(
            keyed_hash(&TEST_KEY, input).as_bytes(),
            &single_block_keyed_hash(&TEST_KEY, input).unwrap(),
            "{case}"
        );

        // derive_key
        let context = "BLAKE3 2019-12-27 16:13:59 example context";
        assert_eq!(
            derive_key(context, input),
            single_block_derive_key(context, input).unwrap(),
            "{case}"
        );
    }
}
