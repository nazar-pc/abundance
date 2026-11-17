use crate::single_disk_farm::PlotMetadataHeader;
use ab_direct_io_file::OwnedPages;
use parity_scale_codec::{Decode, Encode};

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
