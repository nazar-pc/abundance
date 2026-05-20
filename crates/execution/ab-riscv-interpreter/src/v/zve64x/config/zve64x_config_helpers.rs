//! Opaque helpers for Zve64x extension

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::zve64x_helpers::INSTRUCTION_SIZE;
use crate::{ExecutionError, ProgramCounter, RegisterFile};
use ab_riscv_primitives::prelude::*;
use core::fmt;

/// Apply `vsetvli` / `vsetvl` logic.
///
/// Both share identical `AVL` resolution; they differ only in how the `vtype` value is obtained
/// (immediate for `vsetvli`, register for `vsetvl`).
#[doc(hidden)]
pub fn apply_vsetvl<Reg, Regs, ExtState, Memory, PC, CustomError>(
    regs: &mut Regs,
    ext_state: &mut ExtState,
    program_counter: &PC,
    rd: Reg,
    rs1: Reg,
    rs1_value: Reg::Type,
    vtype_raw: Reg::Type,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
{
    // Check whether vector instructions are enabled
    if !ext_state.vector_instructions_allowed() {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }

    let Some(new_vtype) = Vtype::from_raw::<Reg>(vtype_raw) else {
        ext_state.set_vtype(None);
        ext_state.set_vl(0);
        regs.write(rd, Reg::Type::from(0u8));
        ext_state.mark_vs_dirty();
        ext_state.reset_vstart();

        return Ok(());
    };

    let vlmax = ext_state.vlmax_for_vtype(new_vtype);

    let rs1_is_zero = rs1 == Reg::ZERO;
    let rd_is_zero = rd == Reg::ZERO;

    let new_vl = if !rs1_is_zero {
        // Truncate to `u32`: `VLMAX` fits in `u32` (max 65536)
        let avl = rs1_value.as_u64() as u32;
        ext_state.compute_vl(avl, vlmax)
    } else if !rd_is_zero {
        //` rs1=x0, rd!=x0`: `AVL = max`, `result` is `VLMAX`
        vlmax
    } else {
        // `rs1=x0, rd=x0`: use current `vl` as `AVL`, keep `vl` unchanged if `VLMAX` stays the
        // same. If `VLMAX` changes, this is reserved, and we set `vill` (conservative choice per
        // spec).
        let current_vl = ext_state.vl();
        let old_vtype = ext_state.vtype();
        let old_vlmax = old_vtype.map_or_default(|old_vtype| ext_state.vlmax_for_vtype(old_vtype));

        if vlmax != old_vlmax {
            ext_state.set_vtype(None);
            ext_state.set_vl(0);
            ext_state.mark_vs_dirty();
            ext_state.reset_vstart();

            return Ok(());
        }

        ext_state.compute_vl(current_vl, vlmax)
    };

    ext_state.set_vtype(Some(new_vtype));
    ext_state.set_vl(new_vl);
    regs.write(rd, Reg::Type::from(new_vl));
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();

    Ok(())
}

/// Apply `vsetivli` logic.
///
/// `AVL` comes from 5-bit zero-extended immediate (0..31). No `rs1=x0/rd=x0` special casing
/// applies to this variant.
#[doc(hidden)]
pub fn apply_vsetivli<Reg, Regs, ExtState, Memory, PC, CustomError>(
    regs: &mut Regs,
    ext_state: &mut ExtState,
    program_counter: &PC,
    rd: Reg,
    uimm: u8,
    vtypei: u16,
) -> Result<(), ExecutionError<Reg::Type, CustomError>>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
{
    // Check whether vector instructions are enabled
    if !ext_state.vector_instructions_allowed() {
        return Err(ExecutionError::IllegalInstruction {
            address: program_counter.old_pc(INSTRUCTION_SIZE),
        });
    }

    let vtype_raw = Reg::Type::from(vtypei);

    if let Some(new_vtype) = Vtype::from_raw::<Reg>(vtype_raw) {
        let vlmax = ext_state.vlmax_for_vtype(new_vtype);
        let avl = u32::from(uimm);
        let new_vl = ext_state.compute_vl(avl, vlmax);

        ext_state.set_vtype(Some(new_vtype));
        ext_state.set_vl(new_vl);
        regs.write(rd, Reg::Type::from(new_vl));
    } else {
        ext_state.set_vtype(None);
        ext_state.set_vl(0);
        regs.write(rd, Reg::Type::from(0u8));
    }
    ext_state.mark_vs_dirty();
    ext_state.reset_vstart();

    Ok(())
}
