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
                let Some(vs3) = zvexx_load_helpers::VRegGroup::whole_registers(vs3, nreg, Eew::E8)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // `evl = NREG * VLEN / 8` elements with `EEW = 8`, regardless of `vtype` and `vl`
                let vl = vs3.vl().get();
                // `vstart >= evl` is reserved
                if env.vstart() >= vl {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_store_helpers::execute_unit_stride_store(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vs3),
                    true,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // `evl = ceil(vl / 8)` elements with `EEW = 8`, which never exceeds a single
                // register
                let Some(vs3) =
                    zvexx_load_helpers::VRegGroup::whole_registers(vs3, VRegGroupSize::R1, Eew::E8)
                        .and_then(|vs3| vs3.with_vl(Vl::from(config.vl().bytes())))
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                zvexx_store_helpers::execute_unit_stride_store(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vs3),
                    true,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vs3 =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vs3, eew)?;
                zvexx_store_helpers::execute_unit_stride_store(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vs3),
                    vm,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vs3 =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vs3, eew)?;
                let stride = rs2_value.as_i64();
                zvexx_store_helpers::execute_strided_store(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vs3),
                    vm,
                    rs1_value.as_u64(),
                    stride,
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = config.vtype().vsew().as_eew();
                let vs3 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs3,
                    data_eew,
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match
                if index_eew != data_eew {
                    zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(
                        program_counter,
                        vs2,
                        vs3,
                    )?;
                }
                zvexx_store_helpers::execute_indexed_store(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vs3),
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = config.vtype().vsew().as_eew();
                let vs3 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs3,
                    data_eew,
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match
                if index_eew != data_eew {
                    zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(
                        program_counter,
                        vs2,
                        vs3,
                    )?;
                }
                zvexx_store_helpers::execute_indexed_store(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vs3),
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vs3 =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vs3, eew)?;
                let vs3 =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vs3, nf)?;
                zvexx_store_helpers::execute_unit_stride_store(
                    env,
                    memory,
                    vs3,
                    vm,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vs3 =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vs3, eew)?;
                let vs3 =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vs3, nf)?;
                let stride = rs2_value.as_i64();
                zvexx_store_helpers::execute_strided_store(
                    env,
                    memory,
                    vs3,
                    vm,
                    rs1_value.as_u64(),
                    stride,
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = config.vtype().vsew().as_eew();
                let vs3 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs3,
                    data_eew,
                )?;
                let vs3 =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vs3, nf)?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match across all fields
                if index_eew != data_eew && vs3.fields().any(|field| field.overlaps(vs2)) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_store_helpers::execute_indexed_store(
                    env,
                    memory,
                    vs3,
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
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
                let Some(config) = env.vector_config() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let data_eew = config.vtype().vsew().as_eew();
                let vs3 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs3,
                    data_eew,
                )?;
                let vs3 =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vs3, nf)?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // The index `vs2` and the data `vs3` are both sources, with different EEWs unless
                // they happen to match across all fields
                if index_eew != data_eew && vs3.fields().any(|field| field.overlaps(vs2)) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_store_helpers::execute_indexed_store(
                    env,
                    memory,
                    vs3,
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
