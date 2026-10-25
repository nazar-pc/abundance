//! Shim proof of space implementation that works much faster than Chia and can be used for testing
//! purposes to reduce memory and CPU usage

#[cfg(all(feature = "alloc", test, not(miri)))]
mod tests;

#[cfg(feature = "alloc")]
use crate::PosProofs;
#[cfg(feature = "alloc")]
use crate::TableGenerator;
use crate::{PosTableType, Table};
#[cfg(feature = "alloc")]
use ab_core_primitives::pieces::Record;
use ab_core_primitives::pos::{PosProof, PosSeed};
use ab_core_primitives::sectors::SBucket;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::iter;

/// Proof of space table generator.
///
/// Shim implementation.
#[derive(Debug, Default, Clone)]
#[cfg(feature = "alloc")]
pub struct ShimTableGenerator;

#[cfg(feature = "alloc")]
impl TableGenerator<ShimTable> for ShimTableGenerator {
    fn create_proofs(&self, seed: &PosSeed) -> Box<PosProofs> {
        // SAFETY: Data structure filled with zeroes is a valid invariant
        let mut proofs = unsafe { Box::<PosProofs>::new_zeroed().assume_init() };

        create_proofs_internal(seed, &mut proofs);

        proofs
    }
}

/// Find proofs for exactly one s-bucket of each pair of neighboring s-buckets.
///
/// That gives exactly [`Record::NUM_CHUNKS`] proofs, as many as fit into `proofs`, which must be
/// zero-initialized.
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
fn create_proofs_internal(seed: &PosSeed, proofs: &mut PosProofs) {
    // Each byte of `found_proofs` covers 8 s-buckets, which are 4 pairs
    const PAIRS_PER_BYTE: usize = u8::BITS as usize / 2;
    const {
        assert!(Record::NUM_S_BUCKETS == Record::NUM_CHUNKS * 2);
    }

    let PosProofs {
        found_proofs,
        proofs,
    } = proofs;
    let (proofs, _) = proofs.as_chunks_mut::<PAIRS_PER_BYTE>();

    for ((found_proofs, proofs), first_pair_index) in found_proofs
        .iter_mut()
        .zip(proofs)
        .zip((0..).step_by(PAIRS_PER_BYTE))
    {
        for ((output, pair_index), bit_offset) in proofs
            .iter_mut()
            .zip(first_pair_index..)
            .zip((0..).step_by(2))
        {
            let (second_has_proof, proof) = pair_proof(seed, pair_index);
            *found_proofs |= 1 << (bit_offset + u32::from(second_has_proof));
            *output = proof;
        }
    }
}

/// Proof of space table.
///
/// Shim implementation.
#[derive(Debug)]
pub struct ShimTable;

impl ab_core_primitives::solutions::SolutionPotVerifier for ShimTable {
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool {
        let Some(correct_proof) = find_proof(seed, u32::from(s_bucket)) else {
            return false;
        };

        &correct_proof == proof
    }
}

impl Table for ShimTable {
    const TABLE_TYPE: PosTableType = PosTableType::Shim;
    #[cfg(feature = "alloc")]
    type Generator = ShimTableGenerator;

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool {
        <Self as ab_core_primitives::solutions::SolutionPotVerifier>::is_proof_valid(
            seed, s_bucket, proof,
        )
    }
}

#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
fn find_proof(seed: &PosSeed, challenge_index: u32) -> Option<PosProof> {
    let (second_has_proof, proof) = pair_proof(seed, challenge_index / 2);
    let is_second = !challenge_index.is_multiple_of(2);

    (second_has_proof == is_second).then_some(proof)
}

/// Proof of the pair of neighboring s-buckets `2 * pair_index` and `2 * pair_index + 1`.
///
/// Only one of them has a proof, the second one if `true` is returned.
#[cfg_attr(feature = "no-panic", no_panic::no_panic)]
fn pair_proof(seed: &PosSeed, pair_index: u32) -> (bool, PosProof) {
    let mut bytes_to_hash = [0; PosSeed::SIZE + size_of::<u32>()];
    bytes_to_hash[..PosSeed::SIZE].copy_from_slice(seed.as_slice());
    bytes_to_hash[PosSeed::SIZE..].copy_from_slice(&pair_index.to_le_bytes());
    let quality = ab_blake3::single_block_hash(&bytes_to_hash)
        .expect("Less than a single block worth of bytes; qed");

    let mut proof = PosProof::default();
    proof
        .iter_mut()
        .zip(seed.iter().chain(iter::repeat(quality.iter()).flatten()))
        .for_each(|(output, input)| {
            *output = *input;
        });

    (quality[0] & 1 == 1, proof)
}
