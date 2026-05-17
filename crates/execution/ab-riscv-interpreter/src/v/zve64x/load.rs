//! Zve64x vector load instructions

#[cfg(test)]
mod tests;
pub mod zve64x_load_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for Zve64xLoadInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for Zve64xLoadInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for Zve64xLoadInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg, CustomError>,
    [(); ExtState::ELEN as usize]:,
    [(); ExtState::VLEN as usize]:,
    [(); ExtState::VLENB as usize]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory, CustomError>,
    CustomError: fmt::Debug,
{
    #[inline(always)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        ext_state: &mut ExtState,
        memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> Result<
        ControlFlow<(), (Self::Reg, <Self::Reg as Register>::Type)>,
        ExecutionError<Reg::Type, CustomError>,
    > {
        match self {
            // Whole-register load: loads `nreg` consecutive registers starting at `vd` directly
            // from memory. `vd` must be aligned to `nreg`. Ignores vtype, vl, vstart, masking.
            Self::Vlr {
                vd,
                rs1: _,
                nreg,
                eew: _,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if u32::from(vd.bits()) % u32::from(nreg) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let base = rs1_value.as_u64();
                let vlenb = u64::from(ExtState::VLENB);
                for reg_off in 0..u64::from(nreg) {
                    let reg_idx = u64::from(vd.bits()) + reg_off;
                    let bytes = match memory.read_slice(base + reg_off * vlenb, ExtState::VLENB) {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            if reg_off > 0 {
                                ext_state.mark_vs_dirty();
                                ext_state.reset_vstart();
                            }
                            Err(ExecutionError::MemoryAccess(error))?
                        }
                    };
                    // SAFETY: `reg_idx < 32` because the decoder guarantees nreg in {1,2,4,8}
                    // and vd is nreg-aligned (checked above), so vd.bits() + nreg - 1 <= 31.
                    // `read_slice` returns a slice of exactly `ExtState::VLENB` bytes on success,
                    // matching `dst`'s length, so `copy_from_slice` cannot panic.
                    let dst = unsafe { ext_state.write_vreg().get_unchecked_mut(reg_idx as usize) };
                    dst.copy_from_slice(bytes);
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }

            // Mask load: loads ceil(vl / 8) bytes from base into vd with no masking applied.
            // Does not require a valid vtype: when vill is set vl is 0, so zero bytes are read.
            Self::Vlm { vd, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vl = ext_state.vl();
                let byte_count = vl.div_ceil(u8::BITS);
                if byte_count > 0 {
                    let base = rs1_value.as_u64();
                    let bytes = memory.read_slice(base, byte_count)?;
                    // SAFETY: `vd.bits() < 32` is guaranteed by the `VReg` type.
                    // `bytes.len() == byte_count = vl.div_ceil(8) <= VLEN / 8 = VLENB` because
                    // `vl <= VLMAX <= VLEN`, so `..bytes.len()` is in bounds within the
                    // `VLENB`-byte destination register.
                    unsafe {
                        ext_state
                            .write_vreg()
                            .get_unchecked_mut(usize::from(vd.bits()))
                            .get_unchecked_mut(..bytes.len())
                            .copy_from_slice(bytes);
                    }
                }
                ext_state.mark_vs_dirty();
                ext_state.reset_vstart();
            }

            // Unit-stride load.
            //
            // Destination EMUL = EEW/SEW * LMUL, computed via `index_register_count`. This
            // gives `group_regs` such that `VLMAX = group_regs * VLENB / eew.bytes()` matches
            // the architectural `vl`.
            Self::Vle {
                vd,
                rs1: _,
                vm,
                eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype
                    .vlmul()
                    .index_register_count(eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && zve64x_load_helpers::groups_overlap(vd, group_regs, VReg::V0, 1) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // SAFETY:
                // - 1 <= MAX_NF
                // - alignment: `check_register_group_alignment` verified `vd % group_regs == 0` and
                //   `vd + group_regs <= 32`, satisfying both the alignment and nf=1 bounds
                //   preconditions
                // - `vl <= group_regs * VLENB / eew.bytes()`: `group_regs` is the EMUL computed for
                //   this `eew` and `vtype`, so this VLMAX equals the architectural VLMAX that
                //   bounds `vl`
                // - mask overlap: checked above via `groups_overlap`
                unsafe {
                    zve64x_load_helpers::execute_unit_stride_load(
                        ext_state,
                        memory,
                        vd,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        1,
                        false,
                    )?;
                }
            }

            // Fault-only-first unit-stride load. Preconditions identical to `Vle`.
            Self::Vleff {
                vd,
                rs1: _,
                vm,
                eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype
                    .vlmul()
                    .index_register_count(eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && zve64x_load_helpers::groups_overlap(vd, group_regs, VReg::V0, 1) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // SAFETY: preconditions identical to `Vle`; see that arm for the full argument.
                unsafe {
                    zve64x_load_helpers::execute_unit_stride_load(
                        ext_state,
                        memory,
                        vd,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        1,
                        true,
                    )?;
                }
            }

            // Strided load. Destination EMUL = EEW/SEW * LMUL as for unit-stride.
            Self::Vlse {
                vd,
                rs1: _,
                rs2: _,
                vm,
                eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype
                    .vlmul()
                    .index_register_count(eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm && zve64x_load_helpers::groups_overlap(vd, group_regs, VReg::V0, 1) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // rs2 holds a signed stride; reinterpret the register value as signed.
                let stride = rs2_value.as_u64().cast_signed();
                // SAFETY:
                // - alignment and nf=1 bounds: `check_register_group_alignment` verified `vd %
                //   group_regs == 0` and `vd + group_regs <= 32`
                // - `vl <= group_regs * VLENB / eew.bytes()`: `group_regs` is the EMUL for this
                //   `eew` and `vtype`, so this VLMAX equals the architectural VLMAX bounding `vl`
                // - mask overlap: checked above via `groups_overlap`
                unsafe {
                    zve64x_load_helpers::execute_strided_load(
                        ext_state,
                        memory,
                        vd,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        stride,
                        eew,
                        group_regs,
                        1,
                    )?;
                }
            }

            // Indexed-unordered load: eew is the index EEW; data EEW comes from vtype.vsew().
            // The data destination uses the base LMUL (data EEW = SEW for indexed loads).
            Self::Vluxei {
                vd,
                rs1: _,
                vs2,
                vm,
                eew: index_eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    data_group_regs,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                if zve64x_load_helpers::groups_overlap(vd, data_group_regs, vs2, index_group_regs) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && zve64x_load_helpers::groups_overlap(vd, data_group_regs, VReg::V0, 1) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // SAFETY:
                // - data alignment/nf=1 bounds: `check_register_group_alignment` on `vd`
                // - index alignment/bounds: `check_register_group_alignment` on `vs2`
                // - `vl <= data_group_regs * VLENB / data_eew.bytes()`: data EEW = SEW and
                //   `data_group_regs = LMUL`, so VLMAX = LMUL * VLEN / SEW, which bounds `vl`
                // - `vl <= index_group_regs * VLENB / index_eew.bytes()`: `index_group_regs` is
                //   EMUL_index defined so this VLMAX_index equals the architectural VLMAX
                // - no overlap between data and index groups: checked above
                // - mask overlap: checked above via `groups_overlap`
                unsafe {
                    zve64x_load_helpers::execute_indexed_load(
                        ext_state,
                        memory,
                        vd,
                        vs2,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        1,
                    )?;
                }
            }

            // Indexed-ordered load: functionally identical to `Vluxei` for a software
            // interpreter; memory access ordering has no observable effect here.
            Self::Vloxei {
                vd,
                rs1: _,
                vs2,
                vm,
                eew: index_eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    data_group_regs,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                if zve64x_load_helpers::groups_overlap(vd, data_group_regs, vs2, index_group_regs) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if !vm && zve64x_load_helpers::groups_overlap(vd, data_group_regs, VReg::V0, 1) {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // SAFETY: preconditions identical to `Vluxei`; see that arm for the full
                // argument.
                unsafe {
                    zve64x_load_helpers::execute_indexed_load(
                        ext_state,
                        memory,
                        vd,
                        vs2,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        1,
                    )?;
                }
            }

            // Unit-stride segment load. EMUL = EEW/SEW * LMUL per field group.
            Self::Vlseg {
                vd,
                rs1: _,
                vm,
                eew,
                nf,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype
                    .vlmul()
                    .index_register_count(eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::validate_segment_registers::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vm,
                    group_regs,
                    nf,
                )?;
                if nf > zve64x_load_helpers::MAX_NF {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // SAFETY:
                // - `nf <= MAX_NF` checked above
                // - alignment and nf-group bounds: `validate_segment_registers` verified `vd %
                //   group_regs == 0` and `vd + nf * group_regs <= 32`
                // - `vl <= group_regs * VLENB / eew.bytes()`: `group_regs` is the EMUL for this
                //   `eew` and `vtype`, so this VLMAX equals the architectural VLMAX bounding `vl`
                // - mask overlap with v0: `validate_segment_registers` checked `vd.bits() != 0`
                //   when `vm=false`, ensuring no field group contains v0
                unsafe {
                    zve64x_load_helpers::execute_unit_stride_load(
                        ext_state,
                        memory,
                        vd,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        nf,
                        false,
                    )?;
                }
            }

            // Fault-only-first segment load. Preconditions identical to `Vlseg`.
            Self::Vlsegff {
                vd,
                rs1: _,
                vm,
                eew,
                nf,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype
                    .vlmul()
                    .index_register_count(eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::validate_segment_registers::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vm,
                    group_regs,
                    nf,
                )?;
                if nf > zve64x_load_helpers::MAX_NF {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                // SAFETY: preconditions identical to `Vlseg`; see that arm for the full argument.
                unsafe {
                    zve64x_load_helpers::execute_unit_stride_load(
                        ext_state,
                        memory,
                        vd,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        nf,
                        true,
                    )?;
                }
            }

            // Strided segment load. EMUL = EEW/SEW * LMUL as for `Vlse`.
            Self::Vlsseg {
                vd,
                rs1: _,
                rs2: _,
                vm,
                eew,
                nf,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype
                    .vlmul()
                    .index_register_count(eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::validate_segment_registers::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vm,
                    group_regs,
                    nf,
                )?;
                let stride = rs2_value.as_u64().cast_signed();
                // SAFETY:
                // - alignment and nf-group bounds: `validate_segment_registers` verified `vd %
                //   group_regs == 0` and `vd + nf * group_regs <= 32`
                // - `vl <= group_regs * VLENB / eew.bytes()`: `group_regs` is EMUL for this `eew`
                //   and `vtype`
                // - mask overlap: `validate_segment_registers` checked `vd.bits() != 0` when
                //   `vm=false`
                unsafe {
                    zve64x_load_helpers::execute_strided_load(
                        ext_state,
                        memory,
                        vd,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        stride,
                        eew,
                        group_regs,
                        nf,
                    )?;
                }
            }

            // Indexed-unordered segment load
            Self::Vluxseg {
                vd,
                rs1: _,
                vs2,
                vm,
                eew: index_eew,
                nf,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                // `validate_segment_registers` is called before the per-field overlap loop so
                // that `vd.bits() + f * data_group_regs < 32` is established for all `f < nf`,
                // which is required by the `VReg::from_bits` call inside the loop.
                zve64x_load_helpers::validate_segment_registers::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vm,
                    data_group_regs,
                    nf,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                for f in 0..nf {
                    // SAFETY: `vd.bits() + f * data_group_regs < 32` because
                    // `validate_segment_registers` established `vd.bits() + nf * data_group_regs
                    // <= 32` and `f < nf`. The value is in [0, 31], so it is a valid `VReg`
                    // encoding.
                    let field_vd = unsafe {
                        VReg::from_bits(vd.bits() + f * data_group_regs).unwrap_unchecked()
                    };
                    if zve64x_load_helpers::groups_overlap(
                        field_vd,
                        data_group_regs,
                        vs2,
                        index_group_regs,
                    ) {
                        Err(ExecutionError::IllegalInstruction {
                            address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                        })?;
                    }
                }
                // SAFETY:
                // - data alignment/nf-group bounds: `validate_segment_registers` verified `vd %
                //   data_group_regs == 0` and `vd + nf * data_group_regs <= 32`
                // - index alignment/bounds: `check_register_group_alignment` verified `vs2 %
                //   EMUL_index == 0` and `vs2 + EMUL_index <= 32`
                // - no field/index group overlap: verified by the loop above
                // - `vl <= data_group_regs * VLENB / data_eew.bytes()`: data EEW = SEW and
                //   `data_group_regs = LMUL`, so VLMAX = LMUL * VLEN / SEW bounds `vl`
                // - `vl <= EMUL_index * VLENB / index_eew.bytes()`: `index_group_regs` (EMUL_index)
                //   is defined so this VLMAX_index equals the architectural VLMAX
                // - mask overlap: `validate_segment_registers` checked `vd.bits() != 0` when
                //   `vm=false`, and no field group starts at 0 since groups are contiguous from
                //   `vd` which is nonzero
                unsafe {
                    zve64x_load_helpers::execute_indexed_load(
                        ext_state,
                        memory,
                        vd,
                        vs2,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        nf,
                    )?;
                }
            }

            // Indexed-ordered segment load: functionally identical to `Vluxseg` for a software
            // interpreter
            Self::Vloxseg {
                vd,
                rs1: _,
                vs2,
                vm,
                eew: index_eew,
                nf,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::validate_segment_registers::<Reg, _, _, _>(
                    program_counter,
                    vd,
                    vm,
                    data_group_regs,
                    nf,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                for f in 0..nf {
                    // SAFETY: `vd.bits() + f * data_group_regs < 32` because
                    // `validate_segment_registers` established `vd.bits() + nf * data_group_regs
                    // <= 32` and `f < nf`. The value is in [0, 31], so it is a valid `VReg`
                    // encoding.
                    let field_vd = unsafe {
                        VReg::from_bits(vd.bits() + f * data_group_regs).unwrap_unchecked()
                    };
                    if zve64x_load_helpers::groups_overlap(
                        field_vd,
                        data_group_regs,
                        vs2,
                        index_group_regs,
                    ) {
                        Err(ExecutionError::IllegalInstruction {
                            address: program_counter.old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                        })?;
                    }
                }
                // SAFETY: preconditions identical to `Vluxseg`; see that arm for the full
                // argument
                unsafe {
                    zve64x_load_helpers::execute_indexed_load(
                        ext_state,
                        memory,
                        vd,
                        vs2,
                        vm,
                        ext_state.vl(),
                        u32::from(ext_state.vstart()),
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        nf,
                    )?;
                }
            }
        }

        Ok(ControlFlow::Continue(Default::default()))
    }
}
