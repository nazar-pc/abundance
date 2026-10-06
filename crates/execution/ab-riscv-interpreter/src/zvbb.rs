//! Zvbb extension

#[cfg(test)]
mod tests;
pub mod zvbb_helpers;
pub mod zvkb;

use crate::v::vector_registers::{VectorRegisters, VectorRegistersExt};
use crate::v::zvexx::arith::zvexx_arith_helpers;
use crate::v::zvexx::carry::zvexx_carry_helpers;
use crate::v::zvexx::config::zvexx_config_helpers;
use crate::v::zvexx::fixed_point::zvexx_fixed_point_helpers;
use crate::v::zvexx::load::zvexx_load_helpers;
use crate::v::zvexx::mask::zvexx_mask_helpers;
use crate::v::zvexx::muldiv::zvexx_muldiv_helpers;
use crate::v::zvexx::perm::zvexx_perm_helpers;
use crate::v::zvexx::reduction::zvexx_reduction_helpers;
use crate::v::zvexx::store::zvexx_store_helpers;
use crate::v::zvexx::widen_narrow::zvexx_widen_narrow_helpers;
use crate::v::zvexx::zvexx_helpers;
use crate::zicsr::zicsr_helpers;
use crate::zvbb::zvkb::zvkb_helpers;
use crate::{
    CsrError, Csrs, ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands,
    ExecutionError, ExecutionResult, FetchInstructionResult, InstructionFetcher,
    OpaqueThreadedExecutionResult, PackedAddress, ProgramCounter, RegisterFile,
    Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction, ThreadedExecutionResult,
    VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg, Hart> ExecutableInstructionOperands for ZvbbInstruction<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
{
}

#[instruction_execution]
const impl<Reg, Hart, Env> ExecutableInstructionCsr<Env> for ZvbbInstruction<Hart>
where
    Reg: Register,
    Hart: VectorHartConfig<Reg = Reg>,
{
}

#[instruction_execution]
impl<Reg, Hart, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZvbbInstruction<Hart>
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
            // vbrev: reverse all bits within each SEW-wide element
            Self::VbrevV { vd, vs2, vm } => {
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
                zvbb_helpers::execute_vbrev::<Reg, _>(env, vd, vs2, sew, vm);
            }
            // vclz: count leading zeros within each SEW-wide element; result in [0, SEW]
            Self::VclzV { vd, vs2, vm } => {
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
                zvbb_helpers::execute_vclz::<Reg, _>(env, vd, vs2, sew, vm);
            }
            // vctz: count trailing zeros within each SEW-wide element; result in [0, SEW]
            Self::VctzV { vd, vs2, vm } => {
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
                zvbb_helpers::execute_vctz::<Reg, _>(env, vd, vs2, sew, vm);
            }
            // vcpop: population count (number of set bits) within each SEW-wide element
            Self::VcpopV { vd, vs2, vm } => {
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
                zvbb_helpers::execute_vcpop::<Reg, _>(env, vd, vs2, vm);
            }
            // vwsll: widening shift-left-logical; vd is 2*SEW wide, vs2/src are SEW wide.
            // SEW=E64 is illegal (cannot double); LMUL=M8 is illegal (EMUL(vd)=16 out of range).
            Self::VwsllVv { vd, vs2, vs1, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<Env::Hart>::new(sew) else {
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
                    sew.as_eew(),
                )?;
                let vs1 = zvexx_helpers::vreg_group::<Reg, Env, _, _>(
                    program_counter,
                    config,
                    vs1,
                    sew.as_eew(),
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
                zvbb_helpers::execute_vwsll::<Reg, _>(
                    env,
                    vd,
                    vs2,
                    zvbb_helpers::OpSrc::Vreg(vs1),
                    widening_sew,
                    vm,
                );
            }
            Self::VwsllVx {
                vm,
                vd,
                vs2,
                rs1: _,
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<Env::Hart>::new(sew) else {
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
                    sew.as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                let scalar = rs1_value.as_i64().cast_unsigned();
                zvbb_helpers::execute_vwsll::<Reg, _>(
                    env,
                    vd,
                    vs2,
                    zvbb_helpers::OpSrc::Scalar(scalar),
                    widening_sew,
                    vm,
                );
            }
            // vwsll.vi: standard 5-bit immediate; vm is the normal mask-control bit
            Self::VwsllVi { vd, vs2, uimm, vm } => {
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
                let Some(widening_sew) = zvexx_helpers::WideningSew::<Env::Hart>::new(sew) else {
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
                    sew.as_eew(),
                )?;
                // The wide destination may only overlap narrow sources in its highest-numbered part
                zvexx_helpers::check_destination_overlap::<Reg, Env, _, _>(
                    program_counter,
                    vd,
                    vs2,
                )?;
                zvbb_helpers::execute_vwsll::<Reg, _>(
                    env,
                    vd,
                    vs2,
                    zvbb_helpers::OpSrc::Scalar(u64::from(uimm)),
                    widening_sew,
                    vm,
                );
            }
        }
        ExecutionResult::ContinueNoWrite
    }
}
