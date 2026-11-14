use crate::protocols::request_response::handlers::cached_piece_by_index::{
    CachedPieceByIndexRequest, CachedPieceByIndexResponse, ClosestPeers, PieceResult,
};
use crate::protocols::request_response::handlers::generic_request_handler::GenericRequest;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use parity_scale_codec::{Decode, Encode};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[test]
fn closest_peers_encoding() {
    // Basic encoding/decoding
    {
        let closest_peers = ClosestPeers::from(vec![
            {
                let peer_id = PeerId::random();
                let addresses = vec![
                    Multiaddr::from(IpAddr::V4(Ipv4Addr::LOCALHOST))
                        .with(Protocol::Tcp(1234))
                        .with_p2p(peer_id)
                        .unwrap(),
                ];

                (peer_id, addresses)
            },
            {
                let peer_id = PeerId::random();
                let addresses = vec![
                    Multiaddr::from(IpAddr::V4(Ipv4Addr::LOCALHOST))
                        .with(Protocol::Tcp(1234))
                        .with_p2p(peer_id)
                        .unwrap(),
                    Multiaddr::from(IpAddr::V6(Ipv6Addr::LOCALHOST))
                        .with(Protocol::Udp(1234))
                        .with_p2p(peer_id)
                        .unwrap(),
                ];

                (peer_id, addresses)
            },
        ]);

        let decoded_closest_peers =
            ClosestPeers::decode(&mut closest_peers.encode().as_ref()).unwrap();
        assert_eq!(closest_peers, decoded_closest_peers);
    }

    // Addresses must end with `/p2p`
    {
        let closest_peers = ClosestPeers::from(vec![{
            let peer_id = PeerId::random();
            let addresses =
                vec![Multiaddr::from(IpAddr::V4(Ipv4Addr::LOCALHOST)).with(Protocol::Tcp(1234))];

            (peer_id, addresses)
        }]);

        ClosestPeers::decode(&mut closest_peers.encode().as_ref()).unwrap_err();
    }

    // Addresses must end with correct `/p2p`
    {
        let closest_peers = ClosestPeers::from(vec![{
            let peer_id = PeerId::random();
            let addresses = vec![
                Multiaddr::from(IpAddr::V4(Ipv4Addr::LOCALHOST))
                    .with(Protocol::Tcp(1234))
                    .with_p2p(PeerId::random())
                    .unwrap(),
            ];

            (peer_id, addresses)
        }]);

        ClosestPeers::decode(&mut closest_peers.encode().as_ref()).unwrap_err();
    }

    // Addresses list must not be empty
    {
        let closest_peers = ClosestPeers::from(vec![(PeerId::random(), vec![])]);

        ClosestPeers::decode(&mut closest_peers.encode().as_ref()).unwrap_err();
    }
}

#[test]
fn cached_piece_by_index_response_encoding() {
    let mut piece = Piece::default();
    piece.as_mut()[0] = 1;
    piece.as_mut()[Piece::SIZE - 1] = 2;
    let peer_id = PeerId::random();
    let closest_peers = ClosestPeers::from(vec![(
        peer_id,
        vec![
            Multiaddr::from(IpAddr::V4(Ipv4Addr::LOCALHOST))
                .with(Protocol::Tcp(1234))
                .with_p2p(peer_id)
                .unwrap(),
        ],
    )]);

    for response in [
        CachedPieceByIndexResponse {
            result: PieceResult::Piece(piece.clone()),
            cached_pieces: vec![PieceIndex::from(3), PieceIndex::from(4)],
        },
        CachedPieceByIndexResponse {
            result: PieceResult::ClosestPeers(closest_peers),
            cached_pieces: vec![PieceIndex::from(5)],
        },
    ] {
        let payload = CachedPieceByIndexRequest::encode_response(&response).unwrap();
        // Piece is not copied into the payload
        if let PieceResult::Piece(piece) = &response.result {
            assert!(
                payload
                    .buffers()
                    .iter()
                    .any(|buffer| buffer.as_byte_ptr() == piece.buffer().as_byte_ptr())
            );
        }

        // The same encoding as SCALE codec
        let buffer = payload.into_buffer();
        assert_eq!(buffer.as_slice(), response.encode());

        let decoded = CachedPieceByIndexRequest::decode_response(buffer.clone()).unwrap();
        assert_eq!(decoded, response);
        // Piece references the payload
        if let PieceResult::Piece(piece) = &decoded.result {
            let range = buffer.as_slice().as_ptr_range();
            assert!(range.contains(&piece.as_ref().as_ptr()));
        }
    }
}

#[test]
fn cached_piece_by_index_response_decoding_errors() {
    let response = CachedPieceByIndexResponse {
        result: PieceResult::Piece(Piece::default()),
        cached_pieces: vec![PieceIndex::from(3)],
    };
    let encoded = response.encode();

    // Invalid variant of `PieceResult`
    let mut invalid = encoded.clone();
    invalid[0] = 2;
    CachedPieceByIndexRequest::decode_response(SharedAlignedBuffer::from_bytes(&invalid))
        .unwrap_err();

    // Truncated within the piece and after it
    for length in [0, 1, 5, Piece::SIZE, encoded.len() - 1] {
        CachedPieceByIndexRequest::decode_response(SharedAlignedBuffer::from_bytes(
            &encoded[..length],
        ))
        .unwrap_err();
    }
}
