//! Opaque helpers for ZveXx extension

use crate::v::vector_config::VectorConfig;
use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::zvexx_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, PackedAddress, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Apply `vsetvli` / `vsetvl` logic.
///
/// Both share identical `AVL` resolution; they differ only in how the `vtype` value is obtained
/// (immediate for `vsetvli`, register for `vsetvl`).
///
/// Returns `rd_value`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub const fn apply_vsetvl<Reg, Env, Memory, PC>(
    env: &mut Env,
    program_counter: &PC,
    rd: Reg,
    rs1: Reg,
    rs1_value: Reg::Type,
    vtype_raw: Reg::Type,
) -> Result<Reg::Type, ExecutionError<Reg::Type>>
where
    Reg: [const] Register,
    Env: [const] VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    PC: [const] ProgramCounter<Reg::Type, Memory>,
{
    // Check whether vector instructions are enabled
    if !env.vector_instructions_allowed() {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    }

    let Some(new_vtype) = Vtype::from_raw::<Reg>(vtype_raw) else {
        cold_path();
        env.set_vector_config(None);
        env.mark_vs_dirty();
        env.reset_vstart();

        return Ok(Reg::Type::from(0u8));
    };

    let rs1_is_zero = rs1 == Reg::ZERO;
    let rd_is_zero = rd == Reg::ZERO;

    let new_config = if !rs1_is_zero {
        // AVL is an unsigned XLEN-wide value. Saturate rather than truncate, since anything above
        // `u32::MAX` exceeds any `VLMAX` just as well.
        let avl = u32::try_from(rs1_value.as_u64()).unwrap_or(u32::MAX);
        VectorConfig::from_avl(new_vtype, Vl::new_saturating(avl))
    } else if !rd_is_zero {
        // `rs1=x0, rd!=x0`: `AVL = max`, `result` is `VLMAX`
        VectorConfig::from_avl(new_vtype, new_vtype.vlmax())
    } else if let Some(old_config) = env.vector_config()
        && old_config.vlmax() == new_vtype.vlmax()
    {
        // `rs1=x0, rd=x0`: use current `vl` as `AVL`, which keeps `vl` unchanged since `VLMAX`
        // stays the same
        VectorConfig::from_avl(new_vtype, old_config.vl())
    } else {
        // `rs1=x0, rd=x0` with `VLMAX` changing (or `vill` set) is reserved, and we set `vill`
        // (conservative choice per spec)
        cold_path();
        env.set_vector_config(None);
        env.mark_vs_dirty();
        env.reset_vstart();

        return Ok(Reg::Type::from(0u8));
    };

    env.set_vector_config(Some(new_config));
    env.mark_vs_dirty();
    env.reset_vstart();

    Ok(Reg::Type::from(u32::from(new_config.vl())))
}

/// Apply `vsetivli` logic.
///
/// `AVL` comes from 5-bit zero-extended immediate (0..31). No `rs1=x0/rd=x0` special casing
/// applies to this variant.
///
/// Returns `rd_value`.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub const fn apply_vsetivli<Reg, Env, Memory, PC>(
    env: &mut Env,
    program_counter: &PC,
    uimm: u8,
    vtypei: u16,
) -> Result<Reg::Type, ExecutionError<Reg::Type>>
where
    Reg: [const] Register,
    Env: [const] VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    PC: [const] ProgramCounter<Reg::Type, Memory>,
{
    // Check whether vector instructions are enabled
    if !env.vector_instructions_allowed() {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    }

    let vtype_raw = Reg::Type::from(vtypei);

    let rd_value = if let Some(new_vtype) = Vtype::from_raw::<Reg>(vtype_raw) {
        let new_config = VectorConfig::from_avl(new_vtype, Vl::from(uimm));
        env.set_vector_config(Some(new_config));
        Reg::Type::from(u32::from(new_config.vl()))
    } else {
        cold_path();
        env.set_vector_config(None);
        Reg::Type::from(0u8)
    };
    env.mark_vs_dirty();
    env.reset_vstart();

    Ok(rd_value)
}
