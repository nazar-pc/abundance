use crate::rocm::rocm_devices;
use ab_core_primitives::pieces::{PieceOffset, Record};
use ab_core_primitives::sectors::SectorId;
use ab_core_primitives::segments::HistorySize;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::plotting::{CpuRecordsEncoder, RecordsEncoder};
use ab_farmer_components::sector::SectorContentsMap;
use ab_proof_of_space::Table;
use ab_proof_of_space::chia::ChiaTable;
use std::num::NonZeroUsize;
use std::slice;

type PosTable = ChiaTable;

#[test]
fn basic() {
    let rocm_device = rocm_devices()
        .into_iter()
        .next()
        .expect("Need ROCm device to run this test");

    let table_generator = PosTable::generator();
    let erasure_coding = ErasureCoding::new();
    let global_mutex = Default::default();
    let mut cpu_records_encoder = CpuRecordsEncoder::<PosTable>::new(
        slice::from_ref(&table_generator),
        &erasure_coding,
        &global_mutex,
    );

    let history_size = HistorySize::ONE;
    let sector_id = SectorId::new(blake3::hash(b"hello").into(), 500, history_size);
    let mut record = Record::new_boxed();
    record.iter_mut().enumerate().for_each(|(index, chunk)| {
        chunk.copy_from_slice(blake3::hash(&index.to_le_bytes()).as_bytes())
    });

    let mut cpu_encoded_records = Record::new_zero_vec(2);
    for cpu_encoded_record in &mut cpu_encoded_records {
        cpu_encoded_record.clone_from(&record);
    }
    let cpu_sector_contents_map = cpu_records_encoder
        .encode_records(&sector_id, &mut cpu_encoded_records, &Default::default())
        .unwrap();

    let mut gpu_encoded_records = Record::new_zero_vec(2);
    for gpu_encoded_record in &mut gpu_encoded_records {
        gpu_encoded_record.clone_from(&record);
    }
    let mut gpu_sector_contents_map = SectorContentsMap::new(2);
    rocm_device
        .generate_and_encode_pospace(
            &sector_id.derive_evaluation_seed(PieceOffset::ZERO),
            &mut gpu_encoded_records[0],
            gpu_sector_contents_map
                .iter_record_bitfields_mut()
                .next()
                .unwrap()
                .iter_mut(),
        )
        .unwrap();
    rocm_device
        .generate_and_encode_pospace(
            &sector_id.derive_evaluation_seed(PieceOffset::ONE),
            &mut gpu_encoded_records[1],
            gpu_sector_contents_map
                .iter_record_bitfields_mut()
                .nth(1)
                .unwrap()
                .iter_mut(),
        )
        .unwrap();

    assert!(cpu_sector_contents_map == gpu_sector_contents_map);
    assert!(cpu_encoded_records == gpu_encoded_records);
}
