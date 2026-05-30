use crate::shim::ShimTable;
use crate::{Table, TableGenerator};
use ab_core_primitives::pos::PosSeed;
use ab_core_primitives::sectors::SBucket;

#[test]
fn basic() {
    let seed = PosSeed::from([
        35, 2, 52, 4, 51, 55, 23, 84, 91, 10, 111, 12, 13, 222, 151, 16, 228, 211, 254, 45, 92,
        198, 204, 10, 9, 10, 11, 129, 139, 171, 15, 23,
    ]);

    let proofs = ShimTable::generator().create_proofs(&seed);

    let s_bucket_without_proof = SBucket::from(1);
    assert!(proofs.for_s_bucket(s_bucket_without_proof).is_none());

    {
        let s_bucket_with_proof = SBucket::from(0);
        let proof = proofs.for_s_bucket(s_bucket_with_proof).unwrap();
        assert!(ShimTable::is_proof_valid(
            &seed,
            s_bucket_with_proof,
            &proof
        ));
    }
}
