//! Helpers shared by tests of the payload modules

use crate::payload::TransactionMethodContext;
use crate::payload::caller::CallPrepared;
use ab_contracts_common::ContractError;
use ab_contracts_common::env::PreparedMethod;
use ab_contracts_common::method::MethodFingerprint;
use ab_core_primitives::address::Address;
use ab_io_type::trivial_type::TrivialType;

/// Offset of the number of `#[slot]` arguments of the first method in the payload
pub(crate) const NUM_SLOT_ARGUMENTS_OFFSET: usize = Address::SIZE as usize
    + MethodFingerprint::SIZE as usize
    + size_of::<TransactionMethodContext>();

/// Calls prepared methods with a function instead of the host
pub(crate) struct CallWith<Call>(Call);

impl<Call> CallWith<Call>
where
    Call: Fn(PreparedMethod<'_>) -> Result<(), ContractError>,
{
    /// Call methods with `call`
    pub(crate) fn new(call: Call) -> Self {
        Self(call)
    }
}

// SAFETY: Functions in tests write outputs and return values that later methods refer to the way
// `ExecutorContext` requires, and other `#[output]` arguments keep size `0` that they were given
unsafe impl<Call> CallPrepared for CallWith<Call>
where
    Call: Fn(PreparedMethod<'_>) -> Result<(), ContractError>,
{
    fn call_prepared(&self, method: PreparedMethod<'_>) -> Result<(), ContractError> {
        (self.0)(method)
    }
}
