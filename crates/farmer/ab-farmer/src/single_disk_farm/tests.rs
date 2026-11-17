use crate::single_disk_farm::{
    PlotMetadataHeader, RESERVED_PLOT_METADATA, SECTOR_METADATA_READ_BATCH, read_sectors_metadata,
};
use ab_core_primitives::pieces::Record;
use ab_core_primitives::sectors::SectorIndex;
use ab_core_primitives::segments::{HistorySize, SegmentIndex};
use ab_direct_io_file::{DirectIoFile, OwnedPages};
use ab_farmer_components::sector::{SectorMetadata, SectorMetadataChecksummed};
use parity_scale_codec::{Decode, Encode};
use std::fs::OpenOptions;
use std::io;
use tempfile::tempdir;

#[test]
fn metadata_header_page() {
    let metadata_header = PlotMetadataHeader {
        version: 1,
        plotted_sector_count: 2,
    };

    let page = metadata_header.encode_page();
    assert_eq!(page.len(), OwnedPages::ALIGN);
    let (header_bytes, padding) = page.split_at(metadata_header.encoded_size());
    let decoded = PlotMetadataHeader::decode(&mut &*header_bytes).unwrap();
    assert_eq!(decoded.version, metadata_header.version);
    assert_eq!(
        decoded.plotted_sector_count,
        metadata_header.plotted_sector_count
    );
    assert!(padding.iter().all(|&byte| byte == 0));
}

#[test]
// TODO: Extremely slow under Miri: https://github.com/rust-lang/miri/issues/4463
#[cfg_attr(miri, ignore)]
fn read_sectors_metadata_in_batches() {
    let tempdir = tempdir().unwrap();
    let mut open_options = OpenOptions::new();
    open_options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false);
    let file = DirectIoFile::open(open_options, tempdir.path().join("metadata.bin")).unwrap();

    // Two whole batches and a partial one
    let sector_count = SECTOR_METADATA_READ_BATCH * 2 + 1;
    let sector_metadata_size = SectorMetadataChecksummed::disk_size() as u64;
    file.set_len(RESERVED_PLOT_METADATA + u64::from(sector_count) * sector_metadata_size)
        .unwrap();
    let sectors_metadata = (0..sector_count)
        .map(|sector_index| {
            SectorMetadataChecksummed::from(SectorMetadata {
                sector_index: SectorIndex::from(sector_index),
                pieces_in_sector: 1,
                s_bucket_sizes: Box::new([sector_index; Record::NUM_S_BUCKETS]),
                history_size: HistorySize::from(SegmentIndex::ZERO),
            })
        })
        .collect::<Vec<_>>();
    for (offset, sector_metadata) in (0..).zip(&sectors_metadata) {
        file.write_at(
            sector_metadata.encode_for_disk(),
            RESERVED_PLOT_METADATA + offset * sector_metadata_size,
        )
        .unwrap();
    }

    let mut read_sectors = Vec::new();
    read_sectors_metadata::<io::Error, _>(&file, sector_count, |sector_index, bytes| {
        read_sectors.push((
            sector_index,
            SectorMetadataChecksummed::decode_from_disk(bytes).unwrap(),
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(read_sectors.len(), sectors_metadata.len());
    for ((sector_index, read_sector), sector_metadata) in read_sectors.iter().zip(&sectors_metadata)
    {
        assert_eq!(*sector_index, sector_metadata.sector_index);
        assert_eq!(read_sector.encode(), sector_metadata.encode());
    }

    // Nothing to read
    read_sectors_metadata::<io::Error, _>(&file, 0, |_sector_index, _bytes| {
        panic!("No sectors to read");
    })
    .unwrap();
}
