//! ZveXx multiply and divide instructions

#[cfg(test)]
mod tests;
pub mod zvexx_muldiv_helpers;

use crate::v::vector_registers::VectorRegistersExt;
use crate::v::zvexx::zvexx_helpers;
use crate::{
    ExecutableInstruction, ExecutableInstructionCsr, ExecutableInstructionOperands, ExecutionError,
    ExecutionResult, FetchInstructionResult, InstructionFetcher, PackedAddress, ProgramCounter,
    RegisterFile, Rs1Rs2OperandValues, Rs1Rs2Operands, ThreadedExecutableInstruction,
    ThreadedExecutionResult, VirtualMemory,
};
use ab_riscv_macros::instruction_execution;
use ab_riscv_primitives::prelude::*;

#[instruction_execution]
const impl<Reg> ExecutableInstructionOperands for ZveXxMulDivInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, ExtState> ExecutableInstructionCsr<ExtState> for ZveXxMulDivInstruction<Reg> where
    Reg: Register
{
}

#[instruction_execution]
impl<Reg, Regs, ExtState, Memory, PC, InstructionHandler>
    ExecutableInstruction<Regs, ExtState, Memory, PC, InstructionHandler>
    for ZveXxMulDivInstruction<Reg>
where
    Reg: Register,
    Regs: RegisterFile<Reg>,
    ExtState: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ ExtState::ELEN }, { ExtState::VLEN }>]:,
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
        ext_state: &mut ExtState,
        _memory: &mut Memory,
        program_counter: &mut PC,
        _system_instruction_handler: &mut InstructionHandler,
    ) -> ExecutionResult<Self::Reg> {
        match self {
            // vmul.vv / vmul.vx - signed multiply, low half
            Self::VmulVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_mul(b),
                    );
                }
            }
            Self::VmulVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, _| a.wrapping_mul(b),
                    );
                }
            }
            // vmulh.vv / vmulh.vx - signed×signed multiply, high half
            Self::VmulhVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Zve64x excludes the high-half multiplies at SEW=64 (spec §18.2). The full "V"
                // extension includes them; the arithmetic itself is width-complete because the
                // 2*SEW product is formed in i128/u128
                if !Self::implements_extension::<V<_>>() && vtype.vsew() == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::mulh_ss,
                    );
                }
            }
            Self::VmulhVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Zve64x excludes the high-half multiplies at SEW=64 (spec §18.2). The full "V"
                // extension includes them; the arithmetic itself is width-complete because the
                // 2*SEW product is formed in i128/u128
                if !Self::implements_extension::<V<_>>() && vtype.vsew() == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::mulh_ss,
                    );
                }
            }
            // vmulhu.vv / vmulhu.vx - unsigned×unsigned multiply, high half
            Self::VmulhuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Zve64x excludes the high-half multiplies at SEW=64 (spec §18.2). The full "V"
                // extension includes them; the arithmetic itself is width-complete because the
                // 2*SEW product is formed in i128/u128
                if !Self::implements_extension::<V<_>>() && vtype.vsew() == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::mulhu_uu,
                    );
                }
            }
            Self::VmulhuVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Zve64x excludes the high-half multiplies at SEW=64 (spec §18.2). The full "V"
                // extension includes them; the arithmetic itself is width-complete because the
                // 2*SEW product is formed in i128/u128
                if !Self::implements_extension::<V<_>>() && vtype.vsew() == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::mulhu_uu,
                    );
                }
            }
            // vmulhsu.vv / vmulhsu.vx - signed×unsigned multiply, high half
            Self::VmulhsuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Zve64x excludes the high-half multiplies at SEW=64 (spec §18.2). The full "V"
                // extension includes them; the arithmetic itself is width-complete because the
                // 2*SEW product is formed in i128/u128
                if !Self::implements_extension::<V<_>>() && vtype.vsew() == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // vs2 is signed, vs1 is unsigned
                        zvexx_muldiv_helpers::mulhsu_su,
                    );
                }
            }
            Self::VmulhsuVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Zve64x excludes the high-half multiplies at SEW=64 (spec §18.2). The full "V"
                // extension includes them; the arithmetic itself is width-complete because the
                // 2*SEW product is formed in i128/u128
                if !Self::implements_extension::<V<_>>() && vtype.vsew() == Vsew::E64 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // scalar from rs1 is the unsigned operand; vs2 elements are signed
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        // vs2 is signed, scalar (rs1) is unsigned
                        zvexx_muldiv_helpers::mulhsu_su,
                    );
                }
            }
            // vdivu.vv / vdivu.vx - unsigned divide
            Self::VdivuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // Division by zero: quotient = all-ones for the SEW width (spec §12.11)
                        |a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            dividend.checked_div(divisor).unwrap_or(mask)
                        },
                    );
                }
            }
            Self::VdivuVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            dividend.checked_div(divisor).unwrap_or(mask)
                        },
                    );
                }
            }
            // vdiv.vv / vdiv.vx - signed divide
            Self::VdivVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::sdiv,
                    );
                }
            }
            Self::VdivVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::sdiv,
                    );
                }
            }
            // vremu.vv / vremu.vx - unsigned remainder
            Self::VremuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // Division by zero: remainder = dividend (spec §12.11)
                        |a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            if divisor == 0 {
                                dividend
                            } else {
                                dividend % divisor
                            }
                        },
                    );
                }
            }
            Self::VremuVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            let dividend = a & mask;
                            let divisor = b & mask;
                            if divisor == 0 {
                                dividend
                            } else {
                                dividend % divisor
                            }
                        },
                    );
                }
            }
            // vrem.vv / vrem.vx - signed remainder
            Self::VremVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::srem,
                    );
                }
            }
            Self::VremVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_arith_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        zvexx_muldiv_helpers::srem,
                    );
                }
            }
            // vwmulu.vv / vwmulu.vx - unsigned widening multiply
            Self::VwmuluVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                // dest_group_regs encodes EMUL=2*LMUL; None means EMUL>8, which is illegal
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // vd and vs2/vs1 must not overlap
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            (a & mask).wrapping_mul(b & mask)
                        },
                    );
                }
            }
            Self::VwmuluVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            (a & mask).wrapping_mul(b & mask)
                        },
                    );
                }
            }
            // vwmulsu.vv / vwmulsu.vx - signed×unsigned widening multiply
            Self::VwmulsuVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // vs2 is signed, vs1 is unsigned; widen both to full u64 before multiply
                        |a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zvexx_muldiv_helpers::sew_mask(sew);
                            sa.cast_unsigned().wrapping_mul(ub)
                        },
                    );
                }
            }
            Self::VwmulsuVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // scalar from rs1 is the unsigned operand; vs2 elements are signed
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zvexx_muldiv_helpers::sew_mask(sew);
                            sa.cast_unsigned().wrapping_mul(ub)
                        },
                    );
                }
            }
            // vwmul.vv / vwmul.vx - signed widening multiply
            Self::VwmulVv { vd, vs2, vs1, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs1),
                        vm,
                        sew,
                        // Both operands sign-extended; full 2*SEW product fits in u64
                        |a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let sb = zvexx_muldiv_helpers::sign_extend(b, sew);
                            sa.cast_unsigned().wrapping_mul(sb.cast_unsigned())
                        },
                    );
                }
            }
            Self::VwmulVx {
                vd,
                vs2,
                rs1: _,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // scalar from rs1 is sign-extended to XLEN; treat as signed SEW-wide
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_op(
                        ext_state,
                        vd,
                        vs2,
                        zvexx_muldiv_helpers::OpSrc::Scalar(scalar),
                        vm,
                        sew,
                        |a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let sb = zvexx_muldiv_helpers::sign_extend(b, sew);
                            sa.cast_unsigned().wrapping_mul(sb.cast_unsigned())
                        },
                    );
                }
            }
            // vmacc.vv / vmacc.vx - vd = vd + vs1 * vs2
            Self::VmaccVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vmacc: vd[i] = vd[i] + vs1[i] * vs2[i]
                        |acc, a, b, _| acc.wrapping_add(a.wrapping_mul(b)),
                    );
                }
            }
            Self::VmaccVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        |acc, a, b, _| acc.wrapping_add(a.wrapping_mul(b)),
                    );
                }
            }
            // vnmsac.vv / vnmsac.vx - vd = vd - vs1 * vs2
            Self::VnmsacVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vnmsac: vd[i] = vd[i] - vs1[i] * vs2[i]
                        |acc, a, b, _| acc.wrapping_sub(a.wrapping_mul(b)),
                    );
                }
            }
            Self::VnmsacVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        |acc, a, b, _| acc.wrapping_sub(a.wrapping_mul(b)),
                    );
                }
            }
            // vmadd.vv / vmadd.vx - vd = vs1 * vd + vs2
            Self::VmaddVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vmadd: vd[i] = vs1[i] * vd[i] + vs2[i]; acc=vd, a=vs1, b=vs2
                        |acc, a, b, _| a.wrapping_mul(acc).wrapping_add(b),
                    );
                }
            }
            Self::VmaddVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vmadd: vd[i] = rs1 * vd[i] + vs2[i]
                        |acc, a, b, _| a.wrapping_mul(acc).wrapping_add(b),
                    );
                }
            }
            // vnmsub.vv / vnmsub.vx - vd = -(vs1 * vd) + vs2
            Self::VnmsubVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vnmsub: vd[i] = -(vs1[i] * vd[i]) + vs2[i]; acc=vd, a=vs1, b=vs2
                        |acc, a, b, _| b.wrapping_sub(a.wrapping_mul(acc)),
                    );
                }
            }
            Self::VnmsubVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                let group_regs = vtype.vlmul().register_count();
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let sew = vtype.vsew();
                let scalar = rs1_value.as_i64().cast_unsigned();
                // SAFETY: alignment checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vnmsub: vd[i] = -(rs1 * vd[i]) + vs2[i]
                        |acc, a, b, _| b.wrapping_sub(a.wrapping_mul(acc)),
                    );
                }
            }
            // vwmaccu.vv / vwmaccu.vx - unsigned widening multiply-add
            Self::VwmaccuVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                // vd holds the 2*SEW accumulator
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vwmaccu: vd[i] = vd[i] + zext(vs1[i]) * zext(vs2[i])
                        |acc, a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add((a & mask).wrapping_mul(b & mask))
                        },
                    );
                }
            }
            Self::VwmaccuVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        |acc, a, b, sew| {
                            let mask = zvexx_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add((a & mask).wrapping_mul(b & mask))
                        },
                    );
                }
            }
            // vwmacc.vv / vwmacc.vx - signed widening multiply-add
            Self::VwmaccVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vwmacc: vd[i] = vd[i] + sext(vs1[i]) * sext(vs2[i])
                        |acc, a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let sb = zvexx_muldiv_helpers::sign_extend(b, sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(sb.cast_unsigned()))
                        },
                    );
                }
            }
            Self::VwmaccVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        |acc, a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let sb = zvexx_muldiv_helpers::sign_extend(b, sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(sb.cast_unsigned()))
                        },
                    );
                }
            }
            // vwmaccsu.vv / vwmaccsu.vx - signed×unsigned widening multiply-add
            Self::VwmaccsuVv { vd, vs1, vs2, vm } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs1,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs1,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_op(
                        ext_state,
                        vd,
                        vs1,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vwmaccsu: vd[i] = vd[i] + sext(vs1[i]) * zext(vs2[i])
                        |acc, a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zvexx_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(ub))
                        },
                    );
                }
            }
            Self::VwmaccsuVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // scalar (rs1) is the signed operand; vs2 elements are unsigned
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vwmaccsu.vx: vd[i] = vd[i] + sext(rs1) * zext(vs2[i])
                        // Helper passes (acc, scalar_as_a, vs2_as_b, sew): a=rs1 (signed),
                        // b=vs2 (unsigned)
                        |acc, a, b, sew| {
                            let sa = zvexx_muldiv_helpers::sign_extend(a, sew);
                            let ub = b & zvexx_muldiv_helpers::sew_mask(sew);
                            acc.wrapping_add(sa.cast_unsigned().wrapping_mul(ub))
                        },
                    );
                }
            }
            // vwmaccus.vx - unsigned×signed widening multiply-add (vx only)
            Self::VwmaccusVx {
                vd,
                rs1: _,
                vs2,
                vm,
            } => {
                if !ext_state.vector_instructions_allowed() {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let Some(vtype) = ext_state.vtype() else {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                };
                // Widening produces a 2*SEW result; an EEW above ELEN is reserved for every
                // implementation, so this is not a Zve64x-specific restriction
                if !zvexx_muldiv_helpers::widening_eew_supported(vtype.vsew(), ExtState::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let group_regs = vtype.vlmul().register_count();
                let dest_group_regs = zvexx_muldiv_helpers::widening_dest_register_count(
                    vtype.vlmul(),
                )
                .ok_or(ExecutionError::IllegalInstruction {
                    address: PackedAddress::new(
                        program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                    ),
                })?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    dest_group_regs,
                )?;
                zvexx_muldiv_helpers::check_vreg_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    group_regs,
                )?;
                if !vm && vd == VReg::V0 {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                zvexx_muldiv_helpers::check_no_widening_overlap::<Reg, _, _>(
                    program_counter,
                    vd,
                    vs2,
                    dest_group_regs,
                    group_regs,
                )?;
                let sew = vtype.vsew();
                // scalar (rs1) is the unsigned operand; vs2 elements are signed
                let scalar = rs1_value.as_u64();
                // SAFETY: alignment and overlap checked above; 2*SEW <= ELEN checked above
                unsafe {
                    zvexx_muldiv_helpers::execute_widening_muladd_scalar_op(
                        ext_state,
                        vd,
                        scalar,
                        zvexx_muldiv_helpers::OpSrc::Vreg(vs2),
                        vm,
                        sew,
                        // vwmaccus.vx: vd[i] = vd[i] + zext(rs1) * sext(vs2[i])
                        // Helper passes (acc, scalar_as_a, vs2_as_b, sew): a=rs1 (unsigned),
                        // b=vs2 (signed)
                        |acc, a, b, sew| {
                            let ua = a & zvexx_muldiv_helpers::sew_mask(sew);
                            let sb = zvexx_muldiv_helpers::sign_extend(b, sew);
                            acc.wrapping_add(sb.cast_unsigned().wrapping_mul(ua))
                        },
                    );
                }
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
