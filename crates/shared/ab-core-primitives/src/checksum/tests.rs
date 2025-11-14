use super::Blake3Checksummed;
use crate::hashes::Blake3Hash;
use chacha20::ChaCha8Rng;
use chacha20::rand_core::{RngCore, SeedableRng};
use parity_scale_codec::{Decode, Encode};

#[test]
fn basic() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());
    let random_bytes = {
        let mut random_bytes = [0u8; 64];
        rng.fill_bytes(&mut random_bytes);
        random_bytes
    };

    let plain_encoding = random_bytes.encode();
    let checksummed_encoding = Blake3Checksummed(random_bytes).encode();

    // Encoding is extended with checksum
    assert_eq!(
        plain_encoding.len() + Blake3Hash::SIZE,
        checksummed_encoding.len()
    );

    // Decoding succeeds
    let Blake3Checksummed(decoded_random_bytes) =
        Blake3Checksummed::<[u8; 64]>::decode(&mut checksummed_encoding.as_slice()).unwrap();
    // Decodes to original data
    assert_eq!(random_bytes, decoded_random_bytes);

    // Non-checksummed encoding fails to decode
    assert!(Blake3Checksummed::<[u8; 64]>::decode(&mut plain_encoding.as_slice()).is_err());
    // Incorrectly checksummed data fails to decode
    assert!(Blake3Checksummed::<[u8; 32]>::decode(&mut random_bytes.as_ref()).is_err());
}
