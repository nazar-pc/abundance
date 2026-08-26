#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    generic_const_args,
    generic_const_items,
    macroless_generic_const_args,
    min_generic_const_args
)]
#![no_std]

use core::fmt;
use reed_solomon_simd::Error;
use reed_solomon_simd::engine::DefaultEngine;
use reed_solomon_simd::rate::{HighRateDecoder, HighRateEncoder, RateDecoder, RateEncoder};

/// Error that occurs when erasure coding data
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ErasureCodingError {
    /// Decoder error
    #[error("Decoder error: {0}")]
    DecoderError(#[from] Error),
}

/// Number of `u64` words needed for a bit per shard
const NUM_WORDS<const NUM_SHARDS: usize>: usize = NUM_SHARDS.div_ceil(u64::BITS as usize);

/// A bit per shard, typically saying whether that shard is present.
///
/// Bit `index % 64` of word `index / 64` corresponds to shard `index`, which is the same layout
/// `reed_solomon_simd::rate::ReceivedShards` uses.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ShardsBitmap<const NUM_SHARDS: usize> {
    words: [u64; NUM_WORDS::<NUM_SHARDS>],
}

impl<const NUM_SHARDS: usize> fmt::Debug for ShardsBitmap<NUM_SHARDS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShardsBitmap")
            .field("num_shards", &NUM_SHARDS)
            .field("num_set", &self.count())
            .finish()
    }
}

impl<const NUM_SHARDS: usize> Default for ShardsBitmap<NUM_SHARDS> {
    #[inline(always)]
    fn default() -> Self {
        Self::none()
    }
}

impl<const NUM_SHARDS: usize> ShardsBitmap<NUM_SHARDS> {
    /// Bitmap with no shards set
    #[inline(always)]
    pub const fn none() -> Self {
        Self {
            words: [0; NUM_WORDS::<NUM_SHARDS>],
        }
    }

    /// Bitmap with all shards set
    #[inline(always)]
    pub const fn all() -> Self {
        let mut this = Self {
            words: [u64::MAX; NUM_WORDS::<NUM_SHARDS>],
        };

        // Bits past the last shard must not be set
        let unused_bits = NUM_WORDS::<NUM_SHARDS> * u64::BITS as usize - NUM_SHARDS;
        if unused_bits > 0 {
            this.words[NUM_WORDS::<NUM_SHARDS> - 1] >>= unused_bits;
        }

        this
    }

    /// Whether the shard at `index` is set, `false` if `index` is out of bounds
    #[inline(always)]
    pub const fn get(&self, index: usize) -> bool {
        if index >= NUM_SHARDS {
            return false;
        }

        (self.words[index / u64::BITS as usize] >> (index % u64::BITS as usize)) & 1 == 1
    }

    /// Sets the shard at `index`, does nothing if `index` is out of bounds
    #[inline(always)]
    pub const fn set(&mut self, index: usize) {
        if index < NUM_SHARDS {
            self.words[index / u64::BITS as usize] |= 1 << (index % u64::BITS as usize);
        }
    }

    /// Unsets the shard at `index`, does nothing if `index` is out of bounds
    #[inline(always)]
    pub const fn unset(&mut self, index: usize) {
        if index < NUM_SHARDS {
            self.words[index / u64::BITS as usize] &= !(1 << (index % u64::BITS as usize));
        }
    }

    /// Number of shards that are set
    #[inline]
    pub const fn count(&self) -> usize {
        let mut count = 0;
        let mut word_index = 0;
        while word_index < NUM_WORDS::<NUM_SHARDS> {
            count += self.words[word_index].count_ones() as usize;
            word_index += 1;
        }

        count
    }
}

/// Which source and parity shards are present
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct ShardsPresent<const NUM_SHARDS: usize> {
    /// Which source shards are present
    pub source: ShardsBitmap<NUM_SHARDS>,
    /// Which parity shards are present
    pub parity: ShardsBitmap<NUM_SHARDS>,
}

impl<const NUM_SHARDS: usize> ShardsPresent<NUM_SHARDS> {
    /// Nothing is present
    #[inline(always)]
    pub const fn none() -> Self {
        Self {
            source: ShardsBitmap::none(),
            parity: ShardsBitmap::none(),
        }
    }

    /// Everything is present
    #[inline(always)]
    pub const fn all() -> Self {
        Self {
            source: ShardsBitmap::all(),
            parity: ShardsBitmap::all(),
        }
    }
}

/// Erasure coding abstraction.
///
/// Supports creation of parity shards and recovery of missing data.
#[derive(Debug, Clone)]
pub struct ErasureCoding;

impl Default for ErasureCoding {
    fn default() -> Self {
        Self::new()
    }
}

impl ErasureCoding {
    /// Create new erasure coding instance
    pub fn new() -> Self {
        Self {}
    }

    /// Extend contiguously stored source shards with parity shards
    pub fn extend<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &[[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError> {
        let mut encoder = new_encoder::<NUM_SHARDS, SHARD_BYTES>()?;

        for shard in source {
            encoder.add_original_shard(shard)?;
        }

        let result = encoder.encode()?;

        for (input, output) in result.recovery_iter().zip(parity) {
            output.copy_from_slice(input);
        }

        Ok(())
    }

    /// Extend source shards with parity shards, where shards are not stored contiguously
    pub fn extend_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        parity: [ParityShard; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsRef<[u8; SHARD_BYTES]>,
        ParityShard: AsMut<[u8; SHARD_BYTES]>,
    {
        let mut encoder = new_encoder::<NUM_SHARDS, SHARD_BYTES>()?;

        for shard in &source {
            encoder.add_original_shard(shard.as_ref())?;
        }

        let result = encoder.encode()?;

        let mut parity = parity;
        for (input, output) in result.recovery_iter().zip(&mut parity) {
            output.as_mut().copy_from_slice(input);
        }

        Ok(())
    }

    /// Recover missing source shards in place, with everything stored contiguously.
    ///
    /// Parity shards are inputs only, missing ones are simply not used. Prefer this over
    /// [`Self::recover_all()`] when parity shards are not needed.
    pub fn recover_source<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &[[u8; SHARD_BYTES]; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError> {
        let mut decoder = new_decoder::<NUM_SHARDS, SHARD_BYTES>()?;

        for (index, shard) in source.iter().enumerate() {
            if present.source.get(index) {
                decoder.add_original_shard(index, shard)?;
            }
        }
        for (index, shard) in parity.iter().enumerate() {
            if present.parity.get(index) {
                decoder.add_recovery_shard(index, shard)?;
            }
        }

        let result = decoder.decode()?;

        for (index, shard) in source.iter_mut().enumerate() {
            if !present.source.get(index) {
                shard.copy_from_slice(restored_original(&result, index));
            }
        }

        Ok(())
    }

    /// Recover missing source shards in place, where shards are not stored contiguously.
    ///
    /// Parity shards are inputs only, so missing ones simply have no memory.
    pub fn recover_source_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        source_present: &ShardsBitmap<NUM_SHARDS>,
        parity: [Option<ParityShard>; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsMut<[u8; SHARD_BYTES]>,
        ParityShard: AsRef<[u8; SHARD_BYTES]>,
    {
        let mut decoder = new_decoder::<NUM_SHARDS, SHARD_BYTES>()?;

        let mut source = source;
        for (index, shard) in source.iter_mut().enumerate() {
            if source_present.get(index) {
                decoder.add_original_shard(index, shard.as_mut().as_slice())?;
            }
        }
        for (index, shard) in parity.iter().enumerate() {
            if let Some(shard) = shard {
                decoder.add_recovery_shard(index, shard.as_ref())?;
            }
        }

        let result = decoder.decode()?;

        for (index, shard) in source.iter_mut().enumerate() {
            if !source_present.get(index) {
                shard
                    .as_mut()
                    .copy_from_slice(restored_original(&result, index));
            }
        }

        Ok(())
    }

    /// Recover missing source and parity shards in place, with everything stored contiguously.
    ///
    /// Use [`Self::recover_source()`] instead if parity shards are not needed.
    pub fn recover_all<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError> {
        self.recover_source(source, parity, present)?;

        if present.parity.count() == NUM_SHARDS {
            return Ok(());
        }

        // Source shards are complete at this point, so missing parity shards are simply encoded
        // again
        let mut encoder = new_encoder::<NUM_SHARDS, SHARD_BYTES>()?;

        for shard in &*source {
            encoder.add_original_shard(shard)?;
        }

        let result = encoder.encode()?;

        for (index, shard) in parity.iter_mut().enumerate() {
            if !present.parity.get(index) {
                shard.copy_from_slice(recovery(&result, index));
            }
        }

        Ok(())
    }

    /// Recover missing source and parity shards in place, where shards are not stored contiguously
    pub fn recover_all_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        parity: [ParityShard; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsMut<[u8; SHARD_BYTES]>,
        ParityShard: AsMut<[u8; SHARD_BYTES]>,
    {
        let mut source = source;
        let mut parity = parity;

        {
            let mut decoder = new_decoder::<NUM_SHARDS, SHARD_BYTES>()?;

            for (index, shard) in source.iter_mut().enumerate() {
                if present.source.get(index) {
                    decoder.add_original_shard(index, shard.as_mut().as_slice())?;
                }
            }
            for (index, shard) in parity.iter_mut().enumerate() {
                if present.parity.get(index) {
                    decoder.add_recovery_shard(index, shard.as_mut().as_slice())?;
                }
            }

            let result = decoder.decode()?;

            for (index, shard) in source.iter_mut().enumerate() {
                if !present.source.get(index) {
                    shard
                        .as_mut()
                        .copy_from_slice(restored_original(&result, index));
                }
            }
        }

        if present.parity.count() == NUM_SHARDS {
            return Ok(());
        }

        let mut encoder = new_encoder::<NUM_SHARDS, SHARD_BYTES>()?;

        for shard in &mut source {
            encoder.add_original_shard(shard.as_mut().as_slice())?;
        }

        let result = encoder.encode()?;

        for (index, shard) in parity.iter_mut().enumerate() {
            if !present.parity.get(index) {
                shard.as_mut().copy_from_slice(recovery(&result, index));
            }
        }

        Ok(())
    }
}

#[inline(always)]
fn new_encoder<const NUM_SHARDS: usize, const SHARD_BYTES: usize>()
-> Result<HighRateEncoder<DefaultEngine>, ErasureCodingError> {
    Ok(HighRateEncoder::new(
        NUM_SHARDS,
        NUM_SHARDS,
        SHARD_BYTES,
        DefaultEngine::new(),
        None,
    )?)
}

#[inline(always)]
fn new_decoder<const NUM_SHARDS: usize, const SHARD_BYTES: usize>()
-> Result<HighRateDecoder<DefaultEngine>, ErasureCodingError> {
    Ok(HighRateDecoder::new(
        NUM_SHARDS,
        NUM_SHARDS,
        SHARD_BYTES,
        DefaultEngine::new(),
        None,
    )?)
}

#[inline(always)]
fn restored_original<'a>(
    result: &'a reed_solomon_simd::DecoderResult<'_>,
    index: usize,
) -> &'a [u8] {
    result
        .restored_original(index)
        .expect("Always corresponds to a missing source shard; qed")
}

#[inline(always)]
fn recovery<'a>(result: &'a reed_solomon_simd::EncoderResult<'_>, index: usize) -> &'a [u8] {
    result
        .recovery(index)
        .expect("Always corresponds to a missing parity shard; qed")
}
