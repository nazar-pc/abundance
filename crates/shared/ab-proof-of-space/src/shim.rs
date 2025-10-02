//! Shim proof of space implementation that works much faster than Chia and can be used for testing
//! purposes to reduce memory and CPU usage

#[cfg(feature = "alloc")]
use crate::PosProofs;
#[cfg(feature = "alloc")]
use crate::TableGenerator;
use crate::{PosTableType, Table};
#[cfg(feature = "alloc")]
use ab_core_primitives::pieces::Record;
use ab_core_primitives::pos::{PosProof, PosSeed};
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
    fn generate(&self, seed: &PosSeed) -> ShimTable {
        ShimTable { seed: *seed }
    }

    fn create_proofs(&self, seed: &PosSeed) -> Box<PosProofs> {
        // SAFETY: Zeroed contents is a safe invariant
        let mut proofs = unsafe { Box::<PosProofs>::new_zeroed().assume_init() };

        let mut num_found_proofs = 0_usize;
        'outer: for (challenge_indices, found_proofs) in (0..Record::NUM_S_BUCKETS as u32)
            .array_chunks::<{ u8::BITS as usize }>()
            .zip(&mut proofs.found_proofs)
        {
            for (proof_offset, challenge_index) in challenge_indices.into_iter().enumerate() {
                if let Some(proof) = find_proof(seed, challenge_index) {
                    *found_proofs |= 1 << proof_offset;

                    proofs.proofs[num_found_proofs] = proof;
                    num_found_proofs += 1;

                    if num_found_proofs == Record::NUM_CHUNKS {
                        break 'outer;
                    }
                }
            }
        }

        proofs
    }
}

/// Proof of space table.
///
/// Shim implementation.
#[derive(Debug)]
pub struct ShimTable {
    #[cfg(feature = "alloc")]
    seed: PosSeed,
}

impl ab_core_primitives::solutions::SolutionPotVerifier for ShimTable {
    fn is_proof_valid(seed: &PosSeed, challenge_index: u32, proof: &PosProof) -> bool {
        let Some(correct_proof) = find_proof(seed, challenge_index) else {
            return false;
        };

        &correct_proof == proof
    }
}

impl Table for ShimTable {
    const TABLE_TYPE: PosTableType = PosTableType::Shim;
    #[cfg(feature = "alloc")]
    type Generator = ShimTableGenerator;

    #[cfg(feature = "alloc")]
    fn find_proof(&self, challenge_index: u32) -> Option<PosProof> {
        find_proof(&self.seed, challenge_index)
    }

    fn is_proof_valid(seed: &PosSeed, challenge_index: u32, proof: &PosProof) -> bool {
        <Self as ab_core_primitives::solutions::SolutionPotVerifier>::is_proof_valid(
            seed,
            challenge_index,
            proof,
        )
    }
}

fn find_proof(seed: &PosSeed, challenge_index: u32) -> Option<PosProof> {
    let quality = ab_blake3::single_block_hash(&challenge_index.to_le_bytes())
        .expect("Less than a single block worth of bytes; qed");
    if !quality[0].is_multiple_of(3) {
        let mut proof = PosProof::default();
        proof
            .iter_mut()
            .zip(seed.iter().chain(iter::repeat(quality.iter()).flatten()))
            .for_each(|(output, input)| {
                *output = *input;
            });

        Some(proof)
    } else {
        None
    }
}

#[cfg(all(feature = "alloc", test))]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let seed = PosSeed::from([
            35, 2, 52, 4, 51, 55, 23, 84, 91, 10, 111, 12, 13, 222, 151, 16, 228, 211, 254, 45, 92,
            198, 204, 10, 9, 10, 11, 129, 139, 171, 15, 23,
        ]);

        let table = ShimTable::generator().generate(&seed);

        assert!(table.find_proof(1).is_none());

        {
            let challenge_index = 0;
            let proof = table.find_proof(challenge_index).unwrap();
            assert!(ShimTable::is_proof_valid(&seed, challenge_index, &proof));
        }
    }
}
