//! `PreparedMethod::new_untyped()` can only be called in `unsafe` code, since it accepts any memory
//! as arguments and the executor follows pointers in it

use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;

fn prepare(
    contract: Address,
    fingerprint: MethodFingerprint,
    external_args: &mut [u8; 16],
) -> PreparedMethod<'_> {
    PreparedMethod::new_untyped(contract, fingerprint, external_args, MethodContext::Keep)
}

fn main() {}
