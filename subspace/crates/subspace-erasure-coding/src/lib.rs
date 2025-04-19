#![feature(trusted_len)]
#![feature(
    maybe_uninit_slice,
    maybe_uninit_uninit_array_transpose,
    maybe_uninit_write_slice
)]
#![no_std]

#[cfg(test)]
mod tests;

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::iter::TrustedLen;
use core::mem::MaybeUninit;
use reed_solomon_simd::{ReedSolomonDecoder, ReedSolomonEncoder};

/// State of the shard for recovery
pub enum RecoveryShardState<PresentShard, MissingShard> {
    /// Shard is present and will be used for recovery
    Present(PresentShard),
    /// Shard is missing and needs to be recovered
    MissingRecover(MissingShard),
    /// Shard is missing and does not need to be recovered
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
    ) -> Result<(), String>
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
        // TODO: Fix error type
        let mut encoder =
            ReedSolomonEncoder::new(source.size_hint().0, parity.size_hint().0, shard_byte_len)
                .map_err(|error| error.to_string())?;

        for shard in source {
            encoder
                .add_original_shard(shard)
                .map_err(|error| error.to_string())?;
        }

        let result = encoder.encode().map_err(|error| error.to_string())?;

        for (input, mut output) in result.recovery_iter().zip(parity) {
            let output = output.as_mut();
            if output.len() != shard_byte_len {
                return Err("Wrong parity shard byte length; qed".to_string());
            }
            output.copy_from_slice(input);
        }

        Ok(())
    }

    /// Recover missing shards
    // TODO: Refactor to use byte slices once shards are no longer interleaved
    pub fn recover<'a, SourceIter, ParityIter>(
        &self,
        source: SourceIter,
        parity: ParityIter,
    ) -> Result<(), String>
    where
        SourceIter: TrustedLen<Item = RecoveryShardState<&'a [u8], &'a mut [u8]>>,
        ParityIter: TrustedLen<Item = RecoveryShardState<&'a [u8], &'a mut [u8]>>,
    {
        // TODO: Fix error type
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

        let mut decoder = ReedSolomonDecoder::new(num_source, num_parity, shard_byte_len)
            .map_err(|error| error.to_string())?;

        let mut all_source_shards = vec![MaybeUninit::uninit(); num_source];
        let mut source_shards_to_recover = Vec::new();
        for (index, shard) in source {
            match shard {
                RecoveryShardState::Present(shard_bytes) => {
                    all_source_shards[index].write(shard_bytes);
                    decoder
                        .add_original_shard(index, shard_bytes)
                        .map_err(|error| error.to_string())?;
                }
                RecoveryShardState::MissingRecover(shard_bytes) => {
                    source_shards_to_recover.push((index, shard_bytes));
                }
                RecoveryShardState::MissingIgnore => {}
            }
        }

        let mut parity_shards_to_recover = Vec::new();
        for (index, shard) in parity {
            match shard {
                RecoveryShardState::Present(shard_bytes) => {
                    decoder
                        .add_recovery_shard(index, shard_bytes)
                        .map_err(|error| error.to_string())?;
                }
                RecoveryShardState::MissingRecover(shard_bytes) => {
                    parity_shards_to_recover.push((index, shard_bytes));
                }
                RecoveryShardState::MissingIgnore => {}
            }
        }

        let result = decoder.decode().map_err(|error| error.to_string())?;

        for (index, output) in source_shards_to_recover {
            if output.len() != shard_byte_len {
                return Err("Wrong source shard byte length; qed".to_string());
            }
            let shard = result
                .restored_original(index)
                .expect("Always corresponds to a missing original shard; qed");
            all_source_shards[index].write(shard);
            output.copy_from_slice(shard);
        }

        let all_source_shards = unsafe { all_source_shards.assume_init_ref() };
        if !parity_shards_to_recover.is_empty() {
            let mut encoder = ReedSolomonEncoder::new(num_source, num_parity, shard_byte_len)
                .map_err(|error| error.to_string())?;

            for shard in all_source_shards {
                encoder
                    .add_original_shard(shard)
                    .map_err(|error| error.to_string())?;
            }

            let result = encoder.encode().map_err(|error| error.to_string())?;

            for (index, output) in parity_shards_to_recover {
                if output.len() != shard_byte_len {
                    return Err("Wrong parity shard byte length; qed".to_string());
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
