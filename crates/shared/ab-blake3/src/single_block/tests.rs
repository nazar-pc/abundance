use crate::platform::{le_bytes_from_words_32, words_from_le_bytes_64};
use crate::single_block::single_block_hash_many_exact;
use crate::{
    BLOCK_LEN, CHUNK_LEN, CVBytes, OUT_LEN, single_block_derive_key, single_block_hash,
    single_block_hash_portable_words, single_block_keyed_hash, single_block_keyed_hash_many_exact,
};
use blake3::{derive_key, hash, keyed_hash};

// Interesting input lengths to run tests on.
const TEST_CASES: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, BLOCK_LEN - 1, BLOCK_LEN];

// There's a test to make sure these two are equal below.
const TEST_KEY: CVBytes = *b"whats the Elvish word for friend";

#[test]
fn test_compare_with_upstream() {
    let mut input_buf = [0; CHUNK_LEN + BLOCK_LEN];

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

    test_compare_with_upstream_exact::<17>(&input_buf);
    test_compare_with_upstream_exact::<16>(&input_buf);
    test_compare_with_upstream_exact::<15>(&input_buf);
    test_compare_with_upstream_exact::<9>(&input_buf);
    test_compare_with_upstream_exact::<8>(&input_buf);
    test_compare_with_upstream_exact::<7>(&input_buf);
    test_compare_with_upstream_exact::<5>(&input_buf);
    test_compare_with_upstream_exact::<4>(&input_buf);
    test_compare_with_upstream_exact::<3>(&input_buf);
    test_compare_with_upstream_exact::<2>(&input_buf);
    test_compare_with_upstream_exact::<1>(&input_buf);
}

fn test_compare_with_upstream_exact<const NUM_BLOCKS: usize>(input_buf: &[u8]) {
    assert!(input_buf.len() >= BLOCK_LEN * NUM_BLOCKS);
    let inputs = unsafe { &*input_buf.as_ptr().cast::<[[u8; BLOCK_LEN]; NUM_BLOCKS]>() };

    // Regular hash
    {
        let mut outputs = [[0u8; OUT_LEN]; NUM_BLOCKS];

        single_block_hash_many_exact(inputs, &mut outputs);

        for (index, (input, output)) in inputs.iter().zip(&outputs).enumerate() {
            assert_eq!(
                hash(input).as_bytes(),
                output,
                "NUM_BLOCKS={NUM_BLOCKS} index={index}"
            );
        }
    }

    // Keyed hash
    {
        let mut outputs = [[0u8; OUT_LEN]; NUM_BLOCKS];
        single_block_keyed_hash_many_exact(&TEST_KEY, inputs, &mut outputs);

        for (index, (input, output)) in inputs.iter().zip(&outputs).enumerate() {
            assert_eq!(
                keyed_hash(&TEST_KEY, input).as_bytes(),
                output,
                "NUM_BLOCKS={NUM_BLOCKS} index={index}"
            );
        }
    }
}
