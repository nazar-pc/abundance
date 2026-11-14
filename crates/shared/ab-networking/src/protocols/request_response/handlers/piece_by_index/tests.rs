use crate::protocols::request_response::handlers::generic_request_handler::GenericRequest;
use crate::protocols::request_response::handlers::piece_by_index::{
    PieceByIndexRequest, PieceByIndexResponse,
};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_core_primitives::pieces::{Piece, PieceIndex};
use parity_scale_codec::Encode;

#[test]
fn piece_by_index_response_encoding() {
    let mut piece = Piece::default();
    piece.as_mut()[0] = 1;
    piece.as_mut()[Piece::SIZE - 1] = 2;

    for response in [
        PieceByIndexResponse {
            piece: Some(piece.clone()),
            cached_pieces: vec![PieceIndex::from(3), PieceIndex::from(4)],
        },
        PieceByIndexResponse {
            piece: Some(piece.clone()),
            cached_pieces: Vec::new(),
        },
        PieceByIndexResponse {
            piece: None,
            cached_pieces: vec![PieceIndex::from(5)],
        },
    ] {
        let payload = PieceByIndexRequest::encode_response(&response).unwrap();
        // Piece is not copied into the payload
        if let Some(piece) = &response.piece {
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

        let decoded = PieceByIndexRequest::decode_response(buffer.clone()).unwrap();
        assert_eq!(decoded, response);
        // Piece references the payload
        if let Some(piece) = &decoded.piece {
            let range = buffer.as_slice().as_ptr_range();
            assert!(range.contains(&piece.as_ref().as_ptr()));
        }
    }
}

#[test]
fn piece_by_index_response_decoding_errors() {
    let response = PieceByIndexResponse {
        piece: Some(Piece::default()),
        cached_pieces: vec![PieceIndex::from(3)],
    };
    let encoded = response.encode();

    // Invalid variant of `Option`
    let mut invalid = encoded.clone();
    invalid[0] = 2;
    PieceByIndexRequest::decode_response(SharedAlignedBuffer::from_bytes(&invalid)).unwrap_err();

    // Truncated within the piece and after it
    for length in [0, 1, 5, Piece::SIZE, encoded.len() - 1] {
        PieceByIndexRequest::decode_response(SharedAlignedBuffer::from_bytes(&encoded[..length]))
            .unwrap_err();
    }
}
