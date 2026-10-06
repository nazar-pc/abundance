//! ZveXx fixed-point arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zvexx_fixed_point_helpers;

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
const impl<Reg, Hart> ExecutableInstructionOperands for ZveXxFixedPointInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for ZveXxFixedPointInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxFixedPointInstruction<Hart>
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
            // vsaddu.vv / vsaddu.vx / vsaddu.vi - saturating unsigned add
            Self::VsadduVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_addu(a, b, sew, vxsat),
                );
            }
            Self::VsadduVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_addu(a, b, sew, vxsat),
                );
            }
            Self::VsadduVi { vd, vs2, imm, vm } => {
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
                // Per v-spec §12.1 / §11.1: the 5-bit immediate is sign-extended to SEW,
                // then interpreted as an unsigned SEW-wide value for the saturating add.
                // Sign-extend i8 -> i64 -> bit-cast to u64; sat_addu masks to SEW internally.
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_addu(a, b, sew, vxsat),
                );
            }
            // vsadd.vv / vsadd.vx / vsadd.vi - saturating signed add
            Self::VsaddVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_add(a, b, sew, vxsat),
                );
            }
            Self::VsaddVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_add(a, b, sew, vxsat),
                );
            }
            Self::VsaddVi { vd, vs2, imm, vm } => {
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
                // Sign-extend 5-bit immediate for signed sat add
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_add(a, b, sew, vxsat),
                );
            }
            // vssubu.vv / vssubu.vx - saturating unsigned subtract
            Self::VssubuVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_subu(a, b, sew, vxsat),
                );
            }
            Self::VssubuVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_subu(a, b, sew, vxsat),
                );
            }
            // vssub.vv / vssub.vx - saturating signed subtract
            Self::VssubVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_sub(a, b, sew, vxsat),
                );
            }
            Self::VssubVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, _vxrm, vxsat| zvexx_fixed_point_helpers::sat_sub(a, b, sew, vxsat),
                );
            }
            // vaaddu.vv / vaaddu.vx - averaging unsigned add
            Self::VaadduVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_addu(a, b, sew, vxrm),
                );
            }
            Self::VaadduVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_addu(a, b, sew, vxrm),
                );
            }
            // vaadd.vv / vaadd.vx - averaging signed add
            Self::VaaddVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_add(a, b, sew, vxrm),
                );
            }
            Self::VaaddVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_add(a, b, sew, vxrm),
                );
            }
            // vasubu.vv / vasubu.vx - averaging unsigned subtract
            Self::VasubuVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_subu(a, b, sew, vxrm),
                );
            }
            Self::VasubuVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_subu(a, b, sew, vxrm),
                );
            }
            // vasub.vv / vasub.vx - averaging signed subtract
            Self::VasubVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_sub(a, b, sew, vxrm),
                );
            }
            Self::VasubVx {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| zvexx_fixed_point_helpers::avg_sub(a, b, sew, vxrm),
                );
            }
            // vsmul.vv / vsmul.vx - fractional multiply with rounding and saturation
            Self::VsmulVv { vd, vs2, vs1, vm } => {
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
                // Not supported for SEW=64 in Zve64x (would need 128-bit result)
                if !Self::implements_extension::<V<_>>() && sew == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::smul(a, b, sew, vxrm, vxsat)
                    },
                );
            }
            Self::VsmulVx {
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
                // Not supported for SEW=64 in Zve64x (would need 128-bit result)
                if !Self::implements_extension::<V<_>>() && sew == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
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
                let scalar = rs1_value.as_u64();
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::smul(a, b, sew, vxrm, vxsat)
                    },
                );
            }
            // vssrl.vv / vssrl.vx / vssrl.vi - scaling shift right logical
            Self::VssrlVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| {
                        // Shift amount masked to log2(SEW) bits per spec §12.7
                        let shamt = (b & u64::from(sew.bits_width() - 1)) as u32;
                        let masked_a = a & zvexx_fixed_point_helpers::sew_mask(sew);
                        zvexx_fixed_point_helpers::rounded_srl(masked_a, shamt, vxrm)
                            & zvexx_fixed_point_helpers::sew_mask(sew)
                    },
                );
            }
            Self::VssrlVx {
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
                let scalar = rs1_value.as_u64();
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| {
                        let shamt = (b & u64::from(sew.bits_width() - 1)) as u32;
                        let masked_a = a & zvexx_fixed_point_helpers::sew_mask(sew);
                        zvexx_fixed_point_helpers::rounded_srl(masked_a, shamt, vxrm)
                            & zvexx_fixed_point_helpers::sew_mask(sew)
                    },
                );
            }
            Self::VssrlVi { vd, vs2, imm, vm } => {
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
                // Immediate is unsigned 5-bit; mask to log2(SEW) here too
                let shamt = (u64::from(imm) & u64::from(sew.bits_width() - 1)) as u32;
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(u64::from(shamt)),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| {
                        let shamt = b as u32;
                        let masked_a = a & zvexx_fixed_point_helpers::sew_mask(sew);
                        zvexx_fixed_point_helpers::rounded_srl(masked_a, shamt, vxrm)
                            & zvexx_fixed_point_helpers::sew_mask(sew)
                    },
                );
            }
            // vssra.vv / vssra.vx / vssra.vi - scaling shift right arithmetic
            Self::VssraVv { vd, vs2, vs1, vm } => {
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
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| {
                        let shamt = (b & u64::from(sew.bits_width() - 1)) as u32;
                        zvexx_fixed_point_helpers::rounded_sra(a, shamt, vxrm, sew)
                            & zvexx_fixed_point_helpers::sew_mask(sew)
                    },
                );
            }
            Self::VssraVx {
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
                let scalar = rs1_value.as_u64();
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| {
                        let shamt = (b & u64::from(sew.bits_width() - 1)) as u32;
                        zvexx_fixed_point_helpers::rounded_sra(a, shamt, vxrm, sew)
                            & zvexx_fixed_point_helpers::sew_mask(sew)
                    },
                );
            }
            Self::VssraVi { vd, vs2, imm, vm } => {
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
                let shamt = (u64::from(imm) & u64::from(sew.bits_width() - 1)) as u32;
                zvexx_fixed_point_helpers::execute_fixed_point_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(u64::from(shamt)),
                    vm,
                    sew,
                    |a, b, sew, vxrm, _vxsat| {
                        let shamt = b as u32;
                        zvexx_fixed_point_helpers::rounded_sra(a, shamt, vxrm, sew)
                            & zvexx_fixed_point_helpers::sew_mask(sew)
                    },
                );
            }
            // vnclipu.wv / vnclipu.wx / vnclipu.wi - narrowing unsigned clip
            Self::VnclipuWv { vd, vs2, vs1, vm } => {
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
                // The source is `2*SEW` wide, which must not exceed `ELEN`
                let widening_sew =
                    zvexx_fixed_point_helpers::check_narrowing_sew::<{ Env::ELEN }, Reg, _, _>(
                        program_counter,
                        sew,
                    )?;
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    sew.as_eew(),
                )?;
                // vs2 holds 2*SEW elements; its register group is double-width
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                // vs1 is a normal SEW-wide source for the shift amount
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    sew.as_eew(),
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                zvexx_fixed_point_helpers::execute_narrowing_clip_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    |wide, shamt, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::nclipu(wide, shamt, sew, vxrm, vxsat)
                    },
                );
            }
            Self::VnclipuWx {
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
                // The source is `2*SEW` wide, which must not exceed `ELEN`
                let widening_sew =
                    zvexx_fixed_point_helpers::check_narrowing_sew::<{ Env::ELEN }, Reg, _, _>(
                        program_counter,
                        sew,
                    )?;
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
                    widening_sew.wide().as_eew(),
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_u64();
                zvexx_fixed_point_helpers::execute_narrowing_clip_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    |wide, shamt, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::nclipu(wide, shamt, sew, vxrm, vxsat)
                    },
                );
            }
            Self::VnclipuWi { vd, vs2, imm, vm } => {
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
                // The source is `2*SEW` wide, which must not exceed `ELEN`
                let widening_sew =
                    zvexx_fixed_point_helpers::check_narrowing_sew::<{ Env::ELEN }, Reg, _, _>(
                        program_counter,
                        sew,
                    )?;
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
                    widening_sew.wide().as_eew(),
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_fixed_point_helpers::execute_narrowing_clip_op(
                    env,
                    vd,
                    vs2,
                    // Immediate is the shift amount directly; masking done inside the helper
                    zvexx_fixed_point_helpers::OpSrc::Scalar(u64::from(imm)),
                    vm,
                    widening_sew,
                    |wide, shamt, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::nclipu(wide, shamt, sew, vxrm, vxsat)
                    },
                );
            }
            // vnclip.wv / vnclip.wx / vnclip.wi - narrowing signed clip
            Self::VnclipWv { vd, vs2, vs1, vm } => {
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
                // The source is `2*SEW` wide, which must not exceed `ELEN`
                let widening_sew =
                    zvexx_fixed_point_helpers::check_narrowing_sew::<{ Env::ELEN }, Reg, _, _>(
                        program_counter,
                        sew,
                    )?;
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    sew.as_eew(),
                )?;
                // vs2 holds 2*SEW elements; its register group is double-width
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    sew.as_eew(),
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                zvexx_fixed_point_helpers::execute_narrowing_clip_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    |wide, shamt, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::nclip(wide, shamt, sew, vxrm, vxsat)
                    },
                );
            }
            Self::VnclipWx {
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
                // The source is `2*SEW` wide, which must not exceed `ELEN`
                let widening_sew =
                    zvexx_fixed_point_helpers::check_narrowing_sew::<{ Env::ELEN }, Reg, _, _>(
                        program_counter,
                        sew,
                    )?;
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
                    widening_sew.wide().as_eew(),
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_u64();
                zvexx_fixed_point_helpers::execute_narrowing_clip_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    |wide, shamt, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::nclip(wide, shamt, sew, vxrm, vxsat)
                    },
                );
            }
            Self::VnclipWi { vd, vs2, imm, vm } => {
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
                // The source is `2*SEW` wide, which must not exceed `ELEN`
                let widening_sew =
                    zvexx_fixed_point_helpers::check_narrowing_sew::<{ Env::ELEN }, Reg, _, _>(
                        program_counter,
                        sew,
                    )?;
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
                    widening_sew.wide().as_eew(),
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_fixed_point_helpers::execute_narrowing_clip_op(
                    env,
                    vd,
                    vs2,
                    zvexx_fixed_point_helpers::OpSrc::Scalar(u64::from(imm)),
                    vm,
                    widening_sew,
                    |wide, shamt, sew, vxrm, vxsat| {
                        zvexx_fixed_point_helpers::nclip(wide, shamt, sew, vxrm, vxsat)
                    },
                );
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
