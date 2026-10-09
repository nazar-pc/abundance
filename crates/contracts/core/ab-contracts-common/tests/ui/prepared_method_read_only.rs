//! Fields of `PreparedMethod` can't be modified after it is created by one of its constructors

use ab_contracts_common::env::{MethodContext, PreparedMethod};
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use core::ffi::c_void;
use core::ptr::NonNull;

fn modify(
    prepared_method: &mut PreparedMethod<'_>,
    contract: Address,
    fingerprint: MethodFingerprint,
    external_args: NonNull<c_void>,
    method_context: MethodContext,
) {
    prepared_method.contract = contract;
    prepared_method.fingerprint = fingerprint;
    prepared_method.external_args = external_args;
    prepared_method.method_context = method_context;
}

fn main() {}
