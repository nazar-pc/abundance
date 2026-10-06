use crate::hart::VectorLengths;
use crate::instructions::v::{Elen, Vlen};

#[test]
fn vector_lengths_require_elen_within_vlen() {
    assert!(VectorLengths::new(Elen::L32, Vlen::L32).is_some());
    assert!(VectorLengths::new(Elen::L64, Vlen::L128).is_some());
    assert!(VectorLengths::new(Elen::L64, Vlen::L32).is_none());
}
