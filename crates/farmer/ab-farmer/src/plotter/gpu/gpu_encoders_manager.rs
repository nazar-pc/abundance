//! Thread pool managing utilities for plotting purposes

use ab_proof_of_space_gpu::GpuRecordsEncoder;
use event_listener::Event;
use parking_lot::Mutex;
use std::num::{NonZeroUsize, TryFromIntError};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// Wrapper around [`GpuRecordsEncoder`] that on `Drop` will return the records encoder into
/// corresponding [`GpuRecordsEncoderManager`].
#[derive(Debug)]
#[must_use]
pub(super) struct GpuRecordsEncoderGuard {
    inner: Arc<(Mutex<Vec<GpuRecordsEncoder>>, Event)>,
    gpu_records_encoder: Option<GpuRecordsEncoder>,
}

impl Deref for GpuRecordsEncoderGuard {
    type Target = GpuRecordsEncoder;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.gpu_records_encoder
            .as_ref()
            .expect("Value exists until `Drop`; qed")
    }
}

impl DerefMut for GpuRecordsEncoderGuard {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.gpu_records_encoder
            .as_mut()
            .expect("Value exists until `Drop`; qed")
    }
}

impl Drop for GpuRecordsEncoderGuard {
    #[inline]
    fn drop(&mut self) {
        let (mutex, event) = &*self.inner;
        mutex.lock().push(
            self.gpu_records_encoder
                .take()
                .expect("Happens only once in `Drop`; qed"),
        );
        event.notify_additional(1);
    }
}

/// GPU records encoder manager.
///
/// This abstraction wraps a set of GPU records encoders and allows to use them one at a time.
#[derive(Debug)]
pub(super) struct GpuRecordsEncoderManager {
    inner: Arc<(Mutex<Vec<GpuRecordsEncoder>>, Event)>,
    gpu_records_encoders: NonZeroUsize,
}

impl Clone for GpuRecordsEncoderManager {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            gpu_records_encoders: self.gpu_records_encoders,
        }
    }
}

impl GpuRecordsEncoderManager {
    /// Create a new instance.
    ///
    /// Returns an error if empty list of encoders is provided.
    pub(super) fn new(
        gpu_records_encoders: Vec<GpuRecordsEncoder>,
    ) -> Result<Self, TryFromIntError> {
        let count = gpu_records_encoders.len().try_into()?;

        Ok(Self {
            inner: Arc::new((Mutex::new(gpu_records_encoders), Event::new())),
            gpu_records_encoders: count,
        })
    }

    /// How many gpu records encoders are being managed here
    pub(super) fn gpu_records_encoders(&self) -> NonZeroUsize {
        self.gpu_records_encoders
    }

    /// Get one of inner thread pool pairs, will wait until one is available if needed
    pub(super) async fn get_encoder(&self) -> GpuRecordsEncoderGuard {
        let (mutex, event) = &*self.inner;

        let gpu_records_encoder = loop {
            let listener = event.listen();

            if let Some(gpu_records_encoder) = mutex.lock().pop() {
                break gpu_records_encoder;
            }

            listener.await;
        };

        GpuRecordsEncoderGuard {
            inner: Arc::clone(&self.inner),
            gpu_records_encoder: Some(gpu_records_encoder),
        }
    }
}
