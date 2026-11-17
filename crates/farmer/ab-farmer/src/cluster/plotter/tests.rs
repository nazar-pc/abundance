use crate::cluster::plotter::{ClusterSectorPlottingProgress, SectorChunk};
use ab_aligned_buffer::SharedAlignedBuffer;
use ab_aligned_buffer::alignment::Align4096;
use ab_direct_io_file::OwnedPages;
use parity_scale_codec::{Compact, Decode, Encode};

#[test]
fn sector_chunk_encoding() {
    let bytes = (0..OwnedPages::ALIGN * 2)
        .map(|index| index as u8)
        .collect::<Vec<_>>();
    let sector_chunk = SectorChunk(SharedAlignedBuffer::<Align4096>::from_bytes(&bytes));

    // The same encoding as bytes
    let encoded = sector_chunk.encode();
    assert_eq!(encoded, bytes.encode());
    assert_eq!(sector_chunk.size_hint(), encoded.len());

    // Decoded into a new buffer aligned to the page size
    let decoded = SectorChunk::decode(&mut encoded.as_slice()).unwrap();
    assert_eq!(decoded.0.as_slice(), bytes);
    assert!(
        decoded
            .0
            .as_byte_ptr()
            .addr()
            .is_multiple_of(OwnedPages::ALIGN as usize)
    );

    // The same inside of progress notifications
    let encoded = ClusterSectorPlottingProgress::SectorChunk(Ok(sector_chunk)).encode();
    let ClusterSectorPlottingProgress::SectorChunk(Ok(decoded)) =
        ClusterSectorPlottingProgress::decode(&mut encoded.as_slice()).unwrap()
    else {
        panic!("Expected a sector chunk");
    };
    assert_eq!(decoded.0.as_slice(), bytes);
}

#[test]
fn sector_chunk_decoding_errors() {
    // Not enough bytes
    let mut encoded = vec![1u8; OwnedPages::ALIGN as usize].encode();
    encoded.pop();
    SectorChunk::decode(&mut encoded.as_slice()).unwrap_err();

    // Length that is not a multiple of the page size
    let encoded = vec![1u8; OwnedPages::ALIGN as usize + 1].encode();
    SectorChunk::decode(&mut encoded.as_slice()).unwrap_err();

    // Length that is much larger than the input is rejected before allocating memory
    let encoded = Compact(u32::MAX / OwnedPages::ALIGN * OwnedPages::ALIGN).encode();
    SectorChunk::decode(&mut encoded.as_slice()).unwrap_err();
}
