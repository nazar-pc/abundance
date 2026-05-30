use crate::chia::{ChiaTable, ChiaTableGenerator};
use crate::{Table, TableGenerator};
use ab_core_primitives::pos::PosSeed;
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
