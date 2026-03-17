//! Helper for incoming super segment header requests.
//!
//! Handle (i.e. answer) incoming super segment headers requests from a remote peer received via
//! `RequestResponsesBehaviour` with generic [`GenericRequestHandler`].

use super::generic_request_handler::{GenericRequest, GenericRequestHandler};
use ab_core_primitives::segments::{SuperSegmentHeader, SuperSegmentIndex};
use parity_scale_codec::{Decode, Encode};
use std::sync::Arc;

/// Super segment header by super segment indices protocol request
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub enum SuperSegmentHeaderRequest {
    /// Super segment headers by super segment indices
    SuperSegmentIndices {
        /// Super segment indices to get
        // TODO: Use `Arc<[SuperSegmentIndex]>` once
        //  https://github.com/paritytech/parity-scale-codec/issues/633 is resolved
        super_segment_indices: Arc<Vec<SuperSegmentIndex>>,
    },
    /// Defines how many super segment headers to return.
    ///
    /// Super segments will be in ascending order.
    LastSuperSegmentHeaders {
        /// Number of segment headers to return
        limit: u32,
    },
}

impl GenericRequest for SuperSegmentHeaderRequest {
    const PROTOCOL_NAME: &'static str = "/subspace/super-segment-headers-by-indexes/0.1.0";
    const LOG_TARGET: &'static str = "super-segment-headers-by-indexes-request-response-handler";
    type Response = SuperSegmentHeaderResponse;
}

/// Super segment header by super segment indices protocol response
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub struct SuperSegmentHeaderResponse {
    /// Super segment headers
    pub super_segment_headers: Vec<SuperSegmentHeader>,
}

/// Create a new `super-segment-headers-by-indexes` request handler
pub type SuperSegmentHeaderBySegmentIndexesRequestHandler =
    GenericRequestHandler<SuperSegmentHeaderRequest>;
