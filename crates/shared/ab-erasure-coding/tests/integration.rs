use ab_erasure_coding::{ErasureCoding, ErasureCodingError, ShardsBitmap, ShardsPresent};
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{Rng, SeedableRng};
use reed_solomon_simd::Error;
use std::ops::Range;
use std::{array, assert_matches};

// Miri is very slow, use less data for it
const NUM_SHARDS: usize = if cfg!(miri) {
    2usize.pow(3)
} else {
    2usize.pow(8)
};
const NUM_SOURCE: usize = NUM_SHARDS / 2;
const SHARD_BYTES: usize = 32;

/// Corrupts shards in `range` and unsets them in `present`
fn corrupt_shards(
    shards: &mut [[u8; SHARD_BYTES]; NUM_SOURCE],
    present: &mut ShardsBitmap<NUM_SOURCE>,
    range: Range<usize>,
) {
    for index in range {
        shards[index] = [index as u8; SHARD_BYTES];
        present.unset(index);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn basic_data() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let ec = ErasureCoding::new();

    let mut source_shards = [[0u8; SHARD_BYTES]; NUM_SOURCE];
    for shard in &mut source_shards {
        rng.fill_bytes(shard);
    }
    let mut parity_shards = [[0u8; SHARD_BYTES]; NUM_SOURCE];

    ec.extend(&source_shards, &mut parity_shards).unwrap();

    assert_ne!(source_shards, parity_shards);

    // Both source and parity shards are recovered
    {
        let mut recovered_source_shards = source_shards;
        let mut recovered_parity_shards = parity_shards;
        let mut present = ShardsPresent::<NUM_SOURCE>::all();
        corrupt_shards(
            &mut recovered_source_shards,
            &mut present.source,
            0..NUM_SHARDS / 4,
        );
        corrupt_shards(
            &mut recovered_parity_shards,
            &mut present.parity,
            NUM_SHARDS / 4..NUM_SHARDS * 2 / 4,
        );

        ec.recover_all(
            &mut recovered_source_shards,
            &mut recovered_parity_shards,
            &present,
        )
        .unwrap();

        assert_eq!(recovered_source_shards, source_shards);
        assert_eq!(recovered_parity_shards, parity_shards);
    }

    // Only source shards are recovered, parity shards are inputs only
    {
        let mut recovered_source_shards = source_shards;
        let mut present = ShardsPresent::<NUM_SOURCE>::all();
        corrupt_shards(
            &mut recovered_source_shards,
            &mut present.source,
            0..NUM_SHARDS / 4,
        );

        ec.recover_source(&mut recovered_source_shards, &parity_shards, &present)
            .unwrap();

        assert_eq!(recovered_source_shards, source_shards);
    }

    // Shards must have even length
    assert_matches!(
        ec.extend(&[[0u8; 31]; NUM_SOURCE], &mut [[0u8; 31]; NUM_SOURCE]),
        Err(ErasureCodingError::DecoderError(Error::InvalidShardSize {
            shard_bytes: 31
        }))
    );

    // Too many corrupted shards
    {
        let mut recovered_source_shards = source_shards;
        let mut recovered_parity_shards = parity_shards;
        let mut present = ShardsPresent::<NUM_SOURCE>::all();
        corrupt_shards(
            &mut recovered_source_shards,
            &mut present.source,
            0..NUM_SHARDS / 4 + 1,
        );
        corrupt_shards(
            &mut recovered_parity_shards,
            &mut present.parity,
            NUM_SHARDS / 4..NUM_SHARDS * 2 / 4,
        );

        assert_matches!(
            ec.recover_all(
                &mut recovered_source_shards,
                &mut recovered_parity_shards,
                &present,
            ),
            Err(ErasureCodingError::DecoderError(Error::NotEnoughShards {
                original_count: _,
                original_received_count: _,
                recovery_received_count: _,
            }))
        );
    }
}

/// A shard stored inside a larger data structure, the way records live inside pieces
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
struct Shard {
    bytes: [u8; SHARD_BYTES],
    _unrelated_metadata: u32,
}

impl AsRef<[u8; SHARD_BYTES]> for Shard {
    fn as_ref(&self) -> &[u8; SHARD_BYTES] {
        &self.bytes
    }
}

impl AsMut<[u8; SHARD_BYTES]> for Shard {
    fn as_mut(&mut self) -> &mut [u8; SHARD_BYTES] {
        &mut self.bytes
    }
}

/// Shards do not have to live in one contiguous allocation
#[test]
#[cfg_attr(miri, ignore)]
fn scattered_shards() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let ec = ErasureCoding::new();

    let mut source_shards = [[0u8; SHARD_BYTES]; NUM_SOURCE];
    for shard in &mut source_shards {
        rng.fill_bytes(shard);
    }
    let mut parity_shards = [[0u8; SHARD_BYTES]; NUM_SOURCE];
    ec.extend(&source_shards, &mut parity_shards).unwrap();

    // Same thing, but through shards that live inside a larger data structure
    let mut scattered_source: [Shard; NUM_SOURCE] = array::from_fn(|index| Shard {
        bytes: source_shards[index],
        _unrelated_metadata: index as u32,
    });
    let mut scattered_parity = [Shard::default(); NUM_SOURCE];
    {
        let mut source_iter = scattered_source.iter();
        let source_refs: [&Shard; NUM_SOURCE] = array::from_fn(|_| source_iter.next().unwrap());
        let mut parity_iter = scattered_parity.iter_mut();
        let parity_refs: [&mut Shard; NUM_SOURCE] = array::from_fn(|_| parity_iter.next().unwrap());

        ec.extend_scattered(source_refs, parity_refs).unwrap();
    }
    for (scattered, expected) in scattered_parity.iter().zip(&parity_shards) {
        assert_eq!(&scattered.bytes, expected);
    }

    // Recovery through optional references, where missing parity shards have no memory at all
    let mut source_present = ShardsBitmap::<NUM_SOURCE>::all();
    for (index, shard) in scattered_source.iter_mut().enumerate().take(NUM_SHARDS / 4) {
        shard.bytes = [index as u8; SHARD_BYTES];
        source_present.unset(index);
    }

    let parity_refs: [Option<&Shard>; NUM_SOURCE] =
        array::from_fn(|index| (index < NUM_SHARDS / 4).then_some(&scattered_parity[index]));
    let mut source_iter = scattered_source.iter_mut();
    let source_refs: [&mut Shard; NUM_SOURCE] = array::from_fn(|_| source_iter.next().unwrap());

    ec.recover_source_scattered(source_refs, &source_present, parity_refs)
        .unwrap();

    for (scattered, expected) in scattered_source.iter().zip(&source_shards) {
        assert_eq!(&scattered.bytes, expected);
    }
}
