#![cfg(not(miri))]

use crate::chiapos::{Tables, TablesCache};
use ab_core_primitives::sectors::SBucket;
use alloc::vec::Vec;

const K: u8 = 17;

#[test]
fn self_verification() {
    let seed = [1; 32];
    let cache = TablesCache::default();
    let tables = Tables::<K>::create(seed, &cache);
    #[cfg(feature = "parallel")]
    let tables_parallel = Tables::<K>::create_parallel(seed, &cache);

    let all_proofs = Tables::<K>::create_proofs(seed, &cache);
    #[cfg(feature = "parallel")]
    let all_proofs_parallel = Tables::<K>::create_proofs_parallel(seed, &cache);

    for challenge_index in 0..1000_u32 {
        let mut challenge = [0; 32];
        challenge[..size_of::<u32>()].copy_from_slice(&challenge_index.to_le_bytes());
        let first_challenge_bytes = challenge[..4].try_into().unwrap();
        let qualities = tables.find_quality(&challenge).collect::<Vec<_>>();
        #[cfg(feature = "parallel")]
        assert_eq!(
            qualities,
            tables_parallel.find_quality(&challenge).collect::<Vec<_>>(),
            "challenge index {challenge_index}"
        );
        let proofs = tables.find_proof(first_challenge_bytes).collect::<Vec<_>>();
        #[cfg(feature = "parallel")]
        assert_eq!(
            proofs,
            tables_parallel
                .find_proof(first_challenge_bytes)
                .collect::<Vec<_>>(),
            "challenge index {challenge_index}"
        );

        assert_eq!(qualities.len(), proofs.len());

        for (quality, proof) in qualities.into_iter().zip(&proofs) {
            assert_eq!(
                Some(quality),
                Tables::<K>::verify(&seed, &challenge, proof),
                "challenge index {challenge_index}"
            );
            let mut bad_challenge = [0; 32];
            bad_challenge[..size_of::<u32>()].copy_from_slice(&(challenge_index + 1).to_le_bytes());
            assert!(
                Tables::<K>::verify(&seed, &bad_challenge, proof).is_none(),
                "challenge index {challenge_index}"
            );
        }

        {
            let raw_proofs = tables.find_proof_raw(challenge_index).collect::<Vec<_>>();
            let s_bucket = SBucket::from(challenge_index as u16);
            assert_eq!(
                raw_proofs.first().copied(),
                all_proofs.for_s_bucket(s_bucket),
                "challenge index {challenge_index}"
            );
            #[cfg(feature = "parallel")]
            assert_eq!(
                raw_proofs.first().copied(),
                all_proofs_parallel.for_s_bucket(s_bucket),
                "challenge index {challenge_index}"
            );
        }
    }
}
