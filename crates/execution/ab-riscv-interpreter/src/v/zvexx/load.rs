//! ZveXx vector load instructions

#[cfg(test)]
pub(super) mod tests;
pub mod zvexx_load_helpers;

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
const impl<Reg> ExecutableInstructionOperands for ZveXxLoadInstruction<Reg> where Reg: Register {}

#[instruction_execution]
const impl<Reg, Env> ExecutableInstructionCsr<Env> for ZveXxLoadInstruction<Reg> where Reg: Register {}

#[instruction_execution]
impl<Reg, Regs, Env, Memory, PC> ExecutableInstruction<Regs, Env, Memory, PC>
    for ZveXxLoadInstruction<Reg>
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
            rs2_value,
        }: Rs1Rs2OperandValues<<Self::Reg as Register>::Type>,
        _regs: &mut Regs,
        env: &mut Env,
        memory: &mut Memory,
        program_counter: &mut PC,
    ) -> ExecutionResult<Self::Reg> {
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
                // The element width hint does not affect the result, but `EEW > ELEN` is still
                // reserved
                if u32::from(eew.bits_width()) > u32::from(Env::ELEN) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vd.is_group_aligned(nreg) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // `evl = NREG * VLEN / EEW` elements, regardless of `vtype` and `vl`, which is at
                // most `8 * 65536 / 8` and never saturates
                let vl = Vl::new_saturating(
                    u32::from(nreg.get()) * Env::VLEN.bytes() / u32::from(eew.bytes_width()),
                );
                // `vstart >= evl` is reserved
                if env.vstart() >= vl {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // SAFETY:
                // - alignment: `vd % nreg == 0` checked above, so `vd + nreg <= 32`
                // - `evl = nreg * VLEN.bytes() / eew.bytes()` elements fill the register group
                //   exactly
                // - unmasked
                unsafe {
                    zvexx_load_helpers::execute_unit_stride_load(
                        env,
                        memory,
                        vd,
                        true,
                        rs1_value.as_u64(),
                        eew,
                        nreg,
                        Nf::N1,
                        vl,
                        None,
                    )?;
                }
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
                // `evl = ceil(vl / 8)` elements with `EEW = 8`
                let vl = Vl::from(config.vl().bytes());
                // SAFETY:
                // - a single register is always aligned and within the register file
                // - `evl <= VLEN.bytes()` since `vl <= VLEN`
                // - unmasked
                unsafe {
                    zvexx_load_helpers::execute_unit_stride_load(
                        env,
                        memory,
                        vd,
                        true,
                        rs1_value.as_u64(),
                        Eew::E8,
                        VRegGroupSize::R1,
                        Nf::N1,
                        vl,
                        None,
                    )?;
                }
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
                let vtype = config.vtype();
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm
                    && zvexx_load_helpers::groups_overlap(
                        vd,
                        group_regs,
                        VReg::V0,
                        VRegGroupSize::R1,
                    )
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let vl = config.vl().get();
                // SAFETY:
                // - alignment: `check_register_group_alignment` verified `vd % group_regs == 0` and
                //   `vd + group_regs <= 32`, satisfying both the alignment and nf=1 bounds
                //   preconditions
                // - `vl <= group_regs * VLEN.bytes() / eew.bytes()`: `group_regs` is the EMUL
                //   computed for this `eew` and `vtype`, so this VLMAX equals the architectural
                //   VLMAX that bounds `vl`
                // - mask overlap: checked above via `groups_overlap`
                unsafe {
                    zvexx_load_helpers::execute_unit_stride_load(
                        env,
                        memory,
                        vd,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        Nf::N1,
                        vl,
                        None,
                    )?;
                }
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
                let vtype = config.vtype();
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm
                    && zvexx_load_helpers::groups_overlap(
                        vd,
                        group_regs,
                        VReg::V0,
                        VRegGroupSize::R1,
                    )
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                let vl = config.vl().get();
                // SAFETY: preconditions identical to `Vle`; see that arm for the full argument.
                unsafe {
                    zvexx_load_helpers::execute_unit_stride_load(
                        env,
                        memory,
                        vd,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        Nf::N1,
                        vl,
                        Some(config),
                    )?;
                }
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
                let vtype = config.vtype();
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    group_regs,
                )?;
                if !vm
                    && zvexx_load_helpers::groups_overlap(
                        vd,
                        group_regs,
                        VReg::V0,
                        VRegGroupSize::R1,
                    )
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // rs2 holds a signed stride; reinterpret the register value as signed
                let stride = rs2_value.as_i64();
                // SAFETY:
                // - alignment and nf=1 bounds: `check_register_group_alignment` verified `vd %
                //   group_regs == 0` and `vd + group_regs <= 32`
                // - `vl <= group_regs * VLEN.bytes() / eew.bytes()`: `group_regs` is the EMUL for
                //   this `eew` and `vtype`, so this VLMAX equals the architectural VLMAX bounding
                //   `vl`
                // - mask overlap: checked above via `groups_overlap`
                unsafe {
                    zvexx_load_helpers::execute_strided_load(
                        env,
                        config,
                        memory,
                        vd,
                        vm,
                        rs1_value.as_u64(),
                        stride,
                        eew,
                        group_regs,
                        Nf::N1,
                    )?;
                }
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
                let vtype = config.vtype();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    data_group_regs,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // Non-segment indexed loads permit `vd`/`vs2` overlap under the general
                // EEW-relative overlap rule (e.g. when the data and index EEW match); only
                // disallowed overlaps are reserved.
                if !zvexx_load_helpers::indexed_load_overlap_allowed(
                    vd,
                    data_group_regs,
                    vs2,
                    index_group_regs,
                    index_eew,
                    vtype.vsew(),
                    vtype.vlmul(),
                ) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm
                    && zvexx_load_helpers::groups_overlap(
                        vd,
                        data_group_regs,
                        VReg::V0,
                        VRegGroupSize::R1,
                    )
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // SAFETY:
                // - data alignment/nf=1 bounds: `check_register_group_alignment` on `vd`
                // - index alignment/bounds: `check_register_group_alignment` on `vs2`
                // - `vl <= data_group_regs * VLEN.bytes() / data_eew.bytes()`: data EEW = SEW and
                //   `data_group_regs = LMUL`, so VLMAX = LMUL * VLEN / SEW, which bounds `vl`
                // - `vl <= index_group_regs * VLEN.bytes() / index_eew.bytes()`: `index_group_regs`
                //   is EMUL_index defined so this VLMAX_index equals the architectural VLMAX
                // - `vd`/`vs2` overlap (if any) satisfies the general EEW overlap rule, checked
                //   above; the in-order element loop reads index element `i` before writing data
                //   element `i`, and that rule guarantees a data write never clobbers an index
                //   element that has not yet been consumed
                // - mask overlap: checked above via `groups_overlap`
                unsafe {
                    zvexx_load_helpers::execute_indexed_load(
                        env,
                        config,
                        memory,
                        vd,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        Nf::N1,
                    )?;
                }
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
                let vtype = config.vtype();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vd,
                    data_group_regs,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                // Non-segment indexed loads permit `vd`/`vs2` overlap under the general
                // EEW-relative overlap rule; see the `Vluxei` arm for details.
                if !zvexx_load_helpers::indexed_load_overlap_allowed(
                    vd,
                    data_group_regs,
                    vs2,
                    index_group_regs,
                    index_eew,
                    vtype.vsew(),
                    vtype.vlmul(),
                ) {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                if !vm
                    && zvexx_load_helpers::groups_overlap(
                        vd,
                        data_group_regs,
                        VReg::V0,
                        VRegGroupSize::R1,
                    )
                {
                    ::core::hint::cold_path();
                    return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    });
                }
                // SAFETY: preconditions identical to `Vluxei`; see that arm for the full
                // argument.
                unsafe {
                    zvexx_load_helpers::execute_indexed_load(
                        env,
                        config,
                        memory,
                        vd,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        Nf::N1,
                    )?;
                }
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
                let vtype = config.vtype();
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::validate_segment_registers::<Reg, _, _>(
                    program_counter,
                    vd,
                    vm,
                    group_regs,
                    nf,
                )?;
                let vl = config.vl().get();
                // SAFETY:
                // - alignment and nf-group bounds: `validate_segment_registers` verified `vd %
                //   group_regs == 0` and `vd + nf * group_regs <= 32`
                // - `vl <= group_regs * VLEN.bytes() / eew.bytes()`: `group_regs` is the EMUL for
                //   this `eew` and `vtype`, so this VLMAX equals the architectural VLMAX bounding
                //   `vl`
                // - mask overlap with v0: `validate_segment_registers` checked `vd.to_bits() != 0`
                //   when `vm=false`, ensuring no field group contains v0
                unsafe {
                    zvexx_load_helpers::execute_unit_stride_load(
                        env,
                        memory,
                        vd,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        nf,
                        vl,
                        None,
                    )?;
                }
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
                let vtype = config.vtype();
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::validate_segment_registers::<Reg, _, _>(
                    program_counter,
                    vd,
                    vm,
                    group_regs,
                    nf,
                )?;
                let vl = config.vl().get();
                // SAFETY: preconditions identical to `Vlseg`; see that arm for the full argument.
                unsafe {
                    zvexx_load_helpers::execute_unit_stride_load(
                        env,
                        memory,
                        vd,
                        vm,
                        rs1_value.as_u64(),
                        eew,
                        group_regs,
                        nf,
                        vl,
                        Some(config),
                    )?;
                }
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
                let vtype = config.vtype();
                let group_regs =
                    vtype
                        .eew_register_count(eew)
                        .ok_or(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        })?;
                zvexx_load_helpers::validate_segment_registers::<Reg, _, _>(
                    program_counter,
                    vd,
                    vm,
                    group_regs,
                    nf,
                )?;
                let stride = rs2_value.as_i64();
                // SAFETY:
                // - alignment and nf-group bounds: `validate_segment_registers` verified `vd %
                //   group_regs == 0` and `vd + nf * group_regs <= 32`
                // - `vl <= group_regs * VLEN.bytes() / eew.bytes()`: `group_regs` is EMUL for this
                //   `eew` and `vtype`
                // - mask overlap: `validate_segment_registers` checked `vd.to_bits() != 0` when
                //   `vm=false`
                unsafe {
                    zvexx_load_helpers::execute_strided_load(
                        env,
                        config,
                        memory,
                        vd,
                        vm,
                        rs1_value.as_u64(),
                        stride,
                        eew,
                        group_regs,
                        nf,
                    )?;
                }
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
                let vtype = config.vtype();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                // `validate_segment_registers` is called before the per-field overlap loop so
                // that `vd.to_bits() + f * data_group_regs < 32` is established for all `f < nf`,
                // which is required by the `VReg::from_bits` call inside the loop.
                zvexx_load_helpers::validate_segment_registers::<Reg, _, _>(
                    program_counter,
                    vd,
                    vm,
                    data_group_regs,
                    nf,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                for f in 0..nf.fields_per_segment() {
                    // SAFETY: `vd.to_bits() + f * data_group_regs < 32` because
                    // `validate_segment_registers` established `vd.to_bits() + nf * data_group_regs
                    // <= 32` and `f < nf`. The value is in [0, 31], so it is a valid `VReg`
                    // encoding.
                    let field_vd = unsafe {
                        VReg::from_bits(vd.to_bits() + f * data_group_regs.get()).unwrap_unchecked()
                    };
                    if zvexx_load_helpers::groups_overlap(
                        field_vd,
                        data_group_regs,
                        vs2,
                        index_group_regs,
                    ) {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        });
                    }
                }
                // SAFETY:
                // - data alignment/nf-group bounds: `validate_segment_registers` verified `vd %
                //   data_group_regs == 0` and `vd + nf * data_group_regs <= 32`
                // - index alignment/bounds: `check_register_group_alignment` verified `vs2 %
                //   EMUL_index == 0` and `vs2 + EMUL_index <= 32`
                // - no field/index group overlap: verified by the loop above
                // - `vl <= data_group_regs * VLEN.bytes() / data_eew.bytes()`: data EEW = SEW and
                //   `data_group_regs = LMUL`, so VLMAX = LMUL * VLEN / SEW bounds `vl`
                // - `vl <= EMUL_index * VLEN.bytes() / index_eew.bytes()`: `index_group_regs`
                //   (EMUL_index) is defined so this VLMAX_index equals the architectural VLMAX
                // - mask overlap: `validate_segment_registers` checked `vd.to_bits() != 0` when
                //   `vm=false`, and no field group starts at 0 since groups are contiguous from
                //   `vd` which is nonzero
                unsafe {
                    zvexx_load_helpers::execute_indexed_load(
                        env,
                        config,
                        memory,
                        vd,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        nf,
                    )?;
                }
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
                let vtype = config.vtype();
                let data_group_regs = vtype.vlmul().register_count();
                let index_group_regs = vtype.eew_register_count(index_eew).ok_or(
                    ExecutionError::IllegalInstruction {
                        address: PackedAddress::new(
                            program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                        ),
                    },
                )?;
                zvexx_load_helpers::validate_segment_registers::<Reg, _, _>(
                    program_counter,
                    vd,
                    vm,
                    data_group_regs,
                    nf,
                )?;
                zvexx_load_helpers::check_register_group_alignment::<Reg, _, _>(
                    program_counter,
                    vs2,
                    index_group_regs,
                )?;
                for f in 0..nf.fields_per_segment() {
                    // SAFETY: `vd.to_bits() + f * data_group_regs < 32` because
                    // `validate_segment_registers` established `vd.to_bits() + nf * data_group_regs
                    // <= 32` and `f < nf`. The value is in [0, 31], so it is a valid `VReg`
                    // encoding.
                    let field_vd = unsafe {
                        VReg::from_bits(vd.to_bits() + f * data_group_regs.get()).unwrap_unchecked()
                    };
                    if zvexx_load_helpers::groups_overlap(
                        field_vd,
                        data_group_regs,
                        vs2,
                        index_group_regs,
                    ) {
                        ::core::hint::cold_path();
                        return ExecutionResult::Err(ExecutionError::IllegalInstruction {
                            address: PackedAddress::new(
                                program_counter.old_pc(zvexx_helpers::INSTRUCTION_SIZE),
                            ),
                        });
                    }
                }
                // SAFETY: preconditions identical to `Vluxseg`; see that arm for the full
                // argument
                unsafe {
                    zvexx_load_helpers::execute_indexed_load(
                        env,
                        config,
                        memory,
                        vd,
                        vs2,
                        vm,
                        rs1_value.as_u64(),
                        vtype.vsew().as_eew(),
                        index_eew,
                        data_group_regs,
                        nf,
                    )?;
                }
            }
        }

        ExecutionResult::ContinueNoWrite
    }
}
