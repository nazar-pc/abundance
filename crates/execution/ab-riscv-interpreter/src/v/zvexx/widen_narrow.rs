//! ZveXx widening, narrowing, and extension instructions

#[cfg(test)]
mod tests;
pub mod zvexx_widen_narrow_helpers;

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
const impl<Reg> ExecutableInstructionOperands for ZveXxWidenNarrowInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for ZveXxWidenNarrowInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxWidenNarrowInstruction<Reg>
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
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            // vwaddu.vv - 2*SEW = zext(SEW) + zext(SEW)
            Self::VwadduVv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwaddu.vx - 2*SEW = zext(SEW) + zext(xlen->SEW)
            Self::VwadduVx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                // Scalar is zero-extended to 2*SEW; the low SEW bits are what matter
                let scalar = rs1_value.as_u64();
                zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwadd.vv - 2*SEW = sext(SEW) + sext(SEW)
            Self::VwaddVv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwadd.vx - 2*SEW = sext(SEW) + sext(rs1)
            Self::VwaddVx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                // Scalar is sign-extended from XLEN to 64 bits
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwsubu.vv - 2*SEW = zext(SEW) - zext(SEW)
            Self::VwsubuVv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwsubu.vx - 2*SEW = zext(SEW) - zext(rs1)
            Self::VwsubuVx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_u64();
                zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwsub.vv - 2*SEW = sext(SEW) - sext(SEW)
            Self::VwsubVv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwsub.vx - 2*SEW = sext(SEW) - sext(rs1)
            Self::VwsubVx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwaddu.wv - 2*SEW = 2*SEW + zext(SEW)
            Self::VwadduWv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs2` and `vs1` are read with different EEWs
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                // The wide destination may only overlap the narrow source in its highest-numbered
                // part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwaddu.wx - 2*SEW = 2*SEW + zext(rs1)
            Self::VwadduWx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let scalar = rs1_value.as_u64();
                zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwadd.wv - 2*SEW = 2*SEW + sext(SEW)
            Self::VwaddWv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs2` and `vs1` are read with different EEWs
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                // The wide destination may only overlap the narrow source in its highest-numbered
                // part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwadd.wx - 2*SEW = 2*SEW + sext(rs1)
            Self::VwaddWx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_add,
                );
            }
            // vwsubu.wv - 2*SEW = 2*SEW - zext(SEW)
            Self::VwsubuWv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs2` and `vs1` are read with different EEWs
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                // The wide destination may only overlap the narrow source in its highest-numbered
                // part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwsubu.wx - 2*SEW = 2*SEW - zext(rs1)
            Self::VwsubuWx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let scalar = rs1_value.as_u64();
                zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwsub.wv - 2*SEW = 2*SEW - sext(SEW)
            Self::VwsubWv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs2` and `vs1` are read with different EEWs
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                // The wide destination may only overlap the narrow source in its highest-numbered
                // part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs1,
                )?;
                zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vwsub.wx - 2*SEW = 2*SEW - sext(rs1)
            Self::VwsubWx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.wide().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.wide().as_eew(),
                )?;
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                    u64::wrapping_sub,
                );
            }
            // vnsrl.wv - SEW = (2*SEW) >> SEW (logical)
            Self::VnsrlWv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.narrow().as_eew(),
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
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs2` and `vs1` are read with different EEWs
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                );
            }
            // vnsrl.wx - SEW = (2*SEW) >> rs1 (logical)
            Self::VnsrlWx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.narrow().as_eew(),
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
                zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                );
            }
            // vnsrl.wi - SEW = (2*SEW) >> uimm (logical)
            Self::VnsrlWi { vd, vs2, uimm, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.narrow().as_eew(),
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
                zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                    vm,
                    widening_sew,
                );
            }
            // vnsra.wv - SEW = (2*SEW) >> SEW (arithmetic)
            Self::VnsraWv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.narrow().as_eew(),
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
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs2` and `vs1` are read with different EEWs
                zvexx_helpers::check_groups_disjoint::<Reg, Env, _, _>(program_counter, vs2, vs1)?;
                zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                    vm,
                    widening_sew,
                );
            }
            // vnsra.wx - SEW = (2*SEW) >> rs1 (arithmetic)
            Self::VnsraWx {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.narrow().as_eew(),
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
                zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                    vm,
                    widening_sew,
                );
            }
            // vnsra.wi - SEW = (2*SEW) >> uimm (arithmetic)
            Self::VnsraWi { vd, vs2, uimm, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    widening_sew.narrow().as_eew(),
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
                zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _>(
                    env,
                    vd,
                    vs2,
                    zvexx_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                    vm,
                    widening_sew,
                );
            }
            // vzext.vf2 - zero-extend SEW/2 -> SEW
            Self::VzextVf2 { vd, vs2, vm } => {
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
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F2)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    extension_sew.dest().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    extension_sew.source().as_eew(),
                )?;
                // The destination may only overlap the narrow source in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_widen_narrow_helpers::execute_extension::<false, _, _>(
                    env,
                    vd,
                    vs2,
                    vm,
                    extension_sew,
                );
            }
            // vzext.vf4 - zero-extend SEW/4 -> SEW
            Self::VzextVf4 { vd, vs2, vm } => {
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
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F4)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    extension_sew.dest().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    extension_sew.source().as_eew(),
                )?;
                // The destination may only overlap the narrow source in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_widen_narrow_helpers::execute_extension::<false, _, _>(
                    env,
                    vd,
                    vs2,
                    vm,
                    extension_sew,
                );
            }
            // vzext.vf8 - zero-extend SEW/8 -> SEW
            Self::VzextVf8 { vd, vs2, vm } => {
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
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F8)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    extension_sew.dest().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    extension_sew.source().as_eew(),
                )?;
                // The destination may only overlap the narrow source in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_widen_narrow_helpers::execute_extension::<false, _, _>(
                    env,
                    vd,
                    vs2,
                    vm,
                    extension_sew,
                );
            }
            // vsext.vf2 - sign-extend SEW/2 -> SEW
            Self::VsextVf2 { vd, vs2, vm } => {
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
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F2)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    extension_sew.dest().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    extension_sew.source().as_eew(),
                )?;
                // The destination may only overlap the narrow source in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_widen_narrow_helpers::execute_extension::<true, _, _>(
                    env,
                    vd,
                    vs2,
                    vm,
                    extension_sew,
                );
            }
            // vsext.vf4 - sign-extend SEW/4 -> SEW
            Self::VsextVf4 { vd, vs2, vm } => {
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
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F4)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    extension_sew.dest().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    extension_sew.source().as_eew(),
                )?;
                // The destination may only overlap the narrow source in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_widen_narrow_helpers::execute_extension::<true, _, _>(
                    env,
                    vd,
                    vs2,
                    vm,
                    extension_sew,
                );
            }
            // vsext.vf8 - sign-extend SEW/8 -> SEW
            Self::VsextVf8 { vd, vs2, vm } => {
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
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F8)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vd = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vd,
                    extension_sew.dest().as_eew(),
                )?;
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    extension_sew.source().as_eew(),
                )?;
                // The destination may only overlap the narrow source in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvexx_widen_narrow_helpers::execute_extension::<true, _, _>(
                    env,
                    vd,
                    vs2,
                    vm,
                    extension_sew,
                );
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
