//! Chia proof of space implementation

#[cfg(feature = "alloc")]
use crate::PosProofs;
#[cfg(feature = "alloc")]
use crate::TableGenerator;
use crate::chiapos::Tables;
#[cfg(feature = "alloc")]
use crate::chiapos::TablesCache;
use crate::{PosTableType, Table};
use ab_core_primitives::pos::{PosProof, PosSeed};
use ab_core_primitives::sectors::SBucket;
#[cfg(feature = "alloc")]
use alloc::boxed::Box;

const K: u8 = PosProof::K;

/// Proof of space table generator.
///
/// Chia implementation.
#[derive(Debug, Default, Clone)]
#[cfg(feature = "alloc")]
pub struct ChiaTableGenerator {
    tables_cache: TablesCache,
}

#[cfg(feature = "alloc")]
impl TableGenerator<ChiaTable> for ChiaTableGenerator {
    fn create_proofs(&self, seed: &PosSeed) -> Box<PosProofs> {
        Tables::<K>::create_proofs((*seed).into(), &self.tables_cache).into()
    }

    #[cfg(feature = "parallel")]
    fn create_proofs_parallel(&self, seed: &PosSeed) -> Box<PosProofs> {
        Tables::<K>::create_proofs_parallel((*seed).into(), &self.tables_cache).into()
    }
}

/// Proof of space table.
///
/// Chia implementation.
#[derive(Debug)]
pub struct ChiaTable;

impl ab_core_primitives::solutions::SolutionPotVerifier for ChiaTable {
    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool {
        Tables::<K>::verify_only_raw(seed, u32::from(s_bucket), proof)
    }
}

impl Table for ChiaTable {
    const TABLE_TYPE: PosTableType = PosTableType::Chia;
    #[cfg(feature = "alloc")]
    type Generator = ChiaTableGenerator;

    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool {
        <Self as ab_core_primitives::solutions::SolutionPotVerifier>::is_proof_valid(
            seed, s_bucket, proof,
        )
    }
}

#[cfg(all(feature = "alloc", test, not(miri)))]
mod tests {
    use super::*;
    use ab_core_primitives::sectors::SBucket;

    #[test]
    fn basic() {
        let seed = PosSeed::from([
            35, 2, 52, 4, 51, 55, 23, 84, 91, 10, 111, 12, 13, 222, 151, 16, 228, 211, 254, 45, 92,
            198, 204, 10, 9, 10, 11, 129, 139, 171, 15, 23,
        ]);

        let generator = ChiaTableGenerator::default();
        let proofs = generator.create_proofs(&seed);
        #[cfg(feature = "parallel")]
        let proofs_parallel = generator.create_proofs_parallel(&seed);

        let s_bucket_without_proof = SBucket::from(15651);
        assert!(proofs.for_s_bucket(s_bucket_without_proof).is_none());
        #[cfg(feature = "parallel")]
        assert!(
            proofs_parallel
                .for_s_bucket(s_bucket_without_proof)
                .is_none()
        );

        {
            let s_bucket_with_proof = SBucket::from(31500);
            let proof = proofs.for_s_bucket(s_bucket_with_proof).unwrap();
            #[cfg(feature = "parallel")]
            assert_eq!(
                proof,
                proofs_parallel.for_s_bucket(s_bucket_with_proof).unwrap()
            );
            assert!(ChiaTable::is_proof_valid(
                &seed,
                s_bucket_with_proof,
                &proof
            ));
        }
    }
}
