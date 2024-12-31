// TODO: This should probably be reduced to something like 32-bit or 64-bit with compile-time
//  checks for collisions, 256-bit is unnecessarily redundant here
// TODO: Probably more efficient metadata structure to locate fingerprints and metadata of specific methods
//  quicker
/// Hash of method's compact metadata
#[derive(Copy, Clone)]
#[repr(C)]
pub struct MethodFingerprint([u8; 32]);

impl MethodFingerprint {
    /// Create new method fingerprint from compact metadata hash
    pub const fn new(compact_metadata: &[u8]) -> Self {
        // TODO: Hash
        let _ = compact_metadata;
        Self([0; 32])
    }
}

/// Marker trait for external arguments when calling methods.
///
/// # Safety
/// Struct that implements this trait must be `#[repr(C)]` and valid `ExternalArgs` for the contract
/// method being invoked.
///
/// **Do not implement this type explicitly!** It implementation is automatically generated by the
/// macro which generates smart contract implementation.
pub unsafe trait ExternalArgs {
    /// Fingerprint of the method being called
    const FINGERPRINT: &MethodFingerprint;
}
