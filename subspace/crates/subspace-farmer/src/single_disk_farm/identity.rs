//! Farm identity

use ab_core_primitives::hashes::Blake3Hash;
use ed25519_zebra::{SigningKey, VerificationKey};
use parity_scale_codec::{Decode, Encode};
use rand::rngs::OsRng;
use std::path::Path;
use std::{fmt, fs, io};
use subspace_verification::ed25519::{Ed25519PublicKey, Ed25519Signature};
use thiserror::Error;
use tracing::debug;
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, Encode, Decode, Zeroize)]
struct IdentityFileContents {
    secret_key: [u8; 32],
}

/// Errors happening when trying to create/open single disk farm
#[derive(Debug, Error)]
pub enum IdentityError {
    /// I/O error occurred
    #[error("Identity I/O error: {0}")]
    Io(#[from] io::Error),
    /// Decoding error
    #[error("Decoding error: {0}")]
    Decoding(#[from] parity_scale_codec::Error),
}

/// `Identity` struct is an abstraction of public & secret key related operations.
///
/// It is basically a wrapper of the keypair (which holds public & secret keys)
/// and a context that will be used for signing.
#[derive(Clone)]
pub struct Identity {
    signing_key: SigningKey,
}

impl fmt::Debug for Identity {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Identity")
            .field("keypair", &self.signing_key)
            .finish_non_exhaustive()
    }
}

impl Identity {
    pub(crate) const FILE_NAME: &'static str = "identity.bin";

    /// Size of the identity file on disk
    pub fn file_size() -> usize {
        IdentityFileContents { secret_key: [0; _] }.encoded_size()
    }

    /// Opens the existing identity, or creates a new one.
    pub fn open_or_create<B: AsRef<Path>>(base_directory: B) -> Result<Self, IdentityError> {
        if let Some(identity) = Self::open(base_directory.as_ref())? {
            Ok(identity)
        } else {
            Self::create(base_directory)
        }
    }

    /// Opens the existing identity, returns `Ok(None)` if it doesn't exist.
    pub fn open<B: AsRef<Path>>(base_directory: B) -> Result<Option<Self>, IdentityError> {
        let identity_file = base_directory.as_ref().join(Self::FILE_NAME);
        if identity_file.exists() {
            debug!("Opening existing keypair");
            let bytes = Zeroizing::new(fs::read(identity_file)?);
            let IdentityFileContents { secret_key } =
                IdentityFileContents::decode(&mut bytes.as_ref())?;

            let signing_key = SigningKey::from(secret_key);

            Ok(Some(Self { signing_key }))
        } else {
            debug!("Existing keypair not found");
            Ok(None)
        }
    }

    /// Creates new identity, overrides identity that might already exist.
    pub fn create<B: AsRef<Path>>(base_directory: B) -> Result<Self, IdentityError> {
        let identity_file = base_directory.as_ref().join(Self::FILE_NAME);
        debug!("Generating new keypair");

        let signing_key = SigningKey::new(OsRng);

        let identity_file_contents = Zeroizing::new(IdentityFileContents {
            secret_key: signing_key.into(),
        });

        fs::write(
            identity_file,
            Zeroizing::new(identity_file_contents.encode()),
        )?;

        Ok(Self { signing_key })
    }

    /// Returns the public key of the identity.
    pub fn public_key(&self) -> Ed25519PublicKey {
        Ed25519PublicKey::from(<[u8; 32]>::from(VerificationKey::from(&self.signing_key)))
    }

    /// Returns the secret key of the identity.
    pub fn secret_key(&self) -> [u8; 32] {
        self.signing_key.into()
    }

    // TODO: Rename reward hash to `pre_seal_hash`
    /// Sign reward hash.
    pub fn sign_reward_hash(&self, header_hash: &Blake3Hash) -> Ed25519Signature {
        Ed25519Signature::from(self.signing_key.sign(header_hash.as_ref()).to_bytes())
    }
}
