//! Handlers for different request-response protocols

pub mod cached_piece_by_index;
pub mod generic_request_handler;
pub mod piece_by_index;
pub mod segment_header;

use crate::protocols::request_response::handlers::generic_request_handler::encode_to_buffer;
use crate::protocols::request_response::request_response_factory::ResponsePayload;
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align1;
use ab_core_primitives::pieces::Piece;
use parity_scale_codec::Encode;

/// Append `prefix` and `piece` to the payload, encoded the same way as SCALE codec encodes them,
/// without copying the contents of the piece
fn encode_piece<P>(payload: &mut ResponsePayload, prefix: &P, piece: &Piece) -> Option<()>
where
    P: Encode + ?Sized,
{
    payload.push(encode_to_buffer(prefix)?);
    payload.push(piece.buffer().clone());

    Some(())
}

/// Decode a piece from `input`, which is the end of `payload`, without copying its contents, see
/// [`encode_piece()`]
fn decode_piece(
    payload: &SharedAlignedBuffer<Align1>,
    input: &mut &[u8],
) -> Result<Piece, parity_scale_codec::Error> {
    let remainder = input
        .get(Piece::SIZE..)
        .ok_or("Not enough bytes for a piece")?;
    let start = payload.as_slice().len() - input.len();

    let piece = u32::try_from(start)
        .ok()
        .zip(u32::try_from(start + Piece::SIZE).ok())
        .and_then(|(start, end)| payload.get_slice(start..end))
        .expect("Piece is within the payload, whose length is `u32`; qed");
    *input = remainder;

    Ok(Piece::try_from(piece).expect("Correct length; qed"))
}
