#![feature(assert_matches, trusted_len)]

use ab_erasure_coding::{ErasureCoding, ErasureCodingError, RecoveryShardState};
use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use reed_solomon_simd::Error;
use std::assert_matches::assert_matches;
use std::iter::TrustedLen;
use std::ops::Range;

fn corrupt_shards<'a, Iter>(
    iter: Iter,
    range: Range<usize>,
) -> impl TrustedLen<Item = RecoveryShardState<&'a [u8], &'a mut [u8]>>
where
    Iter: TrustedLen<Item = &'a mut [u8; 32]>,
{
    iter.enumerate().map(move |(index, shard)| {
        if range.contains(&index) {
            // Corrupt the shard and mark as shard that needs to be recovered
            *shard = [index as u8; 32];
            RecoveryShardState::MissingRecover(shard.as_mut_slice())
        } else {
            RecoveryShardState::Present(shard.as_slice())
        }
    })
}

#[test]
#[cfg_attr(miri, ignore)]
fn basic_data() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let num_shards = 2usize.pow(if cfg!(miri) {
        // Miri is very slow, use less data for it
        3
    } else {
        8
    });
    let ec = ErasureCoding::new();

    let source_shards = (0..num_shards / 2)
        .map(|_| {
            let mut bytes = [0u8; 32];
            rng.fill_bytes(&mut bytes);
            bytes
        })
        .collect::<Vec<_>>();
    let mut parity_shards = vec![[0u8; 32]; source_shards.len()];

    ec.extend(source_shards.iter(), parity_shards.iter_mut())
        .unwrap();

    assert_ne!(source_shards, parity_shards);

    let mut recovered_source_shards = source_shards.clone();
    let mut recovered_parity_shards = parity_shards.clone();

    ec.recover(
        corrupt_shards(recovered_source_shards.iter_mut(), 0..num_shards / 4),
        corrupt_shards(
            recovered_parity_shards.iter_mut(),
            num_shards / 4..num_shards * 2 / 4,
        ),
    )
    .unwrap();

    assert_eq!(recovered_source_shards, source_shards);
    assert_eq!(recovered_parity_shards, parity_shards);

    // Source and parity shards have different size
    assert_matches!(
        ec.extend(
            source_shards.iter(),
            vec![[0u8; 34]; source_shards.len()].iter_mut(),
        ),
        Err(ErasureCodingError::WrongParityShardByteLength {
            expected: 32,
            actual: 34,
        })
    );

    // Shards must have even length
    assert_matches!(
        ec.extend(
            vec![[0u8; 31]; source_shards.len()].iter(),
            vec![[0u8; 31]; source_shards.len()].iter_mut(),
        ),
        Err(ErasureCodingError::DecoderError(Error::InvalidShardSize {
            shard_bytes: 31
        }))
    );

    // Too many corrupted shards
    assert_matches!(
        ec.recover(
            corrupt_shards(
                recovered_source_shards.clone().iter_mut(),
                0..num_shards / 4 + 1
            ),
            corrupt_shards(
                recovered_parity_shards.clone().iter_mut(),
                num_shards / 4..num_shards * 2 / 4,
            ),
        ),
        Err(ErasureCodingError::DecoderError(
            Error::NotEnoughShards { .. }
        ))
    );
}
