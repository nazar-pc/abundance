use crate::protocols::request_response::handlers::generic_request_handler::{
    GenericRequest, GenericRequestHandler, RequestHandlerError,
};
use crate::protocols::request_response::request_response_factory::ProtocolConfig;
use ab_aligned_buffer::SharedAlignedBuffer;
use futures::channel::mpsc;
use futures::executor::block_on;
use libp2p::PeerId;
use parity_scale_codec::{Decode, Encode};
use std::pin::Pin;
use std::sync::Arc;

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

// Doesn't need an async runtime, which isn't supported under Miri on some platforms
#[test]
fn requests_must_be_consumed_fully() {
    let (_request_sender, request_receiver) = mpsc::channel(1);
    let handler = GenericRequestHandler::<ExampleRequest> {
        request_receiver,
        request_handler: Arc::new(
            |_peer_id, ExampleRequest| -> Pin<Box<dyn Future<Output = Option<u32>> + Send>> {
                Box::pin(async { Some(1) })
            },
        ),
        protocol_config: ProtocolConfig::new(ExampleRequest::PROTOCOL_NAME),
    };
    let peer_id = PeerId::random();

    let payload = block_on(handler.handle_request(peer_id, ExampleRequest.encode())).unwrap();
    assert_eq!(payload.into_buffer().as_slice(), 1u32.encode());

    // Trailing bytes after the request
    let mut request = ExampleRequest.encode();
    request.push(0);
    let result = block_on(handler.handle_request(peer_id, request));
    assert!(matches!(
        result,
        Err(RequestHandlerError::InvalidRequestFormat)
    ));
}
