//! ZveXx integer reduction instructions

#[cfg(test)]
mod tests;
pub mod zvexx_reduction_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::arith::zvexx_arith_helpers;
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
const impl<Reg> ExecutableInstructionOperands for ZveXxReductionInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for ZveXxReductionInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxReductionInstruction<Reg>
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
            rs1_value: _,
            rs2_value: _,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        _memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            Self::Vredsum { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, _sew| acc.wrapping_add(elem),
                );
            }
            Self::Vredand { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, _sew| acc & elem,
                );
            }
            Self::Vredor { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, _sew| acc | elem,
                );
            }
            Self::Vredxor { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, _sew| acc ^ elem,
                );
            }
            Self::Vredminu { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        if elem & mask < acc & mask { elem } else { acc }
                    },
                );
            }
            Self::Vredmin { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, sew| {
                        if zvexx_arith_helpers::sign_extend(elem, sew)
                            < zvexx_arith_helpers::sign_extend(acc, sew)
                        {
                            elem
                        } else {
                            acc
                        }
                    },
                );
            }
            Self::Vredmaxu { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, sew| {
                        let mask = zvexx_arith_helpers::sew_mask(sew);
                        if elem & mask > acc & mask { elem } else { acc }
                    },
                );
            }
            Self::Vredmax { vd, vs2, vs1, vm } => {
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
                zvexx_reduction_helpers::execute_reduce_op(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    sew,
                    |acc, elem, sew| {
                        if zvexx_arith_helpers::sign_extend(elem, sew)
                            > zvexx_arith_helpers::sign_extend(acc, sew)
                        {
                            elem
                        } else {
                            acc
                        }
                    },
                );
            }
            Self::Vwredsumu { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) =
                    zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(config.vtype().vsew())
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs1` is read with EEW=2*SEW and `vs2` with EEW=SEW
                zvexx_helpers::check_register_outside_group::<Reg, Env, _, _>(
                    program_counter,
                    vs2,
                    vs1,
                )?;
                zvexx_reduction_helpers::execute_widening_reduce_op::<false, _, _, _>(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    widening_sew,
                    // Zero-extend vs2 elements then accumulate
                    |acc, elem, _sew| acc.wrapping_add(elem),
                );
            }
            Self::Vwredsum { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) =
                    zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(config.vtype().vsew())
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let vs2 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs2,
                    widening_sew.narrow().as_eew(),
                )?;
                // `vs1` is read with EEW=2*SEW and `vs2` with EEW=SEW
                zvexx_helpers::check_register_outside_group::<Reg, Env, _, _>(
                    program_counter,
                    vs2,
                    vs1,
                )?;
                zvexx_reduction_helpers::execute_widening_reduce_op::<true, _, _, _>(
                    env,
                    vd,
                    vs2,
                    vs1,
                    vm,
                    widening_sew,
                    // Sign-extend vs2 elements then accumulate
                    |acc, elem, _sew| acc.wrapping_add(elem),
                );
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
