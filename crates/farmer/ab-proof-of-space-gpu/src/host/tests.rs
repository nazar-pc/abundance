use crate::Device;
use ab_core_primitives::hashes::Blake3Hash;
use ab_core_primitives::pieces::Record;
use ab_core_primitives::sectors::{SectorId, SectorIndex};
use ab_core_primitives::segments::HistorySize;
use ab_core_primitives::solutions::ShardCommitmentHash;
use ab_erasure_coding::ErasureCoding;
use ab_farmer_components::plotting::{CpuRecordsEncoder, RecordsEncoder};
use ab_proof_of_space::Table;
use ab_proof_of_space::chia::ChiaTable;
use chacha20::ChaCha8Rng;
use futures::executor::block_on;
use rand::prelude::*;
use rclite::Arc;
use std::num::NonZeroU8;
use std::slice;
use std::sync::Arc as StdArc;
use std::sync::atomic::AtomicBool;

#[test]
fn basic() {
    let mut rng = ChaCha8Rng::from_seed(Default::default());

    let erasure_coding = ErasureCoding::new();
    let global_mutex = Arc::default();
    let abort_early = AtomicBool::new(false);

    let chia_table_generator = ChiaTable::generator();
    let mut cpu_records_encoder = CpuRecordsEncoder::<ChiaTable>::new(
        slice::from_ref(&chia_table_generator),
        &erasure_coding,
        &global_mutex,
    );

    let public_key_hash = Blake3Hash::from([1; _]);
    let shard_commitments_root = &ShardCommitmentHash::default();
    let sector_index = SectorIndex::new(1);
    let history_size = HistorySize::ONE;
    let sector_id = SectorId::new(
        &public_key_hash,
        shard_commitments_root,
        sector_index,
        history_size,
    );

    let source_records = {
        let mut records = Record::new_zero_vec(1);
        for record in &mut records {
            rng.fill_bytes(record.as_flattened_mut());
        }
        records
    };

    let mut expected_encoded_records = source_records.clone();
    let expected_sector_contents_map = cpu_records_encoder
        .encode_records(&sector_id, &mut expected_encoded_records, &abort_early)
        .unwrap();

    let devices = block_on(Device::enumerate(|_| NonZeroU8::MIN));
    for device in devices {
        let mut device_instance = device
            .instantiate(erasure_coding.clone(), StdArc::default())
            .unwrap();

        let mut actual_encoded_records = source_records.clone();

        let actual_sector_contents_map = device_instance
            .encode_records(&sector_id, &mut actual_encoded_records, &abort_early)
            .unwrap();

        for (
            piece_offset,
            (
                (expected_found_proofs, actual_found_proofs),
                (expected_encoded_record, actual_encoded_record),
            ),
        ) in expected_sector_contents_map
            .iter_record_chunks_used()
            .iter()
            .zip(actual_sector_contents_map.iter_record_chunks_used())
            .zip(expected_encoded_records.iter().zip(&actual_encoded_records))
            .enumerate()
        {
            for (byte_index, (expected, actual)) in expected_found_proofs
                .iter()
                .zip(actual_found_proofs)
                .enumerate()
            {
                assert_eq!(
                    expected, actual,
                    "piece_offset = {piece_offset}, byte_index={byte_index}, \
                    expected={expected:#b}, actual={actual:#b}, device: {device:?}",
                );
            }

            for (chunk_index, (expected, actual)) in expected_encoded_record
                .iter()
                .zip(actual_encoded_record.iter())
                .enumerate()
            {
                assert_eq!(
                    expected, actual,
                    "piece_offset = {piece_offset}, chunk_index={chunk_index}, device: {device:?}",
                );
            }
        }
    }
}
