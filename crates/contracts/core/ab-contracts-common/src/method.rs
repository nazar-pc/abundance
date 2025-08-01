use crate::metadata::ContractMetadataKind;
use ab_blake3::const_hash;
use ab_core_primitives::hashes::Blake3Hash;
use ab_io_type::trivial_type::TrivialType;
use derive_more::Display;

/// Hash of method's compact metadata, which uniquely represents method signature.
///
/// While nothing can be said about method implementation, matching method fingerprint means method
/// name, inputs and outputs are what they are expected to be (struct and field names are ignored as
/// explained in [`ContractMetadataKind::compact`].
#[derive(Debug, Display, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, TrivialType)]
#[repr(C)]
pub struct MethodFingerprint(Blake3Hash);

impl MethodFingerprint {
    /// Create a new method fingerprint from its metadata.
    ///
    /// `None` is returned for invalid metadata (see
    /// [`ContractMetadataKind::compact_external_args()`] for details).
    pub const fn new(method_metadata: &[u8]) -> Option<Self> {
        // `?` is not supported in `const` environment
        let Some((compact_metadata_scratch, compact_metadata_size)) =
            ContractMetadataKind::compact_external_args(method_metadata)
        else {
            return None;
        };
        // The same as `&compact_metadata_scratch[..compact_metadata_size]`, but it is not allowed
        // in const environment yet
        let compact_metadata = compact_metadata_scratch.split_at(compact_metadata_size).0;

        Some(Self(Blake3Hash::new(const_hash(compact_metadata))))
    }

    #[inline(always)]
    pub const fn to_bytes(&self) -> &[u8; Blake3Hash::SIZE] {
        self.0.as_bytes()
    }
}

/// Marker trait for external arguments when calling methods.
///
/// # Safety
/// Struct that implements this trait must be `#[repr(C)]` and valid `ExternalArgs` for the contract
/// method being called.
///
/// **Do not implement this trait explicitly!** Implementation is automatically generated by the
/// macro which generates contract implementation.
pub unsafe trait ExternalArgs {
    /// Fingerprint of the method being called
    const FINGERPRINT: MethodFingerprint;
    /// Metadata that corresponds to a method being called
    const METADATA: &[u8];
}
