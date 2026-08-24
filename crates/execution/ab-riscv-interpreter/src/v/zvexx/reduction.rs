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
                // Spec §14: reductions with vstart > 0 are reserved; raise illegal instruction
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: `vs2` alignment checked; `vstart == 0` checked;
                // `vs1` and `vd` are single-register scalar operands
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc.wrapping_add(elem),
                    );
                }
            }
            Self::Vredand { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc & elem,
                    );
                }
            }
            Self::Vredor { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc | elem,
                    );
                }
            }
            Self::Vredxor { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, _sew| acc ^ elem,
                    );
                }
            }
            Self::Vredminu { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, sew| {
                            let mask = zvexx_arith_helpers::sew_mask(sew);
                            if elem & mask < acc & mask { elem } else { acc }
                        },
                    );
                }
            }
            Self::Vredmin { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
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
            }
            Self::Vredmaxu { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        |acc, elem, sew| {
                            let mask = zvexx_arith_helpers::sew_mask(sew);
                            if elem & mask > acc & mask { elem } else { acc }
                        },
                    );
                }
            }
            Self::Vredmax { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vredsum`
                unsafe {
                    zvexx_reduction_helpers::execute_reduce_op(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
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
            }
            Self::Vwredsumu { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // Widening: 2*SEW must fit in ELEN
                if u32::from(vtype.vsew().bits_width()) * 2 > u32::from(Env::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: `vs2` alignment checked; widening SEW constraint checked above;
                // `vstart == 0` checked; `vd` and `vs1` are single-register 2*SEW scalar operands
                unsafe {
                    zvexx_reduction_helpers::execute_widening_reduce_op::<false, _, _, _>(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        // Zero-extend vs2 elements then accumulate
                        |acc, elem, _sew| acc.wrapping_add(elem),
                    );
                }
            }
            Self::Vwredsum { vd, vs2, vs1, vm } => {
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
                if env.vstart() != Vstart::ZERO {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if u32::from(vtype.vsew().bits_width()) * 2 > u32::from(Env::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_arith_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let vl = env.vl();
                // SAFETY: see `Vwredsumu`
                unsafe {
                    zvexx_reduction_helpers::execute_widening_reduce_op::<true, _, _, _>(
                        env,
                        vd,
                        vs2,
                        vs1,
                        vm,
                        vl,
                        sew,
                        // Sign-extend vs2 elements then accumulate
                        |acc, elem, _sew| acc.wrapping_add(elem),
                    );
                }
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
