//! Chia proof of space implementation

#[cfg(all(feature = "alloc", test, not(miri)))]
mod tests;

#[cfg(feature = "alloc")]
use crate::PosProofs;
#[cfg(feature = "alloc")]
use crate::TableGenerator;
use crate::chiapos::Tables;
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
pub struct ChiaTableGenerator;

#[cfg(feature = "alloc")]
impl TableGenerator<ChiaTable> for ChiaTableGenerator {
    fn create_proofs(&self, seed: &PosSeed) -> Box<PosProofs> {
        Tables::<K>::create_proofs((*seed).into()).into()
    }

    #[cfg(feature = "parallel")]
    fn create_proofs_parallel(&self, seed: &PosSeed) -> Box<PosProofs> {
        Tables::<K>::create_proofs_parallel((*seed).into()).into()
    }
}

/// Proof of space table.
///
/// Chia implementation.
#[derive(Debug)]
pub struct ChiaTable;

impl ab_core_primitives::solutions::SolutionPotVerifier for ChiaTable {
    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool {
        Tables::<K>::verify_only_raw(seed, u32::from(s_bucket), proof)
    }
}

impl Table for ChiaTable {
    const TABLE_TYPE: PosTableType = PosTableType::Chia;
    #[cfg(feature = "alloc")]
    type Generator = ChiaTableGenerator;

    #[cfg_attr(feature = "no-panic", no_panic::no_panic)]
    fn is_proof_valid(seed: &PosSeed, s_bucket: SBucket, proof: &PosProof) -> bool {
        <Self as ab_core_primitives::solutions::SolutionPotVerifier>::is_proof_valid(
            seed, s_bucket, proof,
        )
    }
}
