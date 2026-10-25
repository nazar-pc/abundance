use crate::shim::ShimTable;
use crate::{Table, TableGenerator};
use ab_core_primitives::pieces::Record;
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

#[test]
fn one_s_bucket_of_each_pair_has_proof() {
    let generator = ShimTable::generator();

    for seed in [PosSeed::from([1; _]), PosSeed::from([2; _])] {
        let proofs = generator.create_proofs(&seed);

        for first_s_bucket in (0..Record::NUM_S_BUCKETS).step_by(2) {
            let pair = [first_s_bucket, first_s_bucket + 1].map(|s_bucket| {
                let s_bucket = SBucket::try_from(s_bucket).unwrap();
                let proof = proofs.for_s_bucket(s_bucket)?;
                assert!(ShimTable::is_proof_valid(&seed, s_bucket, &proof));
                Some(proof)
            });

            assert_ne!(pair[0].is_some(), pair[1].is_some(), "{first_s_bucket}");
        }
    }
}

#[test]
fn s_buckets_with_proofs_depend_on_seed() {
    let generator = ShimTable::generator();
    let proofs = generator.create_proofs(&PosSeed::from([1; _]));
    let other_proofs = generator.create_proofs(&PosSeed::from([2; _]));

    assert_ne!(proofs.found_proofs, other_proofs.found_proofs);
}
