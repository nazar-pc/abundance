use crate::farm::{FarmError, PlottedSectors};
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::PieceOffset;
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use async_lock::RwLock as AsyncRwLock;
use async_trait::async_trait;
use futures::{Stream, stream};
use std::sync::Arc;
use subspace_farmer_components::FarmerProtocolInfo;
use subspace_farmer_components::plotting::PlottedSector;
use subspace_farmer_components::sector::SectorMetadataChecksummed;

/// Getter for single disk plotted sectors
#[derive(Debug)]
pub struct SingleDiskPlottedSectors {
    pub(super) public_key_hash: Blake3Hash,
    pub(super) pieces_in_sector: u16,
    pub(super) farmer_protocol_info: FarmerProtocolInfo,
    pub(super) sectors_metadata: Arc<AsyncRwLock<Vec<SectorMetadataChecksummed>>>,
}

#[async_trait]
impl PlottedSectors for SingleDiskPlottedSectors {
    async fn get(
        &self,
    ) -> Result<
        Box<dyn Stream<Item = Result<PlottedSector, FarmError>> + Unpin + Send + '_>,
        FarmError,
    > {
        let sectors_metadata = self.sectors_metadata.read().await.clone();
        Ok(Box::new(stream::iter(
            (SectorIndex::ZERO..).zip(sectors_metadata).map(
                move |(sector_index, sector_metadata)| {
                    let sector_id = SectorId::new(
                        &self.public_key_hash,
                        sector_index,
                        sector_metadata.history_size,
                    );

                    let mut piece_indexes = Vec::with_capacity(usize::from(self.pieces_in_sector));
                    (PieceOffset::ZERO..)
                        .take(usize::from(self.pieces_in_sector))
                        .map(|piece_offset| {
                            sector_id.derive_piece_index(
                                piece_offset,
                                sector_metadata.history_size,
                                self.farmer_protocol_info.max_pieces_in_sector,
                                self.farmer_protocol_info.recent_segments,
                                self.farmer_protocol_info.recent_history_fraction,
                            )
                        })
                        .collect_into(&mut piece_indexes);

                    Ok(PlottedSector {
                        sector_id,
                        sector_index,
                        sector_metadata,
                        piece_indexes,
                    })
                },
            ),
        )))
    }
}
