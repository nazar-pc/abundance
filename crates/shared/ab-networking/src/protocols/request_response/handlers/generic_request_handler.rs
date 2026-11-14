//! Generic request-response handler, typically is used with a type implementing [`GenericRequest`]
//! to significantly reduce boilerplate when implementing [`RequestHandler`].

use crate::protocols::request_response::request_response_factory::{
    IncomingRequest, OutgoingResponse, ProtocolConfig, RequestHandler, ResponsePayload,
};
use ab_aligned_buffer::alignment::Align1;
use ab_aligned_buffer::{OwnedAlignedBuffer, SharedAlignedBuffer};
use async_trait::async_trait;
use futures::channel::mpsc;
use futures::prelude::*;
use libp2p::PeerId;
use parity_scale_codec::{Decode, Encode, Output};
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, trace};

/// Could be changed after the production feedback.
const REQUESTS_BUFFER_SIZE: usize = 50;

/// SCALE codec output that writes into an aligned buffer
struct BufferOutput {
    buffer: OwnedAlignedBuffer<Align1>,
    too_large: bool,
}

impl Output for BufferOutput {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        if !self.buffer.append(bytes) {
            self.too_large = true;
        }
    }
}

/// Encode a value with SCALE codec into a new buffer.
///
/// Returns `None` if the encoding is larger than [`OwnedAlignedBuffer::MAX_CAPACITY`].
pub fn encode_to_buffer<T>(value: &T) -> Option<SharedAlignedBuffer<Align1>>
where
    T: Encode + ?Sized,
{
    #[cfg_attr(
        target_pointer_width = "64",
        expect(
            clippy::unnecessary_min_or_max,
            reason = "`MAX_CAPACITY` is `u32::MAX` where `usize` is 64-bit"
        )
    )]
    let capacity = u32::try_from(value.size_hint())
        .unwrap_or(u32::MAX)
        .min(OwnedAlignedBuffer::<Align1>::MAX_CAPACITY);
    let mut output = BufferOutput {
        buffer: OwnedAlignedBuffer::with_capacity(capacity),
        too_large: false,
    };
    value.encode_to(&mut output);

    (!output.too_large).then(|| output.buffer.into_shared())
}

/// Generic request with associated response
pub trait GenericRequest: Encode + Decode + Send + Sync + 'static {
    /// Defines request-response protocol name.
    const PROTOCOL_NAME: &'static str;
    /// Specifies log-parameters for tracing.
    const LOG_TARGET: &'static str;
    /// Response type that corresponds to this request
    type Response: Encode + Decode + Send + Sync + 'static;

    /// Encode a response into a payload, which must be decodable with [`Self::decode_response()`].
    ///
    /// The default implementation encodes the response with SCALE codec, responses that contain
    /// large buffers can be encoded the same way without copying them into the payload instead.
    ///
    /// Returns `None` if the response is too large.
    #[inline]
    fn encode_response(response: &Self::Response) -> Option<ResponsePayload> {
        encode_to_buffer(response).map(ResponsePayload::from)
    }

    /// Decode a response received from a remote peer.
    ///
    /// The default implementation decodes the response with SCALE codec, responses that contain
    /// large buffers can reference them in the payload without copying them instead.
    #[inline]
    fn decode_response(
        payload: SharedAlignedBuffer<Align1>,
    ) -> Result<Self::Response, parity_scale_codec::Error> {
        Self::Response::decode(&mut payload.as_slice())
    }
}

type RequestHandlerFn<Request> = Arc<
    dyn (Fn(
            PeerId,
            Request,
        )
            -> Pin<Box<dyn Future<Output = Option<<Request as GenericRequest>::Response>> + Send>>)
        + Send
        + Sync
        + 'static,
>;

/// Defines generic request-response protocol handler.
pub struct GenericRequestHandler<Request>
where
    Request: GenericRequest,
{
    request_receiver: mpsc::Receiver<IncomingRequest>,
    request_handler: RequestHandlerFn<Request>,
    protocol_config: ProtocolConfig,
}

impl<Request> fmt::Debug for GenericRequestHandler<Request>
where
    Request: GenericRequest,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenericRequestHandler")
            .field("protocol_name", &Request::PROTOCOL_NAME)
            .finish_non_exhaustive()
    }
}

impl<Request> GenericRequestHandler<Request>
where
    Request: GenericRequest,
{
    /// Creates new [`GenericRequestHandler`] by given handler.
    pub fn create<RH, Fut>(request_handler: RH) -> Box<dyn RequestHandler>
    where
        RH: (Fn(PeerId, Request) -> Fut) + Send + Sync + 'static,
        Fut: Future<Output = Option<Request::Response>> + Send + 'static,
    {
        let (request_sender, request_receiver) = mpsc::channel(REQUESTS_BUFFER_SIZE);

        let mut protocol_config = ProtocolConfig::new(Request::PROTOCOL_NAME);
        protocol_config.inbound_queue = Some(request_sender);

        Box::new(Self {
            request_receiver,
            request_handler: Arc::new(move |peer_id, request| {
                Box::pin(request_handler(peer_id, request))
            }),
            protocol_config,
        })
    }

    /// Invokes external protocol handler.
    async fn handle_request(
        &self,
        peer: PeerId,
        payload: Vec<u8>,
    ) -> Result<ResponsePayload, RequestHandlerError> {
        trace!(%peer, protocol=Request::LOG_TARGET, "Handling request...");
        let request = Request::decode(&mut payload.as_slice())
            .map_err(|_error| RequestHandlerError::InvalidRequestFormat)?;
        let response = (self.request_handler)(peer, request)
            .await
            .ok_or(RequestHandlerError::NoResponse)?;

        Request::encode_response(&response).ok_or(RequestHandlerError::ResponseTooLarge)
    }
}

#[async_trait]
impl<Request> RequestHandler for GenericRequestHandler<Request>
where
    Request: GenericRequest,
{
    /// Run [`RequestHandler`].
    async fn run(&mut self) {
        while let Some(request) = self.request_receiver.next().await {
            let IncomingRequest {
                peer,
                payload,
                pending_response,
            } = request;

            match self.handle_request(peer, payload).await {
                Ok(response_data) => {
                    let response = OutgoingResponse {
                        result: Ok(response_data),
                        sent_feedback: None,
                    };

                    if pending_response.send(response).is_ok() {
                        trace!(target = Request::LOG_TARGET, %peer, "Handled request");
                    } else {
                        debug!(
                            target = Request::LOG_TARGET,
                            protocol = Request::PROTOCOL_NAME,
                            %peer,
                            "Failed to handle request: {}",
                            RequestHandlerError::SendResponse
                        );
                    }
                }
                Err(e) => {
                    debug!(
                        target = Request::LOG_TARGET,
                        protocol = Request::PROTOCOL_NAME,
                        %e,
                        "Failed to handle request.",
                    );

                    let response = OutgoingResponse {
                        result: Err(()),
                        sent_feedback: None,
                    };

                    if pending_response.send(response).is_err() {
                        debug!(
                            target = Request::LOG_TARGET,
                            protocol = Request::PROTOCOL_NAME,
                            %peer,
                            "Failed to handle request: {}", RequestHandlerError::SendResponse
                        );
                    }
                }
            }
        }
    }

    fn protocol_config(&self) -> ProtocolConfig {
        self.protocol_config.clone()
    }

    fn protocol_name(&self) -> &'static str {
        Request::PROTOCOL_NAME
    }

    fn clone_box(&self) -> Box<dyn RequestHandler> {
        let (request_sender, request_receiver) = mpsc::channel(REQUESTS_BUFFER_SIZE);

        let mut protocol_config = ProtocolConfig::new(Request::PROTOCOL_NAME);
        protocol_config.inbound_queue = Some(request_sender);

        Box::new(Self {
            request_receiver,
            request_handler: Arc::clone(&self.request_handler),
            protocol_config,
        })
    }
}

#[derive(Debug, thiserror::Error)]
enum RequestHandlerError {
    #[error("Failed to send response.")]
    SendResponse,

    #[error("Incorrect request format.")]
    InvalidRequestFormat,

    #[error("No response.")]
    NoResponse,

    #[error("Response is too large.")]
    ResponseTooLarge,
}
