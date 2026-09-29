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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                // Widening requires SEW < 64; 2*SEW must fit in ELEN=64
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // Scalar is zero-extended to 2*SEW; the low SEW bits are what matter
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                // Scalar is sign-extended from XLEN to 64 bits
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    Some(vs1),
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                // vs2 is the wide source; vs1 is narrow
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs.get(),
                    vs1,
                    group_regs.get(),
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                // For .wx scalar variants vd may alias vs2 (same wide group); no narrow vs1
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs.get(),
                    vs1,
                    group_regs.get(),
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_add,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs.get(),
                    vs1,
                    group_regs.get(),
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<true, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs.get(),
                    vs1,
                    group_regs.get(),
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    None,
                    group_regs,
                    wide_group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vd_widen_no_src_check::<Reg, _, _>(
                    program_counter,
                    vd,
                    wide_group_regs,
                )?;
                let scalar = zvexx_widen_narrow_helpers::sign_extend_bits(
                    rs1_value.as_u64(),
                    Vsew::from_xlen::<Reg>(),
                )
                .cast_unsigned();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_widen_w_op::<false, _, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                        u64::wrapping_sub,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                // SEW must be < 64 so that 2*SEW fits in ELEN
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs.get(),
                    vs1,
                    group_regs.get(),
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<false, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                        vm,
                        widening_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                // `vs2` is read with EEW=2*SEW and `vs1` with EEW=SEW
                zvexx_helpers::check_sources_disjoint::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs.get(),
                    vs1,
                    group_regs.get(),
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Vreg(vs1),
                        vm,
                        widening_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(scalar),
                        vm,
                        widening_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let group_regs = vtype.vlmul().register_count();
                let Some(widening_sew) = zvexx_helpers::WideningSew::<{ Env::ELEN }>::new(sew)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let wide_eew = Eew::from(widening_sew.wide());
                let wide_group_regs = vtype.vlmul().data_register_count(wide_eew, sew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_widen_narrow_helpers::check_vd_narrow_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                    vs2,
                    wide_group_regs,
                )?;
                zvexx_widen_narrow_helpers::check_vs_wide_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    wide_group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_narrow_shift::<true, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        zvexx_widen_narrow_helpers::OpSrc::Scalar(u64::from(uimm)),
                        vm,
                        widening_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F2)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    vd,
                    group_regs,
                    VsewFactor::F2,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<false, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        vm,
                        extension_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F4)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    vd,
                    group_regs,
                    VsewFactor::F4,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<false, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        vm,
                        extension_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F8)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    vd,
                    group_regs,
                    VsewFactor::F8,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<false, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        vm,
                        extension_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F2)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    vd,
                    group_regs,
                    VsewFactor::F2,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<true, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        vm,
                        extension_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F4)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    vd,
                    group_regs,
                    VsewFactor::F4,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<true, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        vm,
                        extension_sew,
                    );
                }
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
                let vtype = config.vtype();
                let sew = vtype.vsew();
                let Some(extension_sew) = zvexx_helpers::ExtensionSew::new(sew, VsewFactor::F8)
                else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_widen_narrow_helpers::check_vs_ext_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    vd,
                    group_regs,
                    VsewFactor::F8,
                )?;
                zvexx_widen_narrow_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                // SAFETY: alignment/overlap/SEW checked above
                unsafe {
                    zvexx_widen_narrow_helpers::execute_extension::<true, _, _>(
                        env,
                        config,
                        vd,
                        vs2,
                        vm,
                        extension_sew,
                    );
                }
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
