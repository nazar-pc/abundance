use crate::sector::{sector_padding_size, sector_size};
use ab_direct_io_file::AlignedPage;

#[test]
fn sector_size_is_aligned() {
    for pieces_in_sector in [1, 2, 1000, u16::MAX] {
        assert_eq!(sector_size(pieces_in_sector) % AlignedPage::SIZE, 0);
        assert!(sector_padding_size(pieces_in_sector) < AlignedPage::SIZE);
    }
}
