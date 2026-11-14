//! Helper for incoming cached piece requests.
//!
//! Request handler can be created with [`CachedPieceByIndexRequestHandler`].

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
use derive_more::{Deref, DerefMut, From, Into};
use libp2p::kad::K_VALUE;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use multihash::Multihash;
use parity_scale_codec::{Compact, CompactLen, Decode, Encode, EncodeLike, Input, Output};
use std::sync::Arc;

/// Cached-piece-by-index request.
///
/// This is similar to `PieceByIndexRequest`, but will only respond with cached pieces.
#[derive(Debug, Clone, Eq, PartialEq, Encode, Decode)]
pub struct CachedPieceByIndexRequest {
    /// Request key - piece index
    pub piece_index: PieceIndex,
    /// Additional pieces that requester is interested in if they are cached locally
    // TODO: Use `Arc<[PieceIndex]>` once
    //  https://github.com/paritytech/parity-scale-codec/issues/633 is resolved
    pub cached_pieces: Arc<Vec<PieceIndex>>,
}

impl GenericRequest for CachedPieceByIndexRequest {
    const PROTOCOL_NAME: &'static str = "/subspace/cached-piece-by-index/0.1.0";
    const LOG_TARGET: &'static str = "cached-piece-by-index-request-response-handler";
    type Response = CachedPieceByIndexResponse;

    /// Piece is not copied into the payload
    fn encode_response(response: &Self::Response) -> Option<ResponsePayload> {
        let CachedPieceByIndexResponse {
            result,
            cached_pieces,
        } = response;

        let mut payload = ResponsePayload::default();
        match result {
            PieceResult::Piece(piece) => {
                // `PieceResult::Piece` variant
                encode_piece(&mut payload, &0u8, piece)?;
            }
            PieceResult::ClosestPeers(_closest_peers) => {
                payload.push(encode_to_buffer(result)?);
            }
        }
        payload.push(encode_to_buffer(cached_pieces)?);

        Some(payload)
    }

    /// Piece references the payload without copying it
    fn decode_response(
        payload: SharedAlignedBuffer<Align1>,
    ) -> Result<Self::Response, parity_scale_codec::Error> {
        let mut input = payload.as_slice();
        let result = match u8::decode(&mut input)? {
            0 => PieceResult::Piece(decode_piece(&payload, &mut input)?),
            1 => PieceResult::ClosestPeers(ClosestPeers::decode(&mut input)?),
            _ => {
                return Err("Invalid `PieceResult` variant".into());
            }
        };
        let cached_pieces = Vec::decode(&mut input)?;

        Ok(CachedPieceByIndexResponse {
            result,
            cached_pieces,
        })
    }
}

impl CachedPieceByIndexRequest {
    /// Max number of cached piece indexes to accept per request, equals to the number of source
    /// shards in a sector and fits nicely into a single TCP packet
    pub const RECOMMENDED_LIMIT: usize = 128;
}

/// Closest peers
#[derive(Debug, Default, PartialEq, Eq, Clone, From, Into, Deref, DerefMut)]
pub struct ClosestPeers(Vec<(PeerId, Vec<Multiaddr>)>);

impl Encode for ClosestPeers {
    fn size_hint(&self) -> usize {
        let mut size = Compact::compact_len(&(self.0.len() as u32));

        for (peer_id, addresses) in &self.0 {
            size += peer_id.as_ref().encoded_size();
            size += Compact::compact_len(&(addresses.len() as u32));

            for address in addresses {
                size += address.as_ref().encoded_size();
            }
        }

        size
    }

    fn encode_to<T>(&self, dest: &mut T)
    where
        T: Output + ?Sized,
    {
        Compact::from(self.0.len() as u32).encode_to(dest);

        for (peer_id, addresses) in &self.0 {
            peer_id.as_ref().encode_to(dest);
            Compact::from(addresses.len() as u32).encode_to(dest);

            for address in addresses {
                address.as_ref().encode_to(dest);
            }
        }
    }
}

impl EncodeLike for ClosestPeers {}

impl Decode for ClosestPeers {
    fn decode<I>(input: &mut I) -> Result<Self, parity_scale_codec::Error>
    where
        I: Input,
    {
        let mut closest_peers = Vec::with_capacity(K_VALUE.get());

        let closest_peers_count = Compact::<u32>::decode(input)?.0 as usize;
        for _ in 0..closest_peers_count {
            let peer_id =
                PeerId::from_multihash(Multihash::decode(input)?).map_err(|multihash| {
                    parity_scale_codec::Error::from("Can't create `PeerId` from `Multihash`")
                        .chain(format!("Code: {}", multihash.code()))
                })?;
            let p2p = Multiaddr::from(Protocol::P2p(peer_id));
            let mut addresses = Vec::new();

            let addresses_count = Compact::<u32>::decode(input)?.0 as usize;

            if addresses_count == 0 {
                return Err(parity_scale_codec::Error::from(
                    "List of addresses must not be empty",
                ));
            }

            for _ in 0..addresses_count {
                let address = Multiaddr::try_from(Vec::<u8>::decode(input)?).map_err(|error| {
                    parity_scale_codec::Error::from("Failed to decode `Multiaddr`")
                        .chain(error.to_string())
                })?;

                if !address.ends_with(&p2p) {
                    return Err(parity_scale_codec::Error::from(
                        "`Multiaddr` doesn't end with correct p2p suffix",
                    )
                    .chain(format!("Address {address}, PeerId {p2p}")));
                }

                addresses.push(address);
            }

            closest_peers.push((peer_id, addresses));
        }

        Ok(Self(closest_peers))
    }
}

/// Piece result contains either piece itself or the closest known peers to the piece index
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub enum PieceResult {
    /// Piece was cached locally
    Piece(Piece),
    /// Piece was not cached locally, but these are the closest known peers to the piece index
    ClosestPeers(ClosestPeers),
}

/// Cached-piece-by-index response, may be cached piece or stored in one of the farms
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub struct CachedPieceByIndexResponse {
    /// Piece result
    pub result: PieceResult,
    /// Additional pieces that requester is interested in and are cached locally, order from
    /// request is not preserved
    pub cached_pieces: Vec<PieceIndex>,
}

/// Cached-piece-by-index request handler
pub type CachedPieceByIndexRequestHandler = GenericRequestHandler<CachedPieceByIndexRequest>;
