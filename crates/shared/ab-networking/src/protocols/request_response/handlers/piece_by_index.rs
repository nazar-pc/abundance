//! Helper for incoming piece requests.
//!
//! Request handler can be created with [`PieceByIndexRequestHandler`].

#[cfg(test)]
mod tests;

use crate::protocols::request_response::handlers::generic_request_handler::{
    GenericRequest, GenericRequestHandler, encode_to_buffer,
};
use crate::protocols::request_response::handlers::{decode_piece, encode_piece};
use crate::protocols::request_response::request_response_factory::ResponsePayload;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align1;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use parity_scale_codec::{Decode, Encode};
use std::sync::Arc;

/// Piece-by-index request
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct PieceByIndexRequest {
    /// Request key - piece index
    pub piece_index: PieceIndex,
    /// Additional pieces that requester is interested in if they are cached locally
    // TODO: Use `Arc<[PieceIndex]>` once
    //  https://github.com/paritytech/parity-scale-codec/issues/633 is resolved
    pub cached_pieces: Arc<Vec<PieceIndex>>,
}

impl GenericRequest for PieceByIndexRequest {
    const PROTOCOL_NAME: &'static str = "/subspace/piece-by-index/0.1.0";
    const LOG_TARGET: &'static str = "piece-by-index-request-response-handler";
    type Response = PieceByIndexResponse;

    /// Piece is not copied into the payload
    fn encode_response(response: &Self::Response) -> Option<ResponsePayload> {
        let PieceByIndexResponse {
            piece,
            cached_pieces,
        } = response;

        let mut payload = ResponsePayload::default();
        if let Some(piece) = piece {
            // `Some` variant of `Option`
            encode_piece(&mut payload, &1u8, piece)?;
        } else {
            payload.push(encode_to_buffer(&None::<Piece>)?);
        }
        payload.push(encode_to_buffer(cached_pieces)?);

        Some(payload)
    }

    /// Piece references the payload without copying it
    fn decode_response(
        payload: SharedAlignedBuffer<Align1>,
    ) -> Result<Self::Response, parity_scale_codec::Error> {
        let mut input = payload.as_slice();
        let piece = match u8::decode(&mut input)? {
            0 => None,
            1 => Some(decode_piece(&payload, &mut input)?),
            _ => {
                return Err("Invalid `Option<Piece>` variant".into());
            }
        };
        let cached_pieces = Vec::decode(&mut input)?;

        Ok(PieceByIndexResponse {
            piece,
            cached_pieces,
        })
    }
}

impl PieceByIndexRequest {
    /// Max number of cached pieces to accept per request, equals to the number of source shards in
    /// a sector and fits nicely into a single TCP packet
    pub const RECOMMENDED_LIMIT: usize = 128;
}

/// Piece-by-index response, may be cached piece or stored in one of the farms
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub struct PieceByIndexResponse {
    /// Piece, if found
    pub piece: Option<Piece>,
    /// Additional pieces that requester is interested in and are cached locally, order from
    /// request is not preserved
    pub cached_pieces: Vec<PieceIndex>,
}

/// Piece-by-index request handler
pub type PieceByIndexRequestHandler = GenericRequestHandler<PieceByIndexRequest>;
