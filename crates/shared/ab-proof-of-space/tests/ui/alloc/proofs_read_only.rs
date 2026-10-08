//! `PosProofs` and `chiapos::Proofs` can only be created and modified by the crate itself.
//! `for_s_bucket()` relies on at most `Record::NUM_CHUNKS` bits being set in `found_proofs` and
//! passes `proof_index < Record::NUM_CHUNKS` to `hint::assert_unchecked()`, so writing these fields
//! from safe code could cause undefined behavior.

use ab_proof_of_space::PosProofs;
use ab_proof_of_space::chiapos::Proofs;

fn create_pos_proofs(other: &PosProofs) -> PosProofs {
    PosProofs {
        found_proofs: other.found_proofs,
        proofs: other.proofs,
    }
}

fn modify_pos_proofs(proofs: &mut PosProofs, other: &PosProofs) {
    proofs.found_proofs = other.found_proofs;
}

fn create_chiapos_proofs(other: &Proofs<20>) -> Proofs<20> {
    Proofs {
        found_proofs: other.found_proofs,
        proofs: other.proofs,
    }
}

fn modify_chiapos_proofs(proofs: &mut Proofs<20>, other: &Proofs<20>) {
    proofs.found_proofs = other.found_proofs;
}

fn main() {}
