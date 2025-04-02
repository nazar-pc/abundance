//! Inherents for Subspace consensus

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use parity_scale_codec::{Decode, Encode};
use sp_inherents::{Error, InherentData, InherentIdentifier, IsFatalError};
use subspace_core_primitives::segments::SegmentHeader;

/// The Subspace inherent identifier.
pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"subspace";

/// Errors that can occur while checking segment headers.
#[derive(Debug, Encode)]
#[cfg_attr(feature = "std", derive(Decode))]
pub enum InherentError {
    /// List of segment headers is not correct.
    IncorrectSegmentHeadersList {
        /// Expected list of segment headers according to node's inherents.
        expected: Vec<SegmentHeader>,
        /// List of segment headers contained within proposed block.
        actual: Vec<SegmentHeader>,
    },
    /// List of segment headers is not present.
    MissingSegmentHeadersList,
}

impl IsFatalError for InherentError {
    fn is_fatal_error(&self) -> bool {
        true
    }
}

/// The type of the Subspace inherent data.
#[derive(Debug, Encode, Decode)]
pub struct InherentType {
    /// Segment headers expected to be included in the block.
    pub segment_headers: Vec<SegmentHeader>,
}

/// Auxiliary trait to extract Subspace inherent data.
pub trait SubspaceInherentData {
    /// Get Subspace inherent data.
    fn subspace_inherent_data(&self) -> Result<Option<InherentType>, Error>;

    /// Replace Subspace inherent data.
    fn replace_subspace_inherent_data(&mut self, new: InherentType);
}

impl SubspaceInherentData for InherentData {
    fn subspace_inherent_data(&self) -> Result<Option<InherentType>, Error> {
        self.get_data(&INHERENT_IDENTIFIER)
    }

    fn replace_subspace_inherent_data(&mut self, new: InherentType) {
        self.replace_data(INHERENT_IDENTIFIER, &new);
    }
}

/// Provides the segment headers inherent data for Subspace.
#[cfg(feature = "std")]
pub struct InherentDataProvider {
    data: InherentType,
}

#[cfg(feature = "std")]
impl InherentDataProvider {
    /// Create new inherent data provider from the given `segment_headers`.
    pub fn new(segment_headers: Vec<SegmentHeader>) -> Self {
        Self {
            data: InherentType { segment_headers },
        }
    }

    /// Returns the `data` of this inherent data provider.
    pub fn data(&self) -> &InherentType {
        &self.data
    }
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for InherentDataProvider {
    async fn provide_inherent_data(&self, inherent_data: &mut InherentData) -> Result<(), Error> {
        inherent_data.put_data(INHERENT_IDENTIFIER, &self.data)
    }

    async fn try_handle_error(
        &self,
        identifier: &InherentIdentifier,
        error: &[u8],
    ) -> Option<Result<(), Error>> {
        if *identifier != INHERENT_IDENTIFIER {
            return None;
        }

        let error = InherentError::decode(&mut &*error).ok()?;

        Some(Err(Error::Application(Box::from(format!("{error:?}")))))
    }
}
