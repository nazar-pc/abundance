//! Piece reader for single disk farm

use crate::farm::{FarmError, PieceReader};
use crate::single_disk_farm::direct_io_file_wrapper::DirectIoFileWrapper;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::{Piece, PieceOffset};
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use ab_core_primitives::solutions::ShardCommitmentHash;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::sector::{SectorMetadataChecksummed, sector_size};
use ab_farmer_components::shard_commitment::ShardCommitmentsRootsCache;
use ab_farmer_components::{ReadAt, ReadAtAsync, ReadAtSync, reading};
use ab_proof_of_space::Table;
use async_lock::{Mutex as AsyncMutex, RwLock as AsyncRwLock};
use async_trait::async_trait;
use futures::channel::{mpsc, oneshot};
use futures::{SinkExt, StreamExt};
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use tracing::{error, warn};

#[derive(Debug)]
struct ReadPieceRequest {
    sector_index: SectorIndex,
    piece_offset: PieceOffset,
    response_sender: oneshot::Sender<Option<Piece>>,
}

/// Wrapper data structure that can be used to read pieces from single disk farm
#[derive(Debug, Clone)]
pub struct DiskPieceReader {
    read_piece_sender: mpsc::Sender<ReadPieceRequest>,
}

#[async_trait]
impl PieceReader for DiskPieceReader {
    #[inline]
    async fn read_piece(
        &self,
        sector_index: SectorIndex,
        piece_offset: PieceOffset,
    ) -> Result<Option<Piece>, FarmError> {
        Ok(self.read_piece(sector_index, piece_offset).await)
    }
}

impl DiskPieceReader {
    /// Creates new piece reader instance and background future that handles reads internally.
    ///
    /// NOTE: Background future is async, but does blocking operations and should be running in
    /// dedicated thread.
    #[expect(clippy::too_many_arguments)]
    pub(super) fn new<PosTable>(
        public_key_hash: Blake3Hash,
        shard_commitments_roots_cache: ShardCommitmentsRootsCache,
        pieces_in_sector: u16,
        plot_file: Arc<DirectIoFileWrapper>,
        sectors_metadata: Arc<AsyncRwLock<Vec<SectorMetadataChecksummed>>>,
        erasure_coding: ErasureCoding,
        sectors_being_modified: Arc<AsyncRwLock<HashSet<SectorIndex>>>,
        global_mutex: Arc<AsyncMutex<()>>,
    ) -> (Self, impl Future<Output = ()>)
    where
        PosTable: Table,
    {
        let (read_piece_sender, read_piece_receiver) = mpsc::channel(10);

        let reading_fut = async move {
            read_pieces::<PosTable, _>(
                public_key_hash,
                shard_commitments_roots_cache,
                pieces_in_sector,
                &*plot_file,
                sectors_metadata,
                erasure_coding,
                sectors_being_modified,
                read_piece_receiver,
                global_mutex,
            )
            .await
        };

        (Self { read_piece_sender }, reading_fut)
    }

    pub(super) fn close_all_readers(&mut self) {
        self.read_piece_sender.close_channel();
    }

    /// Read piece from sector by offset, `None` means input parameters are incorrect or piece
    /// reader was shut down
    pub async fn read_piece(
        &self,
        sector_index: SectorIndex,
        piece_offset: PieceOffset,
    ) -> Option<Piece> {
        let (response_sender, response_receiver) = oneshot::channel();
        self.read_piece_sender
            .clone()
            .send(ReadPieceRequest {
                sector_index,
                piece_offset,
                response_sender,
            })
            .await
            .ok()?;
        response_receiver.await.ok()?
    }
}

#[expect(clippy::too_many_arguments)]
async fn read_pieces<PosTable, S>(
    public_key_hash: Blake3Hash,
    shard_commitments_roots_cache: ShardCommitmentsRootsCache,
    pieces_in_sector: u16,
    plot_file: S,
    sectors_metadata: Arc<AsyncRwLock<Vec<SectorMetadataChecksummed>>>,
    erasure_coding: ErasureCoding,
    sectors_being_modified: Arc<AsyncRwLock<HashSet<SectorIndex>>>,
    mut read_piece_receiver: mpsc::Receiver<ReadPieceRequest>,
    global_mutex: Arc<AsyncMutex<()>>,
) where
    PosTable: Table,
    S: ReadAtSync,
{
    // TODO: Reuse global table generator (this comment is in many files)
    let table_generator = PosTable::generator();

    while let Some(read_piece_request) = read_piece_receiver.next().await {
        let ReadPieceRequest {
            sector_index,
            piece_offset,
            response_sender,
        } = read_piece_request;

        if response_sender.is_canceled() {
            continue;
        }

        let sectors_being_modified = &*sectors_being_modified.read().await;

        if sectors_being_modified.contains(&sector_index) {
            // Skip sector that is being modified right now
            continue;
        }

        let (sector_metadata, sector_count) = {
            let sectors_metadata = sectors_metadata.read().await;

            let sector_count = sectors_metadata.len() as u16;

            let sector_metadata = match sectors_metadata.get(usize::from(sector_index)) {
                Some(sector_metadata) => sector_metadata.clone(),
                None => {
                    error!(
                        %sector_index,
                        %sector_count,
                        "Tried to read piece from sector that is not yet plotted"
                    );
                    continue;
                }
            };

            (sector_metadata, sector_count)
        };

        // Sector must be plotted
        if u16::from(sector_index) >= sector_count {
            warn!(
                %sector_index,
                %piece_offset,
                %sector_count,
                "Incorrect sector offset"
            );
            // Doesn't matter if receiver still cares about it
            let _ = response_sender.send(None);
            continue;
        }
        // Piece must be within sector
        if u16::from(piece_offset) >= pieces_in_sector {
            warn!(
                %sector_index,
                %piece_offset,
                %sector_count,
                "Incorrect piece offset"
            );
            // Doesn't matter if receiver still cares about it
            let _ = response_sender.send(None);
            continue;
        }

        let sector_size = sector_size(pieces_in_sector);
        let sector = plot_file.offset(u64::from(sector_index) * sector_size as u64);

        // Take mutex briefly to make sure piece reading is allowed right now
        global_mutex.lock().await;

        let maybe_piece = read_piece::<PosTable, _, _>(
            &public_key_hash,
            &shard_commitments_roots_cache.get(sector_metadata.history_size),
            piece_offset,
            &sector_metadata,
            // TODO: Async
            &ReadAt::from_sync(&sector),
            &erasure_coding,
            &table_generator,
        )
        .await;

        // Doesn't matter if receiver still cares about it
        let _ = response_sender.send(maybe_piece);
    }
}

async fn read_piece<PosTable, S, A>(
    public_key_hash: &Blake3Hash,
    shard_commitments_root: &ShardCommitmentHash,
    piece_offset: PieceOffset,
    sector_metadata: &SectorMetadataChecksummed,
    sector: &ReadAt<S, A>,
    erasure_coding: &ErasureCoding,
    table_generator: &PosTable::Generator,
) -> Option<Piece>
where
    PosTable: Table,
    S: ReadAtSync,
    A: ReadAtAsync,
{
    let sector_index = sector_metadata.sector_index;

    let sector_id = SectorId::new(
        public_key_hash,
        shard_commitments_root,
        sector_index,
        sector_metadata.history_size,
    );

    let piece = match reading::read_piece::<PosTable, _, _>(
        piece_offset,
        &sector_id,
        sector_metadata,
        sector,
        erasure_coding,
        table_generator,
    )
    .await
    {
        Ok(piece) => piece,
        Err(error) => {
            error!(
                %sector_index,
                %piece_offset,
                %error,
                "Failed to read piece from sector"
            );
            return None;
        }
    };

    Some(piece)
}
