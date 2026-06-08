//! ZveXx vector store instructions

#[cfg(test)]
mod tests;
pub mod zvexx_store_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::load::zvexx_load_helpers;
use crate::v::zvexx::zvexx_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::ops::ControlFlow;

#[instruction_execution]
impl<Reg> ExecutableInstructionOperands for ZveXxStoreInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, ExtState, CustomError> ExecutableInstructionCsr<ExtState, CustomError>
    for ZveXxStoreInstruction<Reg>
where
    Reg: Register,
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler, CustomError>
    for ZveXxStoreInstruction<Reg>
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
            // Whole-register store: stores `nreg` consecutive registers starting at `vs3` directly
            // to memory as a flat byte array of `EVL = nreg * VLENB` bytes. `vs3` must be aligned
            // to `nreg`. Ignores vtype, vl, masking. Honors `vstart` in byte units: the first
            // `vstart` bytes are skipped. If `vstart >= EVL`, the instruction is a no-op.
            Self::Vsr { vs3, rs1: _, nreg } => {
                let nreg = nreg.num_registers();
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                if vs3.to_bits() % nreg != 0 {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vlenb = u64::from(ExtState::VLENB);
                let evl = u64::from(nreg) * vlenb;
                let vstart = ext_state.vstart();
                if u64::from(vstart) < evl {
                    let base = rs1_value.as_u64();
                    let mut byte_off = u64::from(vstart);
                    while byte_off < evl {
                        let reg_off = byte_off / vlenb;
                        let in_reg = (byte_off % vlenb) as usize;
                        // SAFETY: the decoder guarantees `nreg` in {1,2,4,8} and `vs3` is
                        // `nreg`-aligned (checked above), so `vs3.to_bits() + nreg - 1 <= 31`
                        let reg = unsafe {
                            VReg::from_bits(vs3.to_bits() + reg_off as u8).unwrap_unchecked()
                        };
                        // SAFETY: `in_reg < VLENB` by construction
                        let src =
                            unsafe { ext_state.read_vregs().get(reg).get_unchecked(in_reg..) };
                        if let Err(error) = memory.write_slice(base + byte_off, src) {
                            ext_state.set_vstart(byte_off as u16);
                            return Err(ExecutionError::MemoryAccess(error));
                        }
                        byte_off += src.len() as u64;
                    }
                }
                ext_state.reset_vstart();
            }
            // Mask store: stores `ceil(vl / 8)` bytes from `vs3` to memory with no masking.
            // Does not require a valid vtype: when vill is set vl is 0, so zero bytes are written.
            // Honors `vstart` at byte granularity: the first `vstart / 8` bytes are skipped.
            Self::Vsm { vs3, rs1: _ } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let vl = ext_state.vl();
                let evl_bytes = vl.div_ceil(u8::BITS);
                let start_byte = ext_state.vstart();
                if u32::from(start_byte) < evl_bytes {
                    let base = rs1_value.as_u64();
                    // SAFETY: `evl_bytes = vl.div_ceil(8) <= VLEN / 8 = VLENB` because
                    // `vl <= VLMAX <= VLEN`, so the slice `start_byte..evl_bytes` is in bounds of
                    // the `VLENB`-byte source register
                    let src = unsafe {
                        ext_state
                            .read_vregs()
                            .get(vs3)
                            .get_unchecked(usize::from(start_byte)..evl_bytes as usize)
                    };
                    memory
                        .write_slice(base + u64::from(start_byte), src)
                        .map_err(ExecutionError::MemoryAccess)?;
                }
                ext_state.reset_vstart();
            }
            // Unit-stride store.
            //
            // Source EMUL = EEW/SEW * LMUL, computed via `data_register_count`. This gives
            // `group_regs` such that `VLMAX = group_regs * VLENB / eew.bytes()` matches the
            // architectural `vl`.
            Self::Vse {
                vs3,
                rs1: _,
                vm,
                eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
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
                    zvexx_store_helpers::execute_unit_stride_store(
                        ext_state,
                        memory,
                        vs3,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        Nf::N1,
                    )?;
                }
            }
            // Strided store
            Self::Vsse {
                vs3,
                rs1: _,
                rs2: _,
                vm,
                eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                )?;
                let stride = rs2_value.as_i64();
                // SAFETY: same preconditions as `Vse`.
                unsafe {
                    zvexx_store_helpers::execute_strided_store(
                        ext_state,
                        memory,
                        vs3,
                        vm,
                        rs1_value.as_u64(),
                        stride,
                        eew,
                        group_regs,
                        Nf::N1,
                    )?;
                }
            }
            // Indexed-unordered store. Ordering between elements is not guaranteed.
            Self::Vsuxei {
                vs3,
                rs1: _,
                vs2,
                vm,
                eew: index_eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
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
                    zvexx_store_helpers::execute_indexed_store(
                        ext_state,
                        memory,
                        vs3,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        data_eew,
                        index_eew,
                        data_group_regs,
                        Nf::N1,
                    )?;
                }
            }
            // Indexed-ordered store. Elements must be written in element order.
            // The ordering constraint is visible only to other harts/devices; the implementation
            // here is already sequential, so no additional logic is needed.
            Self::Vsoxei {
                vs3,
                rs1: _,
                vs2,
                vm,
                eew: index_eew,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY: identical precondition argument to `Vsuxei`
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        ext_state,
                        memory,
                        vs3,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        data_eew,
                        index_eew,
                        data_group_regs,
                        Nf::N1,
                    )?;
                }
            }
            // Unit-stride segment store: `nf` fields per element, stored contiguously
            Self::Vsseg {
                vs3,
                rs1: _,
                eew,
                vm_nf,
            } => {
                let vm = vm_nf.vm();
                let nf = vm_nf.nf();
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                    nf,
                )?;
                // SAFETY:
                // - `validate_segment_store_registers` guarantees `vs3 % group_regs == 0` and `vs3
                //   + nf * group_regs <= 32`
                // - `vl <= group_regs * VLENB / eew.bytes()`: same EMUL argument as `Vse`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zvexx_store_helpers::execute_unit_stride_store(
                        ext_state,
                        memory,
                        vs3,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        nf,
                    )?;
                }
            }
            // Strided segment store
            Self::Vssseg {
                vs3,
                rs1: _,
                rs2: _,
                eew,
                vm_nf,
            } => {
                let vm = vm_nf.vm();
                let nf = vm_nf.nf();
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let group_regs = vtype.vlmul().data_register_count(eew, vtype.vsew()).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    },
                )?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                    nf,
                )?;
                let stride = rs2_value.as_i64();
                // SAFETY: same as `Vsseg`.
                unsafe {
                    zvexx_store_helpers::execute_strided_store(
                        ext_state,
                        memory,
                        vs3,
                        vm,
                        rs1_value.as_u64(),
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
                rs1: _,
                vs2,
                eew: index_eew,
                vm_nf,
            } => {
                let vm = vm_nf.vm();
                let nf = vm_nf.nf();
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    })?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                    nf,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY:
                // - `validate_segment_store_registers` covers `vs3` alignment/bounds
                // - `check_register_group_alignment` covers `vs2` alignment/bounds
                // - `vl` bounded by both EMUL groups as in `Vsuxei`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        ext_state,
                        memory,
                        vs3,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
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
                rs1: _,
                vs2,
                eew: index_eew,
                vm_nf,
            } => {
                let vm = vm_nf.vm();
                let nf = vm_nf.nf();
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return Err(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype
                    .vlmul()
                    .index_register_count(index_eew, vtype.vsew())
                    .ok_or(ExecutionError::IllegalInstruction {
                        address: program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    })?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                    nf,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // SAFETY: identical precondition argument to `Vsuxseg`
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        ext_state,
                        memory,
                        vs3,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        data_eew,
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
