#![feature(trusted_len)]
#![no_std]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::iter::TrustedLen;
use core::mem::MaybeUninit;
use reed_solomon_simd::Error;
use reed_solomon_simd::engine::DefaultEngine;
use reed_solomon_simd::rate::{HighRateDecoder, HighRateEncoder, RateDecoder, RateEncoder};

/// Error that occurs when calling [`ErasureCoding::recover()`]
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ErasureCodingError {
    /// Decoder error
    #[error("Decoder error: {0}")]
    DecoderError(#[from] Error),
    /// Ignored source shard
    #[error("Ignored source shard {index}")]
    IgnoredSourceShard {
        /// Shard index
        index: usize,
    },
    /// Wrong source shard byte length
    #[error("Wrong source shard byte length: expected {expected}, actual {actual}")]
    WrongSourceShardByteLength { expected: usize, actual: usize },
    /// Wrong parity shard byte length
    #[error("Wrong parity shard byte length: expected {expected}, actual {actual}")]
    WrongParityShardByteLength { expected: usize, actual: usize },
}

/// State of the shard for recovery
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RecoveryShardState<PresentShard, MissingShard> {
    /// Shard is present and will be used for recovery
    Present(PresentShard),
    /// Shard is missing and needs to be recovered
    MissingRecover(MissingShard),
    /// Shard is missing and does not need to be recovered.
    ///
    /// This is only allowed for parity shards, all source shards must always be present or
    /// recovered.
    MissingIgnore,
}

/// Erasure coding abstraction.
///
/// Supports creation of parity records and recovery of missing data.
#[derive(Debug, Clone)]
pub struct ErasureCoding {}

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

    /// Extend sources using erasure coding
    pub fn extend<'a, SourceIter, ParityIter, SourceBytes, ParityBytes>(
        &self,
        source: SourceIter,
        parity: ParityIter,
    ) -> Result<(), ErasureCodingError>
    where
        SourceIter: TrustedLen<Item = SourceBytes>,
        ParityIter: TrustedLen<Item = ParityBytes>,
        SourceBytes: AsRef<[u8]> + 'a,
        ParityBytes: AsMut<[u8]> + 'a,
    {
        let mut source = source.peekable();
        let shard_byte_len = source
            .peek()
            .map(|shard| shard.as_ref().len())
            .unwrap_or_default();

        let mut encoder = HighRateEncoder::new(
            source.size_hint().0,
            parity.size_hint().0,
            shard_byte_len,
            DefaultEngine::new(),
            None,
        )?;

        for shard in source {
            encoder.add_original_shard(shard)?;
        }

        let result = encoder.encode()?;

        for (input, mut output) in result.recovery_iter().zip(parity) {
            let output = output.as_mut();
            if output.len() != shard_byte_len {
                return Err(ErasureCodingError::WrongParityShardByteLength {
                    expected: shard_byte_len,
                    actual: output.len(),
                });
            }
            output.copy_from_slice(input);
        }

        Ok(())
    }

    /// Recover missing shards
    pub fn recover<'a, SourceIter, ParityIter>(
        &self,
        source: SourceIter,
        parity: ParityIter,
    ) -> Result<(), ErasureCodingError>
    where
        SourceIter: TrustedLen<Item = RecoveryShardState<&'a [u8], &'a mut [u8]>>,
        ParityIter: TrustedLen<Item = RecoveryShardState<&'a [u8], &'a mut [u8]>>,
    {
        let num_source = source.size_hint().0;
        let num_parity = parity.size_hint().0;
        let mut source = source.enumerate().peekable();
        let mut parity = parity.enumerate().peekable();
        let mut shard_byte_len = 0;

        while let Some((_, shard)) = source.peek_mut() {
            match shard {
                RecoveryShardState::Present(shard_bytes) => {
                    shard_byte_len = shard_bytes.len();
                    break;
                }
                RecoveryShardState::MissingRecover(shard_bytes) => {
                    shard_byte_len = shard_bytes.len();
                    break;
                }
                RecoveryShardState::MissingIgnore => {
                    // Skip, it is inconsequential here
                    source.next();
                }
            }
        }
        if shard_byte_len == 0 {
            while let Some((_, shard)) = parity.peek_mut() {
                match shard {
                    RecoveryShardState::Present(shard_bytes) => {
                        shard_byte_len = shard_bytes.len();
                        break;
                    }
                    RecoveryShardState::MissingRecover(shard_bytes) => {
                        shard_byte_len = shard_bytes.len();
                        break;
                    }
                    RecoveryShardState::MissingIgnore => {
                        // Skip, it is inconsequential here
                        parity.next();
                    }
                }
            }
        }

        let mut all_source_shards = vec![MaybeUninit::uninit(); num_source];
        let mut parity_shards_to_recover = Vec::new();

        {
            let mut decoder = HighRateDecoder::new(
                num_source,
                num_parity,
                shard_byte_len,
                DefaultEngine::new(),
                None,
            )?;

            let mut source_shards_to_recover = Vec::new();
            for (index, shard) in source {
                match shard {
                    RecoveryShardState::Present(shard_bytes) => {
                        all_source_shards[index].write(shard_bytes);
                        decoder.add_original_shard(index, shard_bytes)?;
                    }
                    RecoveryShardState::MissingRecover(shard_bytes) => {
                        source_shards_to_recover.push((index, shard_bytes));
                    }
                    RecoveryShardState::MissingIgnore => {
                        return Err(ErasureCodingError::IgnoredSourceShard { index });
                    }
                }
            }

            for (index, shard) in parity {
                match shard {
                    RecoveryShardState::Present(shard_bytes) => {
                        decoder.add_recovery_shard(index, shard_bytes)?;
                    }
                    RecoveryShardState::MissingRecover(shard_bytes) => {
                        parity_shards_to_recover.push((index, shard_bytes));
                    }
                    RecoveryShardState::MissingIgnore => {}
                }
            }

            let result = decoder.decode()?;

            for (index, output) in source_shards_to_recover {
                if output.len() != shard_byte_len {
                    return Err(ErasureCodingError::WrongSourceShardByteLength {
                        expected: shard_byte_len,
                        actual: output.len(),
                    });
                }
                let shard = result
                    .restored_original(index)
                    .expect("Always corresponds to a missing original shard; qed");
                output.copy_from_slice(shard);
                all_source_shards[index].write(output);
            }
        }

        if !parity_shards_to_recover.is_empty() {
            // SAFETY: All `all_source_shards` are either initialized from the start or recovered
            let all_source_shards = unsafe { all_source_shards.assume_init_ref() };

            let mut encoder = HighRateEncoder::new(
                num_source,
                num_parity,
                shard_byte_len,
                DefaultEngine::new(),
                None,
            )?;

            for shard in all_source_shards {
                encoder.add_original_shard(shard)?;
            }

            let result = encoder.encode()?;

            for (index, output) in parity_shards_to_recover {
                if output.len() != shard_byte_len {
                    return Err(ErasureCodingError::WrongParityShardByteLength {
                        expected: shard_byte_len,
                        actual: output.len(),
                    });
                }
                output.copy_from_slice(
                    result
                        .recovery(index)
                        .expect("Always corresponds to a missing parity shard; qed"),
                );
            }
        }

        Ok(())
    }
}
