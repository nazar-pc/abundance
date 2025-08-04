use ab_core_primitives::block::BlockNumber;
use ab_core_primitives::segments::{SegmentHeader, SegmentIndex};
use parity_scale_codec::Encode;
use parking_lot::RwLock;
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use tracing::debug;

/// Error for [`SegmentHeadersStore`]
#[derive(Debug, thiserror::Error)]
pub enum SegmentHeaderStoreError {
    /// Segment index must strictly follow last segment index, can't store segment header
    #[error(
        "Segment index {segment_index} must strictly follow last segment index \
        {last_segment_index}, can't store segment header"
    )]
    MustFollowLastSegmentIndex {
        /// Segment index that was attempted to be inserted
        segment_index: SegmentIndex,
        /// Last segment index
        last_segment_index: SegmentIndex,
    },
    /// First segment index must be zero
    #[error("First segment index must be zero, found {segment_index}")]
    FirstSegmentIndexZero {
        /// Segment index that was attempted to be inserted
        segment_index: SegmentIndex,
    },
}

#[derive(Debug)]
struct SegmentHeadersStoreInner {
    next_key_index: AtomicU16,
    /// In-memory cache of segment headers
    cache: RwLock<Vec<SegmentHeader>>,
}

// TODO: Disk persistence
/// Persistent storage of segment headers.
///
/// It maintains all known segment headers. During sync from DSN it is possible that this data
/// structure contains segment headers that from the point of view of the tip of the current chain
/// are "in the future". This is expected and must be accounted for in the archiver and other
/// places.
///
/// Segment headers are stored in batches (which is more efficient to store and retrieve). Each next
/// batch contains distinct segment headers with monotonically increasing segment indices. During
/// instantiation all previously stored batches will be read and in-memory representation of the
/// whole contents will be created such that queries to this data structure are quick and not
/// involving any disk I/O.
#[derive(Debug)]
pub struct SegmentHeadersStore {
    inner: Arc<SegmentHeadersStoreInner>,
    confirmation_depth_k: BlockNumber,
}

impl Clone for SegmentHeadersStore {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            confirmation_depth_k: self.confirmation_depth_k,
        }
    }
}

impl SegmentHeadersStore {
    const KEY_PREFIX: &'static [u8] = b"segment-headers";
    const INITIAL_CACHE_CAPACITY: usize = 1_000;

    /// Create a new instance
    pub fn new(confirmation_depth_k: BlockNumber) -> Result<Self, SegmentHeaderStoreError> {
        let cache = Vec::with_capacity(Self::INITIAL_CACHE_CAPACITY);

        debug!("Started loading segment headers into cache");
        // Segment headers are stored in batches (which is more efficient to store and retrieve), this is why code deals
        // with key indices here rather that segment indices. Essentially this iterates over keys from 0 until missing
        // entry is hit, which becomes the next key index where additional segment headers will be stored.
        let next_key_index = 0;
        // while let Some(segment_headers) =
        //     aux_store
        //         .get_aux(&Self::key(next_key_index))?
        //         .map(|segment_header| {
        //             Vec::<SegmentHeader>::decode(&mut segment_header.as_slice())
        //                 .expect("Always correct segment header unless DB is corrupted; qed")
        //         })
        // {
        //     cache.extend(segment_headers);
        //     next_key_index += 1;
        // }
        debug!("Finished loading segment headers into cache");

        Ok(Self {
            inner: Arc::new(SegmentHeadersStoreInner {
                // aux_store,
                next_key_index: AtomicU16::new(next_key_index),
                cache: RwLock::new(cache),
            }),
            confirmation_depth_k,
        })
    }

    /// Returns last observed segment header
    pub fn last_segment_header(&self) -> Option<SegmentHeader> {
        self.inner.cache.read().last().cloned()
    }

    /// Returns last observed segment index
    pub fn max_segment_index(&self) -> Option<SegmentIndex> {
        let segment_index = self.inner.cache.read().len().checked_sub(1)? as u64;
        Some(SegmentIndex::from(segment_index))
    }

    /// Add segment headers.
    ///
    /// Multiple can be inserted for efficiency purposes.
    pub fn add_segment_headers(
        &self,
        segment_headers: &[SegmentHeader],
    ) -> Result<(), SegmentHeaderStoreError> {
        let mut maybe_last_segment_index = self.max_segment_index();
        let mut segment_headers_to_store = Vec::with_capacity(segment_headers.len());
        // Check all input segment headers to see which ones are not stored yet and verifying that segment indices are
        // monotonically increasing
        for segment_header in segment_headers {
            let segment_index = segment_header.segment_index();
            match maybe_last_segment_index {
                Some(last_segment_index) => {
                    if segment_index <= last_segment_index {
                        // Skip already stored segment headers
                        continue;
                    }

                    if segment_index != last_segment_index + SegmentIndex::ONE {
                        return Err(SegmentHeaderStoreError::MustFollowLastSegmentIndex {
                            segment_index,
                            last_segment_index,
                        });
                    }

                    segment_headers_to_store.push(segment_header);
                    maybe_last_segment_index.replace(segment_index);
                }
                None => {
                    if segment_index != SegmentIndex::ZERO {
                        return Err(SegmentHeaderStoreError::FirstSegmentIndexZero {
                            segment_index,
                        });
                    }

                    segment_headers_to_store.push(segment_header);
                    maybe_last_segment_index.replace(segment_index);
                }
            }
        }

        if segment_headers_to_store.is_empty() {
            return Ok(());
        }

        // Insert all new segment headers into vacant key index for efficiency purposes
        // TODO: Do compaction when we have too many keys: combine multiple segment headers into a
        //  single entry for faster retrievals and more compact storage
        {
            let key_index = self.inner.next_key_index.fetch_add(1, Ordering::SeqCst);
            let key = Self::key(key_index);
            let value = segment_headers_to_store.encode();
            let insert_data = vec![(key.as_slice(), value.as_slice())];

            black_box(insert_data);
            // self.inner.aux_store.insert_aux(&insert_data, &[])?;
        }
        self.inner.cache.write().extend(segment_headers_to_store);

        Ok(())
    }

    /// Get a single segment header
    pub fn get_segment_header(&self, segment_index: SegmentIndex) -> Option<SegmentHeader> {
        self.inner
            .cache
            .read()
            .get(u64::from(segment_index) as usize)
            .copied()
    }

    fn key(key_index: u16) -> Vec<u8> {
        (Self::KEY_PREFIX, key_index.to_le_bytes()).encode()
    }

    /// Get segment headers that are expected to be included at specified block number.
    pub fn segment_headers_for_block(&self, block_number: BlockNumber) -> Vec<SegmentHeader> {
        let Some(last_segment_index) = self.max_segment_index() else {
            // Not initialized
            return Vec::new();
        };

        // Special case for the initial segment (for genesis block).
        if block_number == BlockNumber::ONE {
            // If there is a segment index present, and we store monotonically increasing segment
            // headers, then the first header exists.
            return vec![
                self.get_segment_header(SegmentIndex::ZERO)
                    .expect("Segment headers are stored in monotonically increasing order; qed"),
            ];
        }

        if last_segment_index == SegmentIndex::ZERO {
            // Genesis segment already included in block #1
            return Vec::new();
        }

        let mut current_segment_index = last_segment_index;
        loop {
            // If the current segment index present, and we store monotonically increasing segment
            // headers, then the current segment header exists as well.
            let current_segment_header = self
                .get_segment_header(current_segment_index)
                .expect("Segment headers are stored in monotonically increasing order; qed");

            // The block immediately after the archived segment adding the confirmation depth
            let target_block_number = current_segment_header.last_archived_block.number()
                + BlockNumber::ONE
                + self.confirmation_depth_k;
            if target_block_number == block_number {
                let mut headers_for_block = vec![current_segment_header];

                // Check block spanning multiple segments
                let last_archived_block_number = current_segment_header.last_archived_block.number;
                let mut segment_index = current_segment_index - SegmentIndex::ONE;

                while let Some(segment_header) = self.get_segment_header(segment_index) {
                    if segment_header.last_archived_block.number == last_archived_block_number {
                        headers_for_block.insert(0, segment_header);
                        segment_index -= SegmentIndex::ONE;
                    } else {
                        break;
                    }
                }

                return headers_for_block;
            }

            // iterate segments further
            if target_block_number > block_number {
                // no need to check the initial segment
                if current_segment_index > SegmentIndex::ONE {
                    current_segment_index -= SegmentIndex::ONE
                } else {
                    break;
                }
            } else {
                // No segment headers required
                return Vec::new();
            }
        }

        // No segment headers required
        Vec::new()
    }
}
