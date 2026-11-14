use crate::protocols::request_response::handlers::generic_request_handler::GenericRequest;
use ab_aligned_buffer::SharedAlignedBuffer;
use parity_scale_codec::{Decode, Encode};

#[derive(Encode, Decode)]
struct ExampleRequest;

impl GenericRequest for ExampleRequest {
    const PROTOCOL_NAME: &'static str = "/example";
    const LOG_TARGET: &'static str = "example_request";
    type Response = u32;
}

#[test]
fn default_response_encoding() {
    let payload = ExampleRequest::encode_response(&1).unwrap();
    let buffer = payload.into_buffer();
    assert_eq!(buffer.as_slice(), 1u32.encode());
    assert_eq!(ExampleRequest::decode_response(buffer).unwrap(), 1);

    // Not enough bytes
    ExampleRequest::decode_response(SharedAlignedBuffer::from_bytes(&[1, 0, 0])).unwrap_err();
    // Trailing bytes after the response
    ExampleRequest::decode_response(SharedAlignedBuffer::from_bytes(&[1, 0, 0, 0, 0])).unwrap_err();
}
