#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::num::NonZeroUsize;
use kzg::{FFTSettings, PolyRecover, DAS};
use rust_kzg_blst::types::fft_settings::FsFFTSettings;
use rust_kzg_blst::types::poly::FsPoly;
use subspace_kzg::Scalar;

/// Erasure coding abstraction.
///
/// Supports creation of parity records and recovery of missing data.
#[derive(Debug, Clone)]
pub struct ErasureCoding {
    fft_settings: Arc<FsFFTSettings>,
}

impl ErasureCoding {
    /// Create new erasure coding instance.
    ///
    /// Number of shards supported is `2^scale`, half of shards are source data and the other half
    /// are parity.
    pub fn new(scale: NonZeroUsize) -> Result<Self, String> {
        let fft_settings = Arc::new(FsFFTSettings::new(scale.get())?);

        Ok(Self { fft_settings })
    }

    /// Max number of shards supported (both source and parity together)
    pub fn max_shards(&self) -> usize {
        self.fft_settings.max_width
    }

    /// Extend sources using erasure coding.
    ///
    /// Returns parity data.
    pub fn extend(&self, source: &[Scalar]) -> Result<Vec<Scalar>, String> {
        // TODO: das_fft_extension modifies buffer internally, it needs to change to use
        //  pre-allocated buffer instead of allocating a new one
        self.fft_settings
            .das_fft_extension(Scalar::slice_to_repr(source))
            .map(Scalar::vec_from_repr)
    }

    /// Recovery of missing shards from given shards (at least 1/2 should be `Some`).
    ///
    /// Both in input and output source shards are interleaved with parity shards:
    /// source, parity, source, parity, ...
    pub fn recover(&self, shards: &[Option<Scalar>]) -> Result<Vec<Scalar>, String> {
        let poly = FsPoly::recover_poly_from_samples(
            Scalar::slice_option_to_repr(shards),
            &self.fft_settings,
        )?;

        Ok(Scalar::vec_from_repr(poly.coeffs))
    }

    /// Recovery of source shards from given shards (at least 1/2 should be `Some`).
    ///
    /// The same as [`ErasureCoding::recover()`], but returns only source shards in form of an
    /// iterator.
    pub fn recover_source(
        &self,
        shards: &[Option<Scalar>],
    ) -> Result<impl ExactSizeIterator<Item = Scalar>, String> {
        Ok(self.recover(shards)?.into_iter().step_by(2))
    }
}
