//! ZveXx integer arithmetic instructions

#[cfg(test)]
mod tests;
pub mod zvexx_arith_helpers;

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
const impl<Reg, Hart> ExecutableInstructionOperands for ZveXxArithInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for ZveXxArithInstruction<Hart>
where
    Reg: Register,
    Hart: HartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxArithInstruction<Hart>
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
            // vadd
            Self::VaddVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, _| a.wrapping_add(b),
                );
            }
            Self::VaddVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a.wrapping_add(b),
                );
            }
            Self::VaddVi { vd, vs2, imm, vm } => {
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
                // Sign-extend imm to u64 so wrapping_add works correctly for all SEW
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a.wrapping_add(b),
                );
            }
            // vsub / vrsub
            Self::VsubVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, _| a.wrapping_sub(b),
                );
            }
            Self::VsubVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a.wrapping_sub(b),
                );
            }
            Self::VrsubVx {
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
                // vrsub: result = src - vs2[i]
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| b.wrapping_sub(a),
                );
            }
            Self::VrsubVi { vd, vs2, imm, vm } => {
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
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| b.wrapping_sub(a),
                );
            }
            // vand
            Self::VandVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, _| a & b,
                );
            }
            Self::VandVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a & b,
                );
            }
            Self::VandVi { vd, vs2, imm, vm } => {
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
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a & b,
                );
            }
            // vor
            Self::VorVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, _| a | b,
                );
            }
            Self::VorVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a | b,
                );
            }
            Self::VorVi { vd, vs2, imm, vm } => {
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
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a | b,
                );
            }
            // vxor
            Self::VxorVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, _| a ^ b,
                );
            }
            Self::VxorVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a ^ b,
                );
            }
            Self::VxorVi { vd, vs2, imm, vm } => {
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
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, _| a ^ b,
                );
            }
            // vsll
            Self::VsllVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    // Shift amount masked to log2(SEW) bits per spec §12.6
                    |a, b, sew| a << (b & u64::from(sew.bits_width() - 1)),
                );
            }
            Self::VsllVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| a << (b & u64::from(sew.bits_width() - 1)),
                );
            }
            Self::VsllVi { vd, vs2, uimm, vm } => {
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
                // Immediate is already unsigned 5-bit; mask to log2(SEW) here too
                let shamt = u64::from(uimm) & u64::from(sew.bits_width() - 1);
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(shamt),
                    vm,
                    sew,
                    |a, b, _| a << b,
                );
            }
            // vsrl
            Self::VsrlVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    // Logical right shift; operate on the SEW-wide portion only
                    |a, b, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        let shamt = b & u64::from(sew.bits_width() - 1);
                        (a & mask) >> shamt
                    },
                );
            }
            Self::VsrlVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        let shamt = b & u64::from(sew.bits_width() - 1);
                        (a & mask) >> shamt
                    },
                );
            }
            Self::VsrlVi { vd, vs2, uimm, vm } => {
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
                let shamt = u64::from(uimm) & u64::from(sew.bits_width() - 1);
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(shamt),
                    vm,
                    sew,
                    |a, b, sew| (a & zvexx_arith_helpers::sew_mask(sew)) >> b,
                );
            }
            // vsra
            Self::VsraVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        let shamt = b & u64::from(sew.bits_width() - 1);
                        let signed = zvexx_arith_helpers::sign_extend(a, sew);
                        (signed >> shamt).cast_unsigned()
                    },
                );
            }
            Self::VsraVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        let shamt = b & u64::from(sew.bits_width() - 1);
                        let signed = zvexx_arith_helpers::sign_extend(a, sew);
                        (signed >> shamt).cast_unsigned()
                    },
                );
            }
            Self::VsraVi { vd, vs2, uimm, vm } => {
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
                let shamt = u64::from(uimm) & u64::from(sew.bits_width() - 1);
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(shamt),
                    vm,
                    sew,
                    |a, b, sew| {
                        let signed = zvexx_arith_helpers::sign_extend(a, sew);
                        (signed >> b).cast_unsigned()
                    },
                );
            }
            // vminu / vmin
            Self::VminuVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        if a & mask <= b & mask { a } else { b }
                    },
                );
            }
            Self::VminuVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        if a & mask <= b & mask { a } else { b }
                    },
                );
            }
            Self::VminVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        if zvexx_arith_helpers::sign_extend(a, sew)
                            <= zvexx_arith_helpers::sign_extend(b, sew)
                        {
                            a
                        } else {
                            b
                        }
                    },
                );
            }
            Self::VminVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        if zvexx_arith_helpers::sign_extend(a, sew)
                            <= zvexx_arith_helpers::sign_extend(b, sew)
                        {
                            a
                        } else {
                            b
                        }
                    },
                );
            }
            // vmaxu / vmax
            Self::VmaxuVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        if a & mask >= b & mask { a } else { b }
                    },
                );
            }
            Self::VmaxuVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        if a & mask >= b & mask { a } else { b }
                    },
                );
            }
            Self::VmaxVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        if zvexx_arith_helpers::sign_extend(a, sew)
                            >= zvexx_arith_helpers::sign_extend(b, sew)
                        {
                            a
                        } else {
                            b
                        }
                    },
                );
            }
            Self::VmaxVx {
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
                zvexx_arith_helpers::execute_arith_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        if zvexx_arith_helpers::sign_extend(a, sew)
                            >= zvexx_arith_helpers::sign_extend(b, sew)
                        {
                            a
                        } else {
                            b
                        }
                    },
                );
            }
            // vmseq
            Self::VmseqVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            == (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmseqVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            == (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmseqVi { vd, vs2, imm, vm } => {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            == (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            // vmsne
            Self::VmsneVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            != (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmsneVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            != (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmsneVi { vd, vs2, imm, vm } => {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            != (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            // vmsltu (unsigned <)
            Self::VmsltuVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            < (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmsltuVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            < (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            // vmslt (signed <)
            Self::VmsltVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            < zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
            Self::VmsltVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            < zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
            // vmsleu (unsigned <=)
            Self::VmsleuVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            <= (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmsleuVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            <= (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmsleuVi { vd, vs2, imm, vm } => {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                // Per spec §12.8: for vmsleu.vi, the immediate is sign-extended to XLEN
                // then the comparison is unsigned. A negative i8 immediate sign-extends to
                // a large u64 (e.g. -1 -> 0xFFFF...FF). Both operands are masked to SEW
                // before comparing, so the effective immediate is (0xFFFF...FF &
                // zve64x_arith_helpers::sew_mask), which equals
                // zve64x_arith_helpers::sew_mask (the maximum SEW-wide unsigned value). This means
                // vs2[i] <= imm is always true for SEW < XLEN when imm < 0.
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            <= (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            // vmsle (signed <=)
            Self::VmsleVv { vd, vs2, vs1, vm } => {
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
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Vreg(vs1),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            <= zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
            Self::VmsleVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            <= zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
            Self::VmsleVi { vd, vs2, imm, vm } => {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            <= zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
            // vmsgtu (unsigned >): no vv form; vx and vi only
            Self::VmsgtuVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            > (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            Self::VmsgtuVi { vd, vs2, imm, vm } => {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        (a & zvexx_arith_helpers::sew_mask(sew))
                            > (b & zvexx_arith_helpers::sew_mask(sew))
                    },
                );
            }
            // vmsgt (signed >): no vv form; vx and vi only
            Self::VmsgtVx {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            > zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
            Self::VmsgtVi { vd, vs2, imm, vm } => {
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
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    sew.as_eew(),
                )?;
                zvexx_arith_helpers::check_mask_dest_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = i64::from(imm).cast_unsigned();
                zvexx_arith_helpers::execute_compare_op(
                    env,
                    vd,
                    vs2,
                    zvexx_arith_helpers::OpSrc::Scalar(scalar),
                    vm,
                    sew,
                    |a, b, sew| {
                        zvexx_arith_helpers::sign_extend(a, sew)
                            > zvexx_arith_helpers::sign_extend(b, sew)
                    },
                );
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
