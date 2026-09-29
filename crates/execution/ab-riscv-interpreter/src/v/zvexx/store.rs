//! ZveXx vector store instructions

#[cfg(test)]
mod tests;
pub mod zvexx_store_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::load::zvexx_load_helpers;
use crate::v::zvexx::zvexx_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, OpaqueThreadedExecutionResult,
    PackedAddress, ProgramCounter, RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands,
    ThreadedExecutableInstruction, ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZveXxStoreInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for ZveXxStoreInstruction<Reg> where Reg: Register
{}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxStoreInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    Memory: VirtualMemory,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    fn execute(
        self,
        Rs1Rs2OperandValues {
            rs1_value,
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            // Whole-register store: stores `nreg` consecutive registers starting at `vs3` directly
            // to memory as a flat byte array of `EVL = nreg * VLEN.bytes()` bytes. `vs3` must be
            // aligned to `nreg`. Ignores vtype, vl, masking. Honors `vstart` in byte
            // units: the first `vstart` bytes are skipped. If `vstart >= EVL`, the
            // instruction is a no-op.
            Self::Vsr { vs3, rs1: _, nreg } => {
                let nreg = nreg.num_registers();
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vs3.is_group_aligned(nreg) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // `evl = NREG * VLEN / 8` elements with `EEW = 8`, regardless of `vtype` and `vl`,
                // which is at most `8 * 65536 / 8` and never saturates
                let vl = Vl::new_saturating(u32::from(nreg.get()) * Env::VLEN.bytes());
                // `vstart >= evl` is reserved
                if env.vstart() >= vl {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // SAFETY:
                // - alignment: `vs3 % nreg == 0` checked above, so `vs3 + nreg <= 32`
                // - `evl = nreg * VLEN.bytes()` elements fill the register group exactly
                // - unmasked
                unsafe {
                    zvexx_store_helpers::execute_unit_stride_store(
                        env,
                        memory,
                        vs3,
                        true,
                        rs1_value.as_u64(),
                        Eew::E8,
                        nreg,
                        Nf::N1,
                        vl,
                    )?;
                }
            }
            // Mask store: stores `ceil(vl / 8)` bytes from `vs3` to memory with no masking.
            // Illegal with vill set, it depends on vtype indirectly through its constraints on vl.
            // Honors `vstart` at byte granularity: the first `vstart / 8` bytes are skipped.
            Self::Vsm { vs3, rs1: _ } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if env.vtype().is_none() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // `evl = ceil(vl / 8)` elements with `EEW = 8`
                let vl = Vl::from(env.vl().bytes());
                // Not within a single register only with `vl` above `VLEN`, which is an
                // inconsistent vector state
                if u32::from(vl) > Env::VLEN.bytes() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // SAFETY:
                // - a single register is always aligned and within the register file
                // - `evl <= VLEN.bytes()` checked above
                // - unmasked
                unsafe {
                    zvexx_store_helpers::execute_unit_stride_store(
                        env,
                        memory,
                        vs3,
                        true,
                        rs1_value.as_u64(),
                        Eew::E8,
                        VRegGroupSize::R1,
                        Nf::N1,
                        vl,
                    )?;
                }
            }
            // Unit-stride store.
            //
            // Source EMUL = EEW/SEW * LMUL, computed via `eew_register_count`. This gives
            // `group_regs` such that `VLMAX = group_regs * VLEN.bytes() / eew.bytes()` matches the
            // architectural `vl`.
            Self::Vse {
                vs3,
                rs1: _,
                vm,
                eew,
            } => {
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                )?;
                let vl = env.vl();
                // SAFETY:
                // - alignment: `check_register_group_alignment` verified `vs3 % group_regs == 0`
                //   and `vs3 + group_regs <= 32`
                // - `vl <= group_regs * VLEN.bytes() / eew.bytes()`: `group_regs` is the EMUL
                //   computed for this `eew` and `vtype`, so this VLMAX equals the architectural
                //   VLMAX that bounds `vl`
                // - vs3/v0 overlap: stores read vs3 as a source; the spec does not restrict
                //   source/v0 overlap
                unsafe {
                    zvexx_store_helpers::execute_unit_stride_store(
                        env,
                        memory,
                        vs3,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        Nf::N1,
                        vl,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                )?;
                let stride = rs2_value.as_i64();
                // SAFETY: same preconditions as `Vse`.
                unsafe {
                    zvexx_store_helpers::execute_strided_store(
                        env,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match
                if index_eew != data_eew {
                    zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                        program_counter,
                        vs2,
                        index_group_regs.get(),
                        vs3,
                        data_group_regs.get(),
                    )?;
                }
                // SAFETY:
                // - `vs3` alignment/bounds: `check_register_group_alignment` verified both
                // - `vs2` alignment/bounds: `check_register_group_alignment` verified both
                // - `vl <= data_group_regs * VLEN.bytes() / data_eew.bytes()`: `data_group_regs` is
                //   the EMUL that bounds `vl`
                // - `vl <= index_group_regs * VLEN.bytes() / index_eew.bytes()`:
                //   `eew_register_count` returns the EMUL for the index group, which by the same
                //   argument bounds `vl`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        env,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match
                if index_eew != data_eew {
                    zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                        program_counter,
                        vs2,
                        index_group_regs.get(),
                        vs3,
                        data_group_regs.get(),
                    )?;
                }
                // SAFETY: identical precondition argument to `Vsuxei`
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        env,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                    nf,
                )?;
                let vl = env.vl();
                // SAFETY:
                // - `validate_segment_store_registers` guarantees `vs3 % group_regs == 0` and `vs3
                //   + nf * group_regs <= 32`
                // - `vl <= group_regs * VLEN.bytes() / eew.bytes()`: same EMUL argument as `Vse`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zvexx_store_helpers::execute_unit_stride_store(
                        env,
                        memory,
                        vs3,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        nf,
                        vl,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _>(
                    program_counter,
                    vs3,
                    group_regs,
                    nf,
                )?;
                let stride = rs2_value.as_i64();
                // SAFETY: same as `Vsseg`.
                unsafe {
                    zvexx_store_helpers::execute_strided_store(
                        env,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                    nf,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match across all fields
                if index_eew != data_eew {
                    zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                        program_counter,
                        vs2,
                        index_group_regs.get(),
                        vs3,
                        nf.fields_per_segment() * data_group_regs.get(),
                    )?;
                }
                // SAFETY:
                // - `validate_segment_store_registers` covers `vs3` alignment/bounds
                // - `check_register_group_alignment` covers `vs2` alignment/bounds
                // - `vl` bounded by both EMUL groups as in `Vsuxei`
                // - vs3/v0 overlap: stores read vs3 as a source; no restriction
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        env,
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
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = env.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = vtype.vsew().as_eew();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_store_helpers::validate_segment_store_registers::<Reg, _, _>(
                    program_counter,
                    vs3,
                    data_group_regs,
                    nf,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match across all fields
                if index_eew != data_eew {
                    zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                        program_counter,
                        vs2,
                        index_group_regs.get(),
                        vs3,
                        nf.fields_per_segment() * data_group_regs.get(),
                    )?;
                }
                // SAFETY: identical precondition argument to `Vsuxseg`
                unsafe {
                    zvexx_store_helpers::execute_indexed_store(
                        env,
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

        ExecutionResult::ContinueNoWrite
    }
}
