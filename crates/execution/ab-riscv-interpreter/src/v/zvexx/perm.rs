//! ZveXx permutation instructions

#[cfg(test)]
mod tests;
pub mod zvexx_perm_helpers;

use crate::v::vector_registers::VectorRegistersExt;
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
const impl<Reg, Hart> ExecutableInstructionOperands for ZveXxPermInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for ZveXxPermInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxPermInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
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
            rs2_value: _,
        }: Rs1Rs2OperandValues<Reg::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Reg> {
        match self {
            // vmv.x.s rd, vs2
            // Copies sign-extended element 0 of vs2 (at current SEW) to GPR rd.
            // Requires valid vtype (needs SEW to know element width).
            // Does not use vl or masking; always reads element 0.
            Self::VmvXS { rd, vs2 } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let raw = env
                    .read_vregs()
                    .read_first(vs2, sew.as_eew())
                    .expect("SEW never exceeds `ELEN`, which never exceeds `VLEN`; qed");
                let sign_extended = zvexx_perm_helpers::sign_extend_to_reg::<Reg>(raw, sew);
                env.mark_vs_dirty();

                return ExecutionResult::Continue {
                    rd,
                    value: sign_extended,
                };
            }
            // vmv.s.x vd, rs1
            // Copies scalar GPR rs1 (zero-extended / truncated to SEW) into element 0 of vd.
            // When vl == 0, the write is suppressed.
            Self::VmvSX { vd, rs1: _ } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let vl = config.vl().get();
                // Per spec §16.1: update only when `vstart < vl`, and `vstart` is zero here
                if vl != Vl::ZERO {
                    let scalar = rs1_value.as_i64().cast_unsigned();
                    env.write_vregs()
                        .write_first(vd, sew.as_eew(), scalar)
                        .expect("SEW never exceeds `ELEN`, which never exceeds `VLEN`; qed");
                }
                env.mark_vs_dirty();
            }
            // vslideup.vx vd, vs2, rs1: _, vm
            // Slides elements of vs2 up by the scalar offset in rs1.
            // Elements vd[0..offset] are unchanged (tail-undisturbed for those positions).
            // Elements vd[i] for offset <= i < vl get vs2[i - offset].
            // Per spec §16.3.1: vd must not overlap vs2.
            Self::VslideupVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                // vd must not overlap vs2
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                let offset = rs1_value.as_u64();
                zvexx_perm_helpers::execute_slideup(env, vd, vs2, vm, offset);
            }
            // vslideup.vi vd, vs2, uimm, vm
            // Same as vslideup.vx but offset is a 5-bit unsigned immediate.
            Self::VslideupVi { vd, vs2, uimm, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                // vd must not overlap vs2
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                let offset = u64::from(uimm);
                zvexx_perm_helpers::execute_slideup(env, vd, vs2, vm, offset);
            }
            // vslidedown.vx vd, vs2, rs1: _, vm
            // Element vd[i] = vs2[i + offset] if i + offset < VLMAX, else 0.
            // vd may overlap vs2 for slidedown.
            Self::VslidedownVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                let offset = rs1_value.as_u64();
                zvexx_perm_helpers::execute_slidedown(env, vd, vs2, vm, offset);
            }
            // vslidedown.vi vd, vs2, uimm, vm
            // Same as vslidedown.vx but offset is a 5-bit unsigned immediate.
            Self::VslidedownVi { vd, vs2, uimm, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                let offset = u64::from(uimm);
                zvexx_perm_helpers::execute_slidedown(env, vd, vs2, vm, offset);
            }
            // vslide1up.vx vd, vs2, rs1: _, vm
            // Element 0 of vd gets the scalar value rs1 (written at SEW width).
            // Elements vd[i] for 1 <= i < vl get vs2[i - 1].
            // vd must not overlap vs2.
            Self::Vslide1upVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                // vd must not overlap vs2
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_perm_helpers::execute_slide1up(env, vd, vs2, vm, scalar);
            }
            // vslide1down.vx vd, vs2, rs1: _, vm
            // Element vd[i] = vs2[i + 1] for 0 <= i < vl - 1.
            // Element vd[vl - 1] gets the scalar value rs1.
            // vd may overlap vs2 for slide1down.
            Self::Vslide1downVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_perm_helpers::execute_slide1down(env, vd, vs2, vm, scalar);
            }
            // vrgather.vv vd, vs2, vs1, vm
            // vd[i] = (vs1[i] < VLMAX) ? vs2[vs1[i]] : 0
            // vd must not overlap vs1 or vs2.
            Self::VrgatherVv { vd, vs2, vs1, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    sew.as_eew(),
                )?;
                // vd must not overlap either source
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs1)?;
                zvexx_perm_helpers::execute_rgather_vv(env, vd, vs2, vs1, vm);
            }
            // vrgather.vx vd, vs2, rs1: _, vm
            // All active elements of vd get vs2[rs1] if rs1 < VLMAX, else 0.
            // vd must not overlap vs2.
            Self::VrgatherVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                // vd must not overlap vs2
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                let index = rs1_value.as_u64();
                zvexx_perm_helpers::execute_rgather_scalar(env, vd, vs2, vm, index);
            }
            // vrgather.vi vd, vs2, uimm, vm
            // Same as vrgather.vx but index is a 5-bit unsigned immediate.
            Self::VrgatherVi { vd, vs2, uimm, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                // vd must not overlap vs2
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                let index = u64::from(uimm);
                zvexx_perm_helpers::execute_rgather_scalar(env, vd, vs2, vm, index);
            }
            // vrgatherei16.vv vd, vs2, vs1, vm
            // Like vrgather.vv but vs1 always uses EEW=16 (regardless of SEW).
            // EMUL_vs1 = (16 / SEW) * LMUL; must be in [1/8, 8] else illegal.
            // vd must not overlap vs1 or vs2.
            Self::Vrgatherei16Vv { vd, vs2, vs1, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    Eew::E16,
                )?;
                // vd must not overlap either source
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs1)?;
                // `vs1` is read with EEW=16 and `vs2` with EEW=SEW
                if sew != Vsew::E16 {
                    zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(
                        program_counter,
                        vs2,
                        vs1,
                    )?;
                }
                zvexx_perm_helpers::execute_rgatherei16(env, vd, vs2, vs1, vm);
            }
            // vmerge.vvm / vmv.v.v
            // When vm=true: vmv.v.v vd, vs1 - broadcast all active elements from vs1.
            //   vs2 is ignored; no overlap restriction on vd/vs2.
            // When vm=false: vmerge.vvm vd, vs2, vs1, v0
            //   vd[i] = v0[i] ? vs1[i] : vs2[i]
            //   vd must not overlap v0 (mask source), which the decoder rejects.
            Self::VmergeVvm { vd, vs2, vs1, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    sew.as_eew(),
                )?;
                // Unmasked, this is `vmv.v.v`, for which the encoding fixes `vs2` to `v0`, which is
                // a valid register group that is not read
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_perm_helpers::execute_merge_vv(env, vd, vs2, vs1, vm);
            }
            // vmerge.vxm / vmv.v.x
            // When vm=true: vmv.v.x vd, rs1 - broadcast scalar to all active elements.
            // When vm=false: vmerge.vxm - vd[i] = v0[i] ? rs1 : vs2[i]
            Self::VmergeVxm {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                // Unmasked, this is `vmv.v.x`/`vmv.v.i`, for which the encoding fixes `vs2` to
                // `v0`, which is a valid register group that is not read
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_perm_helpers::execute_merge_scalar(env, vd, vs2, vm, scalar);
            }
            // vmerge.vim / vmv.v.i
            // When vm=true: vmv.v.i vd, simm5 - broadcast sign-extended immediate.
            // When vm=false: vmerge.vim - vd[i] = v0[i] ? simm5 : vs2[i]
            Self::VmergeVim { vd, vs2, simm5, vm } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                // Unmasked, this is `vmv.v.x`/`vmv.v.i`, for which the encoding fixes `vs2` to
                // `v0`, which is a valid register group that is not read
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                let scalar = i64::from(simm5).cast_unsigned();
                zvexx_perm_helpers::execute_merge_scalar(env, vd, vs2, vm, scalar);
            }
            // vcompress.vm vd, vs2, vs1
            // Packs active elements of vs2 (where vs1 mask bit is set) sequentially into vd.
            // Always unmasked (vm=1 in encoding); vs1 is the explicit mask operand.
            // vd must not overlap vs1 or vs2.
            Self::VcompressVm { vd, vs2, vs1 } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
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
                    sew.as_eew(),
                )?;
                // vd must not overlap vs2
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vd, vs2)?;
                // vs1 is a single mask register, which must not be one of the registers of vd
                zvexx_helpers::check_register_outside_group::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                // `vs1` is a mask with EEW=1 and `vs2` has EEW=SEW
                zvexx_helpers::check_register_outside_group::<Reg, Env, _, _>(
                    program_counter,
                    vs2,
                    vs1,
                )?;
                zvexx_perm_helpers::execute_compress(env, vd, vs2, vs1);
            }
            // vmv1r.v vd, vs2
            // Whole register move: copies 1 register.
            // No masking and no vl dependency, but `EEW = min(VLEN * EMUL, SEW)` depends on vtype,
            // so it is illegal with vill set.
            Self::Vmv1rV { vd, vs2 } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if env.vector_config().is_none() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if zvexx_perm_helpers::execute_whole_reg_move::<1, _>(env.write_vregs(), vd, vs2)
                    .is_none()
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                env.mark_vs_dirty();
            }
            // vmv2r.v vd, vs2
            // Whole register move: copies 2 registers.
            // vd and vs2 must be aligned to 2 (checked here per spec §17.6).
            Self::Vmv2rV { vd, vs2 } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if env.vector_config().is_none() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vd.to_bits().is_multiple_of(2) || !vs2.to_bits().is_multiple_of(2) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if zvexx_perm_helpers::execute_whole_reg_move::<2, _>(env.write_vregs(), vd, vs2)
                    .is_none()
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                env.mark_vs_dirty();
            }
            // vmv4r.v vd, vs2
            // Whole register move: copies 4 registers.
            Self::Vmv4rV { vd, vs2 } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if env.vector_config().is_none() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vd.to_bits().is_multiple_of(4) || !vs2.to_bits().is_multiple_of(4) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if zvexx_perm_helpers::execute_whole_reg_move::<4, _>(env.write_vregs(), vd, vs2)
                    .is_none()
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                env.mark_vs_dirty();
            }
            // vmv8r.v vd, vs2
            // Whole register move: copies 8 registers.
            Self::Vmv8rV { vd, vs2 } => {
                if !zvexx_helpers::non_memory_instruction_allowed::<Reg, _>(env) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if env.vector_config().is_none() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vd.to_bits().is_multiple_of(8) || !vs2.to_bits().is_multiple_of(8) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if zvexx_perm_helpers::execute_whole_reg_move::<8, _>(env.write_vregs(), vd, vs2)
                    .is_none()
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                env.mark_vs_dirty();
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
