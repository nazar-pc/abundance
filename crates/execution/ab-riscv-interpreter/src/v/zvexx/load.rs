//! ZveXx vector load instructions

#[cfg(test)]
pub(super) mod tests;
pub mod zvexx_load_helpers;

use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
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
const impl<Reg, Hart> ExecutableInstructionOperands for ZveXxLoadInstruction<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for ZveXxLoadInstruction<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxLoadInstruction<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
    Regs: RegisterFile<Reg>,
    Env: VectorRegistersExt<Hart = Hart>,
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
        }: Rs1Rs2OperandValues<Reg::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        match self {
            // Whole-register load: loads `nreg` consecutive registers starting at `vd` directly
            // from memory. `vd` must be aligned to `nreg`. Ignores vtype, vl, vstart, masking.
            Self::Vlr {
                vd,
                rs1: _,
                nreg,
                eew,
            } => {
                let nreg = nreg.num_registers();
                if !env.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vd) = zvexx_load_helpers::VRegGroup::whole_registers(vd, nreg, eew) else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // `evl = NREG * VLEN / EEW` elements, regardless of `vtype` and `vl`
                let vl = vd.vl().get();
                // `vstart >= evl` is reserved
                if env.vstart() >= vl {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_load_helpers::execute_unit_stride_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    true,
                    rs1_value.as_u64(),
                    None,
                )?;
            }

            // Mask load: loads ceil(vl / 8) bytes from base into vd with no masking applied.
            // Illegal with vill set, it depends on vtype indirectly through its constraints on vl.
            Self::Vlm { vd, rs1: _ } => {
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
                let Some(vd) =
                    zvexx_load_helpers::VRegGroup::whole_registers(vd, VRegGroupSize::R1, Eew::E8)
                        .and_then(|vd| vd.with_vl(Vl::from(config.vl().bytes())))
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                zvexx_load_helpers::execute_unit_stride_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    true,
                    rs1_value.as_u64(),
                    None,
                )?;
            }

            // Unit-stride load.
            //
            // Destination EMUL = EEW/SEW * LMUL, computed via `eew_register_count`. This
            // gives `group_regs` such that `VLMAX = group_regs * VLEN.bytes() / eew.bytes()`
            // matches the architectural `vl`.
            Self::Vle {
                vd,
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
                let vd =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vd, eew)?;
                zvexx_load_helpers::execute_unit_stride_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    vm,
                    rs1_value.as_u64(),
                    None,
                )?;
            }

            // Fault-only-first unit-stride load. Preconditions identical to `Vle`.
            Self::Vleff {
                vd,
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
                let vd =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vd, eew)?;
                zvexx_load_helpers::execute_unit_stride_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    vm,
                    rs1_value.as_u64(),
                    Some(config),
                )?;
            }

            // Strided load. Destination EMUL = EEW/SEW * LMUL as for unit-stride.
            Self::Vlse {
                vd,
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
                let vd =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vd, eew)?;
                // rs2 holds a signed stride; reinterpret the register value as signed
                let stride = rs2_value.as_i64();
                zvexx_load_helpers::execute_strided_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    vm,
                    rs1_value.as_u64(),
                    stride,
                )?;
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
                let sew = config.vtype().vsew();
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    sew.as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // Non-segment indexed loads permit `vd`/`vs2` overlap under the general
                // EEW-relative overlap rule (e.g. when the data and index EEW
                // match), unlike segment ones
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_load_helpers::execute_indexed_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
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
                let sew = config.vtype().vsew();
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    sew.as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // Non-segment indexed loads permit `vd`/`vs2` overlap under the general
                // EEW-relative overlap rule (e.g. when the data and index EEW
                // match), unlike segment ones
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_load_helpers::execute_indexed_load(
                    env,
                    memory,
                    zvexx_load_helpers::VRegSegmentGroup::single(vd),
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
            }

            // Unit-stride segment load. EMUL = EEW/SEW * LMUL per field group.
            Self::Vlseg {
                vd,
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
                let vd =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vd, eew)?;
                let vd =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vd, nf)?;
                zvexx_load_helpers::execute_unit_stride_load(
                    env,
                    memory,
                    vd,
                    vm,
                    rs1_value.as_u64(),
                    None,
                )?;
            }

            // Fault-only-first segment load. Preconditions identical to `Vlseg`.
            Self::Vlsegff {
                vd,
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
                let vd =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vd, eew)?;
                let vd =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vd, nf)?;
                zvexx_load_helpers::execute_unit_stride_load(
                    env,
                    memory,
                    vd,
                    vm,
                    rs1_value.as_u64(),
                    Some(config),
                )?;
            }

            // Strided segment load. EMUL = EEW/SEW * LMUL as for `Vlse`.
            Self::Vlsseg {
                vd,
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
                let vd =
                    zvexx_helpers::vreg_group::<Reg, Env, _, _>(program_counter, config, vd, eew)?;
                let vd =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vd, nf)?;
                let stride = rs2_value.as_i64();
                zvexx_load_helpers::execute_strided_load(
                    env,
                    memory,
                    vd,
                    vm,
                    rs1_value.as_u64(),
                    stride,
                )?;
            }

            // Indexed-unordered segment load
            Self::Vluxseg {
                vd,
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
                let sew = config.vtype().vsew();
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    sew.as_eew(),
                )?;
                let vd =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vd, nf)?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // No field may overlap the index register group
                if vd.fields().any(|field| field.overlaps(vs2)) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_load_helpers::execute_indexed_load(
                    env,
                    memory,
                    vd,
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
            }

            // Indexed-ordered segment load: functionally identical to `Vluxseg` for a software
            // interpreter
            Self::Vloxseg {
                vd,
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
                let sew = config.vtype().vsew();
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    sew.as_eew(),
                )?;
                let vd =
                    zvexx_helpers::vreg_segment_group::<Reg, Env, _, _>(program_counter, vd, nf)?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    index_eew,
                )?;
                // No field may overlap the index register group
                if vd.fields().any(|field| field.overlaps(vs2)) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_load_helpers::execute_indexed_load(
                    env,
                    memory,
                    vd,
                    vs2,
                    vm,
                    rs1_value.as_u64(),
                )?;
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
