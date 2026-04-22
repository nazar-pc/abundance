//! Zve64x vector store instructions

#[cfg(test)]
mod tests;
pub mod zve64x_store_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zve64x::load::zve64x_load_helpers;
use crate::v::zve64x::zve64x_helpers;
use crate::{
    ExecutableInstruction, ExecutionError, InterpreterState, ProgramCounter, RegisterFile,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<
        InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
        CustomError,
    > for Zve64xStoreInstruction<Reg>
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
        state: &mut InterpreterState<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>,
    ) -> Result<ControlFlow<()>, ExecutionError<Reg::Type, CustomError>> {
        match self {
            // Whole-register store: stores `nreg` consecutive registers starting at `vs3` directly
            // to memory as a flat byte array of `EVL = nreg * VLENB` bytes. `vs3` must be aligned
            // to `nreg`. Ignores vtype, vl, masking. Honors `vstart` in byte units: the first
            // `vstart` bytes are skipped. If `vstart >= EVL`, the instruction is a no-op.
            Self::Vsr { vs3, rs1, nreg } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                if u32::from(vs3.bits()) % u32::from(nreg) != 0 {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vlenb = u64::from(ExtState::VLENB);
                let evl = u64::from(nreg) * vlenb;
                let vstart = u64::from(state.ext_state.vstart());
                if vstart < evl {
                    let base = state.regs.read(rs1).as_u64();
                    let mut byte_off = vstart;
                    while byte_off < evl {
                        let reg_off = byte_off / vlenb;
                        let in_reg = (byte_off % vlenb) as usize;
                        let reg_idx = (u64::from(vs3.bits()) + reg_off) as usize;
                        // SAFETY: `reg_idx < 32` because the decoder guarantees `nreg` in
                        // {1,2,4,8} and `vs3` is `nreg`-aligned (checked above), so
                        // `vs3.bits() + nreg - 1 <= 31`. `in_reg < VLENB` by construction.
                        let src = unsafe {
                            state
                                .ext_state
                                .read_vreg()
                                .get_unchecked(reg_idx)
                                .get_unchecked(in_reg..)
                        };
                        if let Err(error) = state.memory.write_slice(base + byte_off, src) {
                            state.ext_state.set_vstart(byte_off as u16);
                            return Err(ExecutionError::MemoryAccess(error));
                        }
                        byte_off += src.len() as u64;
                    }
                }
                state.ext_state.reset_vstart();
            }
            // Mask store: stores `ceil(vl / 8)` bytes from `vs3` to memory with no masking.
            // Does not require a valid vtype: when vill is set vl is 0, so zero bytes are written.
            // Honors `vstart` at byte granularity: the first `vstart / 8` bytes are skipped.
            Self::Vsm { vs3, rs1 } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vl = state.ext_state.vl();
                let evl_bytes = vl.div_ceil(u8::BITS);
                let start_byte = u32::from(state.ext_state.vstart());
                if start_byte < evl_bytes {
                    let base = state.regs.read(rs1).as_u64();
                    // SAFETY: `vs3.bits() < 32` is guaranteed by `VReg`.
                    // `evl_bytes = vl.div_ceil(8) <= VLEN / 8 = VLENB` because `vl <= VLMAX <=
                    // VLEN`, so the slice `start_byte..evl_bytes` is in bounds of the
                    // `VLENB`-byte source register.
                    let src = unsafe {
                        state
                            .ext_state
                            .read_vreg()
                            .get_unchecked(usize::from(vs3.bits()))
                            .get_unchecked(start_byte as usize..evl_bytes as usize)
                    };
                    state
                        .memory
                        .write_slice(base + u64::from(start_byte), src)
                        .map_err(ExecutionError::MemoryAccess)?;
                }
                state.ext_state.reset_vstart();
            }
            // Unit-stride store.
            //
            // Source EMUL = EEW/SEW * LMUL, computed via `data_register_count`. This gives
            // `group_regs` such that `VLMAX = group_regs * VLENB / eew.bytes()` matches the
            // architectural `vl`.
            Self::Vse { vs3, rs1, vm, eew } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state, vs3, group_regs,
                )?;
                // SAFETY:
                // - alignment: `check_register_group_alignment` verified `vs3 % group_regs == 0`
                //   and `vs3 + group_regs <= 32`
                // - `vl <= group_regs * VLENB / eew.bytes()`: `group_regs` is the EMUL computed for
                //   this `eew` and `vtype`, so this VLMAX equals the architectural VLMAX that
                //   bounds `vl`
                // - vs3/v0 overlap: stores read vs3 as a source; the spec does not restrict
                //   source/v0 overlap
                unsafe {
                    zve64x_store_helpers::execute_unit_stride_store(
                        state,
                        vs3,
                        vm,
                        state.ext_state.vl(),
                        state.ext_state.vstart(),
                        state.regs.read(rs1).as_u64(),
                        eew,
                        group_regs,
                        1,
                    )?;
                }
            }
            // Strided store
            Self::Vsse {
                vs3,
                rs1,
                rs2,
                vm,
                eew,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state, vs3, group_regs,
                )?;
                let stride = state.regs.read(rs2).as_u64().cast_signed();
                // SAFETY: same preconditions as `Vse`.
                unsafe {
                    zve64x_store_helpers::execute_strided_store(
                        state,
                        vs3,
                        vm,
                        state.ext_state.vl(),
                        state.ext_state.vstart(),
                        state.regs.read(rs1).as_u64(),
                        stride,
                        eew,
                        group_regs,
                        1,
                    )?;
                }
            }
            // Indexed-unordered store. Ordering between elements is not guaranteed.
            Self::Vsuxei {
                vs3,
                rs1,
                vs2,
                vm,
                eew: index_eew,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state,
                    vs3,
                    data_group_regs,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY:
                // - `vs3` alignment/bounds: `check_register_group_alignment` verified both
                // - `vs2` alignment/bounds: `check_register_group_alignment` verified both
                // - `vl <= data_group_regs * VLENB / data_eew.bytes()`: `data_group_regs` is the
                //   EMUL that bounds `vl`
                // - `vl <= index_group_regs * VLENB / index_eew.bytes()`: `index_register_count`
                //   returns the EMUL for the index group, which by the same argument bounds `vl`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zve64x_store_helpers::execute_indexed_store(
                        state,
                        vs3,
                        vs2,
                        vm,
                        state.ext_state.vl(),
                        u32::from(state.ext_state.vstart()),
                        state.regs.read(rs1).as_u64(),
                        data_eew,
                        index_eew,
                        data_group_regs,
                        1,
                    )?;
                }
            }
            // Indexed-ordered store. Elements must be written in element order.
            // The ordering constraint is visible only to other harts/devices; the implementation
            // here is already sequential, so no additional logic is needed.
            Self::Vsoxei {
                vs3,
                rs1,
                vs2,
                vm,
                eew: index_eew,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state,
                    vs3,
                    data_group_regs,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY: identical precondition argument to `Vsuxei`
                unsafe {
                    zve64x_store_helpers::execute_indexed_store(
                        state,
                        vs3,
                        vs2,
                        vm,
                        state.ext_state.vl(),
                        u32::from(state.ext_state.vstart()),
                        state.regs.read(rs1).as_u64(),
                        data_eew,
                        index_eew,
                        data_group_regs,
                        1,
                    )?;
                }
            }
            // Unit-stride segment store: `nf` fields per element, stored contiguously
            Self::Vsseg {
                vs3,
                rs1,
                vm,
                eew,
                nf,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_store_helpers::validate_segment_store_registers::<Reg, _, _, _, _, _, _>(
                    state, vs3, group_regs, nf,
                )?;
                // SAFETY:
                // - `validate_segment_store_registers` guarantees `vs3 % group_regs == 0` and `vs3
                //   + nf * group_regs <= 32`
                // - `vl <= group_regs * VLENB / eew.bytes()`: same EMUL argument as `Vse`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zve64x_store_helpers::execute_unit_stride_store(
                        state,
                        vs3,
                        vm,
                        state.ext_state.vl(),
                        state.ext_state.vstart(),
                        state.regs.read(rs1).as_u64(),
                        eew,
                        group_regs,
                        nf,
                    )?;
                }
            }
            // Strided segment store
            Self::Vssseg {
                vs3,
                rs1,
                rs2,
                vm,
                eew,
                nf,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zve64x_store_helpers::validate_segment_store_registers::<Reg, _, _, _, _, _, _>(
                    state, vs3, group_regs, nf,
                )?;
                let stride = state.regs.read(rs2).as_u64().cast_signed();
                // SAFETY: same as `Vsseg`.
                unsafe {
                    zve64x_store_helpers::execute_strided_store(
                        state,
                        vs3,
                        vm,
                        state.ext_state.vl(),
                        state.ext_state.vstart(),
                        state.regs.read(rs1).as_u64(),
                        stride,
                        eew,
                        group_regs,
                        nf,
                    )?;
                }
            }
            // Indexed-unordered segment store
            Self::Vsuxseg {
                vs3,
                rs1,
                vs2,
                vm,
                eew: index_eew,
                nf,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_store_helpers::validate_segment_store_registers::<Reg, _, _, _, _, _, _>(
                    state,
                    vs3,
                    data_group_regs,
                    nf,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY:
                // - `validate_segment_store_registers` covers `vs3` alignment/bounds
                // - `check_register_group_alignment` covers `vs2` alignment/bounds
                // - `vl` bounded by both EMUL groups as in `Vsuxei`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zve64x_store_helpers::execute_indexed_store(
                        state,
                        vs3,
                        vs2,
                        vm,
                        state.ext_state.vl(),
                        u32::from(state.ext_state.vstart()),
                        state.regs.read(rs1).as_u64(),
                        data_eew,
                        index_eew,
                        data_group_regs,
                        nf,
                    )?;
                }
            }
            // Indexed-ordered segment store. Sequential iteration satisfies the ordering
            // requirement.
            Self::Vsoxseg {
                vs3,
                rs1,
                vs2,
                vm,
                eew: index_eew,
                nf,
            } => {
                if !state.ext_state.vector_instructions_allowed() {
                    Err(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                }
                let vtype = state
                    .ext_state
                    .vtype()
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: state
                            .instruction_fetcher
                            .old_pc(zve64x_helpers::INSTRUCTION_SIZE),
                    })?;
                zve64x_store_helpers::validate_segment_store_registers::<Reg, _, _, _, _, _, _>(
                    state,
                    vs3,
                    data_group_regs,
                    nf,
                )?;
                zve64x_load_helpers::check_register_group_alignment::<Reg, _, _, _, _, _, _>(
                    state,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY: identical precondition argument to `Vsuxseg`
                unsafe {
                    zve64x_store_helpers::execute_indexed_store(
                        state,
                        vs3,
                        vs2,
                        vm,
                        state.ext_state.vl(),
                        u32::from(state.ext_state.vstart()),
                        state.regs.read(rs1).as_u64(),
                        data_eew,
                        index_eew,
                        data_group_regs,
                        nf,
                    )?;
                }
            }
        }

        Ok(ControlFlow::Continue(()))
    }
}
