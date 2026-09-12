#![expect(incomplete_features, reason = "generic_const_*")]
#![feature(
    gca_const_items,
    gca_macroless_args,
    gca_min_const_items,
    generic_const_items
)]
#![no_std]

use core::{array, fmt, slice};
use reed_solomon_simd::Error;
use reed_solomon_simd::engine::DefaultEngine;
use reed_solomon_simd::rate::{HighRateDecoder, HighRateEncoder, RateDecoder, RateEncoder};

/// Error that occurs when erasure coding data
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ErasureCodingError {
    /// Decoder error
    #[error("Decoder error: {0}")]
    DecoderError(#[from] Error),
}

/// Erasure coding processes shards in blocks of this many bytes independently from each other
const BLOCK_BYTES: usize = 64;
/// Size of source shards to process at once when shards are large.
///
/// Small enough to fit into L2 cache of CPUs since Skylake and Zen 1 (on most of them even with
/// both SMT threads of a core busy).
const TARGET_COLUMNS_BYTES: usize = 256 * 1024;
/// Narrower columns do not amortize fixed per-call overhead of erasure coding well enough
const MIN_COLUMN_BYTES: usize = 1024;
/// Max number of threads used by parallel methods.
///
/// Erasure coding of a set of shards is too little work to benefit from many threads, so only a few
/// are used to reduce latency without wasting CPU on orchestration.
#[cfg(feature = "parallel")]
const MAX_PARALLELISM: usize = 4;

/// Number of `u64` words needed for a bit per shard
const NUM_WORDS<const NUM_SHARDS: usize>: usize = NUM_SHARDS.div_ceil(u64::BITS as usize);

/// A bit per shard, typically saying whether that shard is present.
///
/// Bit `index % 64` of word `index / 64` corresponds to shard `index`, which is the same layout
/// `reed_solomon_simd::rate::ReceivedShards` uses.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ShardsBitmap<const NUM_SHARDS: usize> {
    words: [u64; NUM_WORDS::<NUM_SHARDS>],
}

impl<const NUM_SHARDS: usize> fmt::Debug for ShardsBitmap<NUM_SHARDS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShardsBitmap")
            .field("num_shards", &NUM_SHARDS)
            .field("num_set", &self.count())
            .finish()
    }
}

impl<const NUM_SHARDS: usize> Default for ShardsBitmap<NUM_SHARDS> {
    #[inline(always)]
    fn default() -> Self {
        Self::none()
    }
}

impl<const NUM_SHARDS: usize> ShardsBitmap<NUM_SHARDS> {
    /// Bitmap with no shards set
    #[inline(always)]
    pub const fn none() -> Self {
        Self {
            words: [0; NUM_WORDS::<NUM_SHARDS>],
        }
    }

    /// Bitmap with all shards set
    #[inline(always)]
    pub const fn all() -> Self {
        let mut this = Self {
            words: [u64::MAX; NUM_WORDS::<NUM_SHARDS>],
        };

        // Bits past the last shard must not be set
        let unused_bits = NUM_WORDS::<NUM_SHARDS> * u64::BITS as usize - NUM_SHARDS;
        if unused_bits > 0 {
            this.words[NUM_WORDS::<NUM_SHARDS> - 1] >>= unused_bits;
        }

        this
    }

    /// Whether the shard at `index` is set, `false` if `index` is out of bounds
    #[inline(always)]
    pub const fn get(&self, index: usize) -> bool {
        if index >= NUM_SHARDS {
            return false;
        }

        (self.words[index / u64::BITS as usize] >> (index % u64::BITS as usize)) & 1 == 1
    }

    /// Sets the shard at `index`, does nothing if `index` is out of bounds
    #[inline(always)]
    pub const fn set(&mut self, index: usize) {
        if index < NUM_SHARDS {
            self.words[index / u64::BITS as usize] |= 1 << (index % u64::BITS as usize);
        }
    }

    /// Unsets the shard at `index`, does nothing if `index` is out of bounds
    #[inline(always)]
    pub const fn unset(&mut self, index: usize) {
        if index < NUM_SHARDS {
            self.words[index / u64::BITS as usize] &= !(1 << (index % u64::BITS as usize));
        }
    }

    /// Number of shards that are set
    #[inline]
    pub const fn count(&self) -> usize {
        let mut count = 0;
        let mut word_index = 0;
        while word_index < NUM_WORDS::<NUM_SHARDS> {
            count += self.words[word_index].count_ones() as usize;
            word_index += 1;
        }

        count
    }
}

/// Which source and parity shards are present
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct ShardsPresent<const NUM_SHARDS: usize> {
    /// Which source shards are present
    pub source: ShardsBitmap<NUM_SHARDS>,
    /// Which parity shards are present
    pub parity: ShardsBitmap<NUM_SHARDS>,
}

impl<const NUM_SHARDS: usize> ShardsPresent<NUM_SHARDS> {
    /// Nothing is present
    #[inline(always)]
    pub const fn none() -> Self {
        Self {
            source: ShardsBitmap::none(),
            parity: ShardsBitmap::none(),
        }
    }

    /// Everything is present
    #[inline(always)]
    pub const fn all() -> Self {
        Self {
            source: ShardsBitmap::all(),
            parity: ShardsBitmap::all(),
        }
    }
}

/// Erasure coding abstraction.
///
/// Supports creation of parity shards and recovery of missing data.
///
/// Erasure coding processes each 64-byte block of shards independently from other blocks, so large
/// shards are split into narrower columns that are processed one by one, which keeps the working
/// set in CPU cache and is much faster than processing whole shards at once. With `parallel`
/// feature enabled, `par_*` methods additionally process columns concurrently.
#[derive(Debug, Clone)]
pub struct ErasureCoding;

impl Default for ErasureCoding {
    fn default() -> Self {
        Self::new()
    }
}

impl ErasureCoding {
    /// Create new erasure coding instance
    pub fn new() -> Self {
        Self {}
    }

    /// Extend contiguously stored source shards with parity shards
    #[inline]
    pub fn extend<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &[[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError> {
        Ok(extend::<NUM_SHARDS, SHARD_BYTES, false, _, _>(
            source, parity,
        )?)
    }

    /// Parallel version of [`Self::extend()`].
    ///
    /// Columns of large shards are processed using up to 4 threads, which is beneficial when
    /// erasure coding is the only thing running. Prefer [`Self::extend()`] when calling from
    /// code that is already parallel to avoid contention.
    ///
    /// Small shards (like many record chunks) are not split into columns and are processed by a
    /// single thread, the same way as [`Self::extend()`] does.
    #[cfg(feature = "parallel")]
    #[inline]
    pub fn par_extend<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &[[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError> {
        Ok(extend::<NUM_SHARDS, SHARD_BYTES, true, _, _>(
            source, parity,
        )?)
    }

    /// Extend source shards with parity shards, where shards are not stored contiguously
    #[inline]
    pub fn extend_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        parity: [ParityShard; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsRef<[u8; SHARD_BYTES]>,
        ParityShard: AsMut<[u8; SHARD_BYTES]>,
    {
        let mut parity = parity;

        Ok(extend::<NUM_SHARDS, SHARD_BYTES, false, _, _>(
            &source.each_ref().map(|shard| shard.as_ref().as_slice()),
            &mut parity.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
        )?)
    }

    /// Parallel version of [`Self::extend_scattered()`].
    ///
    /// Columns of large shards are processed using up to 4 threads, which is beneficial when
    /// erasure coding is the only thing running. Prefer [`Self::extend_scattered()`] when calling
    /// from code that is already parallel to avoid contention.
    ///
    /// Small shards (like many record chunks) are not split into columns and are processed by a
    /// single thread, the same way as [`Self::extend_scattered()`] does.
    #[cfg(feature = "parallel")]
    #[inline]
    pub fn par_extend_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        parity: [ParityShard; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsRef<[u8; SHARD_BYTES]>,
        ParityShard: AsMut<[u8; SHARD_BYTES]>,
    {
        let mut parity = parity;

        Ok(extend::<NUM_SHARDS, SHARD_BYTES, true, _, _>(
            &source.each_ref().map(|shard| shard.as_ref().as_slice()),
            &mut parity.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
        )?)
    }

    /// Recover missing source shards in place, with everything stored contiguously.
    ///
    /// Parity shards are inputs only, missing ones are simply not used. Prefer this over
    /// [`Self::recover_all()`] when parity shards are not needed.
    #[inline]
    pub fn recover_source<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &[[u8; SHARD_BYTES]; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError> {
        Ok(recover_source::<NUM_SHARDS, SHARD_BYTES, false, _, _>(
            source,
            &present.source,
            parity
                .iter()
                .enumerate()
                .map(|(index, shard)| present.parity.get(index).then_some(shard.as_slice())),
        )?)
    }

    /// Parallel version of [`Self::recover_source()`].
    ///
    /// Columns of large shards are processed using up to 4 threads, which is beneficial when
    /// erasure coding is the only thing running. Prefer [`Self::recover_source()`] when calling
    /// from code that is already parallel to avoid contention.
    ///
    /// Small shards (like many record chunks) are not split into columns and are processed by a
    /// single thread, the same way as [`Self::recover_source()`] does.
    #[cfg(feature = "parallel")]
    #[inline]
    pub fn par_recover_source<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &[[u8; SHARD_BYTES]; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError> {
        Ok(recover_source::<NUM_SHARDS, SHARD_BYTES, true, _, _>(
            source,
            &present.source,
            parity
                .iter()
                .enumerate()
                .map(|(index, shard)| present.parity.get(index).then_some(shard.as_slice())),
        )?)
    }

    /// Recover missing source shards in place, where shards are not stored contiguously.
    ///
    /// Parity shards are inputs only, so missing ones simply have no memory.
    #[inline]
    pub fn recover_source_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        source_present: &ShardsBitmap<NUM_SHARDS>,
        parity: [Option<ParityShard>; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsMut<[u8; SHARD_BYTES]>,
        ParityShard: AsRef<[u8; SHARD_BYTES]>,
    {
        let mut source = source;

        Ok(recover_source::<NUM_SHARDS, SHARD_BYTES, false, _, _>(
            &mut source.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
            source_present,
            parity
                .iter()
                .map(|maybe_shard| maybe_shard.as_ref().map(|shard| shard.as_ref().as_slice())),
        )?)
    }

    /// Parallel version of [`Self::recover_source_scattered()`].
    ///
    /// Columns of large shards are processed using up to 4 threads, which is beneficial when
    /// erasure coding is the only thing running. Prefer [`Self::recover_source_scattered()`] when
    /// calling from code that is already parallel to avoid contention.
    ///
    /// Small shards (like many record chunks) are not split into columns and are processed by a
    /// single thread, the same way as [`Self::recover_source_scattered()`] does.
    #[cfg(feature = "parallel")]
    #[inline]
    pub fn par_recover_source_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        source_present: &ShardsBitmap<NUM_SHARDS>,
        parity: [Option<ParityShard>; NUM_SHARDS],
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsMut<[u8; SHARD_BYTES]>,
        ParityShard: AsRef<[u8; SHARD_BYTES]>,
    {
        let mut source = source;

        Ok(recover_source::<NUM_SHARDS, SHARD_BYTES, true, _, _>(
            &mut source.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
            source_present,
            parity
                .iter()
                .map(|maybe_shard| maybe_shard.as_ref().map(|shard| shard.as_ref().as_slice())),
        )?)
    }

    /// Recover missing source and parity shards in place, with everything stored contiguously.
    ///
    /// Use [`Self::recover_source()`] instead if parity shards are not needed.
    #[inline]
    pub fn recover_all<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError> {
        Ok(recover_all::<NUM_SHARDS, SHARD_BYTES, false, _, _>(
            source, parity, present,
        )?)
    }

    /// Parallel version of [`Self::recover_all()`].
    ///
    /// Columns of large shards are processed using up to 4 threads, which is beneficial when
    /// erasure coding is the only thing running. Prefer [`Self::recover_all()`] when calling from
    /// code that is already parallel to avoid contention.
    ///
    /// Small shards (like many record chunks) are not split into columns and are processed by a
    /// single thread, the same way as [`Self::recover_all()`] does.
    #[cfg(feature = "parallel")]
    #[inline]
    pub fn par_recover_all<const NUM_SHARDS: usize, const SHARD_BYTES: usize>(
        &self,
        source: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        parity: &mut [[u8; SHARD_BYTES]; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError> {
        Ok(recover_all::<NUM_SHARDS, SHARD_BYTES, true, _, _>(
            source, parity, present,
        )?)
    }

    /// Recover missing source and parity shards in place, where shards are not stored contiguously
    #[inline]
    pub fn recover_all_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        parity: [ParityShard; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsMut<[u8; SHARD_BYTES]>,
        ParityShard: AsMut<[u8; SHARD_BYTES]>,
    {
        let mut source = source;
        let mut parity = parity;

        Ok(recover_all::<NUM_SHARDS, SHARD_BYTES, false, _, _>(
            &mut source.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
            &mut parity.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
            present,
        )?)
    }

    /// Parallel version of [`Self::recover_all_scattered()`].
    ///
    /// Columns of large shards are processed using up to 4 threads, which is beneficial when
    /// erasure coding is the only thing running. Prefer [`Self::recover_all_scattered()`] when
    /// calling from code that is already parallel to avoid contention.
    ///
    /// Small shards (like many record chunks) are not split into columns and are processed by a
    /// single thread, the same way as [`Self::recover_all_scattered()`] does.
    #[cfg(feature = "parallel")]
    #[inline]
    pub fn par_recover_all_scattered<
        const NUM_SHARDS: usize,
        const SHARD_BYTES: usize,
        SourceShard,
        ParityShard,
    >(
        &self,
        source: [SourceShard; NUM_SHARDS],
        parity: [ParityShard; NUM_SHARDS],
        present: &ShardsPresent<NUM_SHARDS>,
    ) -> Result<(), ErasureCodingError>
    where
        SourceShard: AsMut<[u8; SHARD_BYTES]>,
        ParityShard: AsMut<[u8; SHARD_BYTES]>,
    {
        let mut source = source;
        let mut parity = parity;

        Ok(recover_all::<NUM_SHARDS, SHARD_BYTES, true, _, _>(
            &mut source.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
            &mut parity.each_mut().map(|shard| shard.as_mut().as_mut_slice()),
            present,
        )?)
    }
}

/// Number of bytes of each shard that are processed at once.
///
/// Equals to `shard_bytes` when shards are not split into columns. Column boundaries are aligned to
/// [`BLOCK_BYTES`]-byte blocks, which is what makes processing of columns independent. The last
/// column may be narrower, but it ends where the shard ends, so it has the same layout as the shard
/// itself.
const fn column_bytes(num_shards: usize, shard_bytes: usize) -> usize {
    let Some(column_bytes) = TARGET_COLUMNS_BYTES.checked_div(num_shards) else {
        return shard_bytes;
    };
    let column_bytes = column_bytes / BLOCK_BYTES * BLOCK_BYTES;

    if column_bytes < MIN_COLUMN_BYTES || column_bytes >= shard_bytes {
        shard_bytes
    } else {
        column_bytes
    }
}

fn extend<const NUM_SHARDS: usize, const SHARD_BYTES: usize, const PARALLEL: bool, Source, Parity>(
    source: &[Source; NUM_SHARDS],
    parity: &mut [Parity; NUM_SHARDS],
) -> Result<(), Error>
where
    Source: AsRef<[u8]>,
    Parity: AsMut<[u8]>,
{
    if const { column_bytes(NUM_SHARDS, SHARD_BYTES) == SHARD_BYTES } {
        return encode(SHARD_BYTES, source, parity);
    }

    for_each_column::<NUM_SHARDS, PARALLEL, _, _, _>(
        SHARD_BYTES,
        const { column_bytes(NUM_SHARDS, SHARD_BYTES) },
        source.each_ref().map(AsRef::as_ref),
        parity.each_mut().map(AsMut::as_mut),
        |shard_bytes, source, mut parity| encode(shard_bytes, &source, &mut parity),
    )
}

/// `parity` must yield exactly `NUM_SHARDS` items
fn recover_source<
    'a,
    const NUM_SHARDS: usize,
    const SHARD_BYTES: usize,
    const PARALLEL: bool,
    Source,
    Parity,
>(
    source: &mut [Source; NUM_SHARDS],
    source_present: &ShardsBitmap<NUM_SHARDS>,
    parity: Parity,
) -> Result<(), Error>
where
    Source: AsRef<[u8]> + AsMut<[u8]>,
    Parity: Iterator<Item = Option<&'a [u8]>>,
{
    if const { column_bytes(NUM_SHARDS, SHARD_BYTES) == SHARD_BYTES } {
        return decode_source(SHARD_BYTES, source, source_present, parity);
    }

    let mut parity = parity;
    let parity = array::from_fn::<_, NUM_SHARDS, _>(|_| parity.next().flatten());

    for_each_column::<NUM_SHARDS, PARALLEL, _, _, _>(
        SHARD_BYTES,
        const { column_bytes(NUM_SHARDS, SHARD_BYTES) },
        source.each_mut().map(AsMut::as_mut),
        parity,
        |shard_bytes, mut source, parity| {
            decode_source(shard_bytes, &mut source, source_present, parity)
        },
    )
}

fn recover_all<
    const NUM_SHARDS: usize,
    const SHARD_BYTES: usize,
    const PARALLEL: bool,
    Source,
    Parity,
>(
    source: &mut [Source; NUM_SHARDS],
    parity: &mut [Parity; NUM_SHARDS],
    present: &ShardsPresent<NUM_SHARDS>,
) -> Result<(), Error>
where
    Source: AsRef<[u8]> + AsMut<[u8]>,
    Parity: AsRef<[u8]> + AsMut<[u8]>,
{
    if const { column_bytes(NUM_SHARDS, SHARD_BYTES) == SHARD_BYTES } {
        return decode_all(SHARD_BYTES, source, parity, present);
    }

    for_each_column::<NUM_SHARDS, PARALLEL, _, _, _>(
        SHARD_BYTES,
        const { column_bytes(NUM_SHARDS, SHARD_BYTES) },
        source.each_mut().map(AsMut::as_mut),
        parity.each_mut().map(AsMut::as_mut),
        |shard_bytes, mut source, mut parity| {
            decode_all(shard_bytes, &mut source, &mut parity, present)
        },
    )
}

/// Shard that can be split into columns
trait IntoColumns: Sized {
    /// Iterator over columns, yields at least as many columns as there are in the shard
    type Columns: Iterator<Item = Self>;

    fn into_columns(self, column_bytes: usize) -> Self::Columns;
}

impl<'a> IntoColumns for &'a [u8] {
    type Columns = slice::Chunks<'a, u8>;

    #[inline(always)]
    fn into_columns(self, column_bytes: usize) -> Self::Columns {
        self.chunks(column_bytes)
    }
}

impl<'a> IntoColumns for &'a mut [u8] {
    type Columns = slice::ChunksMut<'a, u8>;

    #[inline(always)]
    fn into_columns(self, column_bytes: usize) -> Self::Columns {
        self.chunks_mut(column_bytes)
    }
}

/// Columns of an optional shard, a missing shard has all columns missing
struct OptionalColumns<'a>(Option<slice::Chunks<'a, u8>>);

impl<'a> Iterator for OptionalColumns<'a> {
    type Item = Option<&'a [u8]>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.0.as_mut().and_then(Iterator::next))
    }
}

impl<'a> IntoColumns for Option<&'a [u8]> {
    type Columns = OptionalColumns<'a>;

    #[inline(always)]
    fn into_columns(self, column_bytes: usize) -> Self::Columns {
        OptionalColumns(self.map(|shard| shard.chunks(column_bytes)))
    }
}

/// Split shards of `shard_bytes` bytes into columns, yielding the number of bytes in a column
/// alongside the same column of every shard.
///
/// `column_bytes` must not be zero.
// TODO: Once `ShardStorage` from https://github.com/AndersTrier/reed-solomon-simd/issues/64 lands,
//  it will be possible to apply erasure coding to scattered shards directly without copying them
//  into a separate work buffer. While natural, it is likely to be slower for columns: columns of
//  large shards are far apart and may map to the same L2 cache sets, so a small contiguous work
//  buffer reused across columns should still be preferred.
fn columns<const NUM_SHARDS: usize, Shard>(
    shard_bytes: usize,
    column_bytes: usize,
    shards: [Shard; NUM_SHARDS],
) -> impl Iterator<Item = (usize, [Shard; NUM_SHARDS])>
where
    Shard: IntoColumns,
{
    let mut shards = shards.map(|shard| shard.into_columns(column_bytes));

    (0..shard_bytes).step_by(column_bytes).map(move |offset| {
        let column = shards.each_mut().map(|shard| {
            shard
                .next()
                .expect("All shards have `shard_bytes` bytes and the same number of columns; qed")
        });

        (column_bytes.min(shard_bytes - offset), column)
    })
}

/// Process shards of `shard_bytes` bytes in columns of `column_bytes` bytes.
///
/// Columns are processed one by one, or with `PARALLEL` shards are split into up to
/// `MAX_PARALLELISM` regions of columns that are processed concurrently.
fn for_each_column<const NUM_SHARDS: usize, const PARALLEL: bool, Source, Parity, Process>(
    shard_bytes: usize,
    column_bytes: usize,
    source: [Source; NUM_SHARDS],
    parity: [Parity; NUM_SHARDS],
    process: Process,
) -> Result<(), Error>
where
    Source: IntoColumns + Send,
    Parity: IntoColumns + Send,
    Process: Fn(usize, [Source; NUM_SHARDS], [Parity; NUM_SHARDS]) -> Result<(), Error> + Sync,
{
    #[cfg(feature = "parallel")]
    if PARALLEL {
        let num_columns = shard_bytes.div_ceil(column_bytes);
        let region_bytes = num_columns.div_ceil(MAX_PARALLELISM) * column_bytes;
        let process = &process;
        let mut results = [const { Ok(()) }; MAX_PARALLELISM];

        rayon::scope(|scope| {
            let regions = columns(shard_bytes, region_bytes, source).zip(columns(
                shard_bytes,
                region_bytes,
                parity,
            ));

            for (((region_bytes, source), (_, parity)), result) in regions.zip(&mut results) {
                scope.spawn(move |_| {
                    *result = for_each_column_sequential(
                        region_bytes,
                        column_bytes,
                        source,
                        parity,
                        process,
                    );
                });
            }
        });

        return results.into_iter().collect();
    }

    for_each_column_sequential(shard_bytes, column_bytes, source, parity, process)
}

fn for_each_column_sequential<const NUM_SHARDS: usize, Source, Parity, Process>(
    shard_bytes: usize,
    column_bytes: usize,
    source: [Source; NUM_SHARDS],
    parity: [Parity; NUM_SHARDS],
    process: Process,
) -> Result<(), Error>
where
    Source: IntoColumns,
    Parity: IntoColumns,
    Process: Fn(usize, [Source; NUM_SHARDS], [Parity; NUM_SHARDS]) -> Result<(), Error>,
{
    let columns =
        columns(shard_bytes, column_bytes, source).zip(columns(shard_bytes, column_bytes, parity));

    for ((shard_bytes, source), (_, parity)) in columns {
        process(shard_bytes, source, parity)?;
    }

    Ok(())
}

/// Write parity shards for source shards
fn encode<const NUM_SHARDS: usize, Source, Parity>(
    shard_bytes: usize,
    source: &[Source; NUM_SHARDS],
    parity: &mut [Parity; NUM_SHARDS],
) -> Result<(), Error>
where
    Source: AsRef<[u8]>,
    Parity: AsMut<[u8]>,
{
    let mut encoder = new_encoder::<NUM_SHARDS>(shard_bytes)?;

    for shard in source {
        encoder.add_original_shard(shard)?;
    }

    let result = encoder.encode()?;

    for (input, output) in result.recovery_iter().zip(parity) {
        output.as_mut().copy_from_slice(input);
    }

    Ok(())
}

/// Recover missing source shards from present source and parity shards
fn decode_source<'a, const NUM_SHARDS: usize, Source, Parity>(
    shard_bytes: usize,
    source: &mut [Source; NUM_SHARDS],
    source_present: &ShardsBitmap<NUM_SHARDS>,
    parity: Parity,
) -> Result<(), Error>
where
    Source: AsRef<[u8]> + AsMut<[u8]>,
    Parity: IntoIterator<Item = Option<&'a [u8]>>,
{
    let mut decoder = new_decoder::<NUM_SHARDS>(shard_bytes)?;

    for (index, shard) in source.iter().enumerate() {
        if source_present.get(index) {
            decoder.add_original_shard(index, shard)?;
        }
    }
    for (index, maybe_shard) in parity.into_iter().enumerate() {
        if let Some(shard) = maybe_shard {
            decoder.add_recovery_shard(index, shard)?;
        }
    }

    let result = decoder.decode()?;

    for (index, shard) in source.iter_mut().enumerate() {
        if !source_present.get(index) {
            shard
                .as_mut()
                .copy_from_slice(restored_original(&result, index));
        }
    }

    Ok(())
}

/// Recover missing source and parity shards from present source and parity shards
fn decode_all<const NUM_SHARDS: usize, Source, Parity>(
    shard_bytes: usize,
    source: &mut [Source; NUM_SHARDS],
    parity: &mut [Parity; NUM_SHARDS],
    present: &ShardsPresent<NUM_SHARDS>,
) -> Result<(), Error>
where
    Source: AsRef<[u8]> + AsMut<[u8]>,
    Parity: AsRef<[u8]> + AsMut<[u8]>,
{
    decode_source(
        shard_bytes,
        source,
        &present.source,
        parity
            .iter()
            .enumerate()
            .map(|(index, shard)| present.parity.get(index).then_some(shard.as_ref())),
    )?;

    if present.parity.count() == NUM_SHARDS {
        return Ok(());
    }

    // Source shards are complete at this point, so missing parity shards are simply encoded again
    let mut encoder = new_encoder::<NUM_SHARDS>(shard_bytes)?;

    for shard in source.iter() {
        encoder.add_original_shard(shard)?;
    }

    let result = encoder.encode()?;

    for (index, shard) in parity.iter_mut().enumerate() {
        if !present.parity.get(index) {
            shard.as_mut().copy_from_slice(recovery(&result, index));
        }
    }

    Ok(())
}

// TODO: Encoder and decoder allocate a work buffer internally on every call, reuse it once in-place
//  encoding from https://github.com/AndersTrier/reed-solomon-simd/issues/64 is available
#[inline(always)]
fn new_encoder<const NUM_SHARDS: usize>(
    shard_bytes: usize,
) -> Result<HighRateEncoder<DefaultEngine>, Error> {
    HighRateEncoder::new(
        NUM_SHARDS,
        NUM_SHARDS,
        shard_bytes,
        DefaultEngine::new(),
        None,
    )
}

#[inline(always)]
fn new_decoder<const NUM_SHARDS: usize>(
    shard_bytes: usize,
) -> Result<HighRateDecoder<DefaultEngine>, Error> {
    HighRateDecoder::new(
        NUM_SHARDS,
        NUM_SHARDS,
        shard_bytes,
        DefaultEngine::new(),
        None,
    )
}

#[inline(always)]
fn restored_original<'a>(
    result: &'a reed_solomon_simd::DecoderResult<'_>,
    index: usize,
) -> &'a [u8] {
    result
        .restored_original(index)
        .expect("Always corresponds to a missing source shard; qed")
}

#[inline(always)]
fn recovery<'a>(result: &'a reed_solomon_simd::EncoderResult<'_>, index: usize) -> &'a [u8] {
    result
        .recovery(index)
        .expect("Always corresponds to a missing parity shard; qed")
}
