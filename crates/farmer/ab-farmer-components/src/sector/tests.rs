use crate::sector::{SectorMetadata, SectorMetadataChecksummed, sector_padding_size, sector_size};
use ab_core_primitives::pieces::Record;
use ab_core_primitives::sectors::SectorIndex;
use ab_core_primitives::segments::{HistorySize, SegmentIndex};
use ab_direct_io_file::AlignedPage;
use parity_scale_codec::Encode;

#[test]
fn sector_size_is_aligned() {
    for pieces_in_sector in [1, 2, 1000, u16::MAX] {
        assert_eq!(sector_size(pieces_in_sector) % AlignedPage::SIZE, 0);
        assert!(sector_padding_size(pieces_in_sector) < AlignedPage::SIZE);
    }
}

#[test]
fn sector_metadata_disk_encoding() {
    assert_eq!(
        SectorMetadataChecksummed::disk_size() % AlignedPage::SIZE,
        0
    );

    let sector_metadata = SectorMetadataChecksummed::from(SectorMetadata {
        sector_index: SectorIndex::from(3),
        pieces_in_sector: 4,
        s_bucket_sizes: Box::new([5; Record::NUM_S_BUCKETS]),
        history_size: HistorySize::from(SegmentIndex::from(6)),
    });

    let encoded = sector_metadata.encode();
    assert_eq!(encoded.len(), SectorMetadataChecksummed::encoded_size());

    let bytes = sector_metadata.encode_for_disk();
    let (encoded_bytes, padding) = bytes.split_at(encoded.len());
    assert_eq!(encoded_bytes, encoded);
    assert!(!padding.is_empty());
    assert!(padding.iter().all(|&byte| byte == 0));

    let decoded = SectorMetadataChecksummed::decode_from_disk(&bytes).unwrap();
    assert_eq!(decoded.sector_index, sector_metadata.sector_index);
    assert_eq!(decoded.pieces_in_sector, sector_metadata.pieces_in_sector);
    assert_eq!(decoded.s_bucket_sizes, sector_metadata.s_bucket_sizes);
    assert_eq!(decoded.history_size, sector_metadata.history_size);
}
