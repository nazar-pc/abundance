//! Opaque helpers for Zicsr extension

use crate::{CsrError, Csrs};
use ab_riscv_primitives::prelude::*;

/// CSR privilege level check helper.
///
/// Returns `Err` if `current` is below the privilege level encoded in `csr_index` bits `[9:8]`.
/// May return `Ok(())` for invalid CSRs for efficiency reasons since those will be rejected by
/// extensions anyway.
#[inline(always)]
#[doc(hidden)]
pub fn check_csr_privilege_level<Reg, C, CustomError>(
    csrs: &C,
    csr_index: u16,
) -> Result<(), CsrError<CustomError>>
where
    Reg: Register,
    [(); Reg::N]:,
    C: Csrs<Reg, CustomError>,
{
    let current = csrs.privilege_level();
    let required_bits = ((csr_index >> 8) & 0b11) as u8;
    // Privilege level uses two bits. Using machine value as a placeholder (`0b11`) allows the
    // compiler to optimize this whole function away if `csrs.privilege_level()` returns fixed
    // `PrivilegeLevel::Machine` value, which is the most common case since `0b11` is larger or
    // equal than any other 2-bit value. Invalid level will still be rejected at a later stage as
    // unknown CSR.
    let required = PrivilegeLevel::from_bits(required_bits).unwrap_or(PrivilegeLevel::Machine);

    if current >= required {
        Ok(())
    } else {
        Err(CsrError::InsufficientPrivilege {
            csr_index,
            required,
            current,
        })
    }
}
