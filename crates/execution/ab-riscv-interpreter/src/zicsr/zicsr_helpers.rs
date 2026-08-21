//! Opaque helpers for Zicsr extension

use crate::{CsrError, Csrs, ExecutableInstructionCsr};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// CSR privilege level check helper.
///
/// Returns `Err` if `current` is below the privilege level encoded in `csr_index` bits `[9:8]`.
/// May return `Ok(())` for invalid CSRs for efficiency reasons since those will be rejected by
/// extensions anyway.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub const fn check_csr_privilege_level<Reg, C>(csrs: &C, csr_index: u16) -> Result<(), CsrError>
where
    Reg: [const] Register,
    C: [const] Csrs<Reg>,
{
    let current = csrs.privilege_level();
    let required_bits = ((csr_index >> 8u8) & 0b11) as u8;
    // Privilege level uses two bits. Using machine value as a placeholder (`0b11`) allows the
    // compiler to optimize this whole function away if `csrs.privilege_level()` returns fixed
    // `PrivilegeLevel::Machine` value, which is the most common case since `0b11` is larger or
    // equal than any other 2-bit value. Invalid level will still be rejected at a later stage as
    // unknown CSR.
    let required = PrivilegeLevel::from_bits(required_bits).unwrap_or(PrivilegeLevel::Machine);

    if current >= required {
        Ok(())
    } else {
        cold_path();
        Err(CsrError::InsufficientPrivilege {
            csr_index,
            required,
            current,
        })
    }
}

/// Process a CSR read by calling [`ExecutableInstructionCsr::prepare_csr_read()`] of the root
/// instruction `I` and returning the output value on success.
///
/// `will_write` indicates whether this same CSR instruction will also perform a write afterward,
/// see [`ExecutableInstructionCsr::prepare_csr_read()`] for details.
#[inline(always)]
#[doc(hidden)]
pub const fn process_csr_read<Reg, ExtState, I>(
    ext_state: &ExtState,
    csr_index: u16,
    will_write: bool,
    raw_value: Reg::Type,
) -> Result<Reg::Type, CsrError>
where
    Reg: [const] Register,
    I: [const] ExecutableInstructionCsr<ExtState, Reg = Reg>,
{
    let mut out = Reg::Type::default();
    match I::prepare_csr_read(ext_state, csr_index, will_write, raw_value, &mut out) {
        Ok(true) => Ok(out),
        Ok(false) => {
            cold_path();
            Err(CsrError::IllegalRead { csr_index })
        }
        Err(err) => {
            cold_path();
            Err(err)
        }
    }
}

/// Process a CSR write by calling [`ExecutableInstructionCsr::prepare_csr_write()`] of the root
/// instruction `I` and returning the output value on success.
#[inline(always)]
#[doc(hidden)]
pub const fn process_csr_write<Reg, ExtState, I>(
    ext_state: &mut ExtState,
    csr_index: u16,
    write_value: Reg::Type,
) -> Result<Reg::Type, CsrError>
where
    Reg: [const] Register,
    I: [const] ExecutableInstructionCsr<ExtState, Reg = Reg>,
{
    let mut out = Reg::Type::default();
    match I::prepare_csr_write(ext_state, csr_index, write_value, &mut out) {
        Ok(true) => Ok(out),
        Ok(false) => {
            cold_path();
            Err(CsrError::IllegalWrite { csr_index })
        }
        Err(err) => {
            cold_path();
            Err(err)
        }
    }
}
