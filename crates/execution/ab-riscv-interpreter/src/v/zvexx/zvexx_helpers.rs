//! Opaque helpers for ZveXx extension

#[cfg(test)]
mod tests;

use crate::v::vector_config::VectorConfig;
use crate::v::vector_registers::{VRegGroup, VRegSegmentGroup, VectorRegistersExt};
use crate::{ExecutionError, PackedAddress, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::cmp::Ordering;
use core::hint::cold_path;

/// Size of an instruction in bytes.
///
/// All instructions here are the same size.
#[doc(hidden)]
pub const INSTRUCTION_SIZE: u8 = size_of::<u32>() as u8;

/// Whether a vector instruction other than a load, a store or `vset{i}vl{i}` can execute.
///
/// Besides vector instructions being enabled, this requires `vstart` to be zero. These
/// instructions either complete or trap without modifying any state, so a non-zero `vstart` is a
/// value this implementation never produces for them, which permits raising an illegal instruction
/// exception for it (`norm:vstart_vtype_dep`). Reductions, `vcompress.vm` and several mask
/// instructions require that regardless.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn non_memory_instruction_allowed<Reg, Env>(env: &Env) -> bool
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
{
    env.vector_instructions_allowed() && env.vstart() == Vstart::ZERO
}

/// Error of [`vreg_group()`] and [`vreg_segment_group()`], an illegal instruction at `address`.
///
/// Unlike a whole [`ExecutionError`], this is fully initialized. A group and an `ExecutionError`
/// sharing a `Result` makes the compiler carry bytes of groups from earlier instructions around the
/// interpreter loop into the partially initialized error, which made register allocation of the
/// loop explode.
#[derive(Debug, Clone, Copy)]
struct InvalidRegisterGroup<Address>
where
    Address: Copy,
{
    address: PackedAddress<Address>,
}

const impl<Address> From<InvalidRegisterGroup<Address>> for ExecutionError<Address>
where
    Address: Copy,
{
    #[inline(always)]
    fn from(error: InvalidRegisterGroup<Address>) -> Self {
        Self::IllegalInstruction {
            address: error.address,
        }
    }
}

/// Register group starting at `base` with elements of width `eew` under `config`.
///
/// Raises an illegal instruction exception if [`VRegGroup::new()`] rejects it.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn vreg_group<Reg, Env, Memory, PC>(
    program_counter: &PC,
    config: VectorConfig<{ Env::ELEN }, { Env::VLEN }>,
    base: VReg,
    eew: Eew,
) -> Result<VRegGroup<{ Env::VLEN }>, impl Into<ExecutionError<Reg::Type>>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    [(); SUPPORTED_ELEN_VLEN::<{ Env::ELEN }, { Env::VLEN }>]:,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    let Some(group) = VRegGroup::new(config, base, eew) else {
        cold_path();
        return Err(InvalidRegisterGroup {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    };
    Ok(group)
}

/// Segment group of `nf` fields, the first of which is `first`.
///
/// Raises an illegal instruction exception if [`VRegSegmentGroup::new()`] rejects it.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn vreg_segment_group<Reg, Env, Memory, PC>(
    program_counter: &PC,
    first: VRegGroup<{ Env::VLEN }>,
    nf: Nf,
) -> Result<VRegSegmentGroup<{ Env::VLEN }>, impl Into<ExecutionError<Reg::Type>>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    let Some(group) = VRegSegmentGroup::new(first, nf) else {
        cold_path();
        return Err(InvalidRegisterGroup {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    };
    Ok(group)
}

/// Check that destination `vd` overlaps source `vs` only in a way the spec permits for groups with
/// different EEWs (spec §5.2):
///
/// - the EEWs are equal, then any overlap is fine;
/// - the destination EEW is smaller and the overlap is in the lowest-numbered part of the source
///   group, i.e. the destination starts at the source's base register (`vnsrl.wv v2, v2, v6`);
/// - the destination EEW is larger, the source `EMUL` is at least 1 and the overlap is in the
///   highest-numbered part of the destination group, i.e. both groups end at the same register
///   (`vwsubu.wv v2, v14, v3` with `LMUL=1`, where the narrow `v3` aliases the high register of the
///   wide `{v2, v3}` destination).
///
/// Any other overlap is illegal, in particular any overlap at all with a fractional source `EMUL`
/// when the destination EEW is larger, even though such a source still occupies a whole register.
/// Instructions that forbid overlaps that this permits check that separately.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn check_destination_overlap<Reg, Env, Memory, PC>(
    program_counter: &PC,
    vd: VRegGroup<{ Env::VLEN }>,
    vs: VRegGroup<{ Env::VLEN }>,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    let allowed = !vd.overlaps(vs)
        || match vd.eew().bytes_width().cmp(&vs.eew().bytes_width()) {
            Ordering::Equal => true,
            Ordering::Less => vd.base() == vs.base(),
            Ordering::Greater => {
                !vs.emul().is_fractional()
                    && vd.base().to_bits() + vd.group_regs().get()
                        == vs.base().to_bits() + vs.group_regs().get()
            }
        };
    if !allowed {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    }
    Ok(())
}

/// Check that register groups `a` and `b` don't overlap
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn check_groups_disjoint<Reg, Env, Memory, PC>(
    program_counter: &PC,
    a: VRegGroup<{ Env::VLEN }>,
    b: VRegGroup<{ Env::VLEN }>,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    if a.overlaps(b) {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    }
    Ok(())
}

/// Check that single register `reg`, like a mask register, is not one of the registers of `group`
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn check_register_outside_group<Reg, Env, Memory, PC>(
    program_counter: &PC,
    group: VRegGroup<{ Env::VLEN }>,
    reg: VReg,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    Env: VectorRegistersExt<Reg>,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    if group.contains(reg) {
        cold_path();
        return Err(ExecutionError::IllegalInstruction {
            address: PackedAddress::new(program_counter.old_pc(INSTRUCTION_SIZE)),
        });
    }
    Ok(())
}

/// Element widths of a widening operation: `SEW` of the narrow operands and `2*SEW` of the wide
/// ones, where `2*SEW <= ELEN`.
///
/// An EEW greater than `ELEN` is reserved by the RISC-V "V" spec §3.4.2 for every implementation,
/// so no vector extension makes `SEW == ELEN` legal for a widening instruction.
#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub struct WideningSew<const ELEN: Elen> {
    narrow: Vsew,
    wide: Vsew,
}

impl<const ELEN: Elen> WideningSew<ELEN> {
    /// Returns `None` when `2*SEW` exceeds `ELEN`.
    ///
    /// `ELEN` above 64 is rejected at compile time: widening with `SEW = 64` would be legal then,
    /// but elements are processed as `u64` here, so a 128-bit wide operand is not supported.
    ///
    /// ```compile_fail
    /// use ab_riscv_interpreter::v::zvexx::zvexx_helpers::WideningSew;
    /// use ab_riscv_primitives::prelude::*;
    ///
    /// let _ = WideningSew::<{ Elen::L128 }>::new(Vsew::E8);
    /// ```
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new(sew: Vsew) -> Option<Self> {
        const {
            assert!(
                ELEN <= Elen::L64,
                "ELEN above 64 is not supported by the interpreter"
            );
        }

        let Some(wide) = sew.double_width() else {
            return None;
        };
        if u32::from(wide.bits_width()) > u32::from(ELEN) {
            return None;
        }
        Some(Self { narrow: sew, wide })
    }

    /// Element width of the narrow operands, `SEW`
    #[inline(always)]
    pub const fn narrow(self) -> Vsew {
        self.narrow
    }

    /// Element width of the wide operands, `2*SEW`
    #[inline(always)]
    pub const fn wide(self) -> Vsew {
        self.wide
    }
}

/// Element widths of an integer extension: `SEW/factor` of the source and `SEW` of the
/// destination
#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub struct ExtensionSew {
    source: Vsew,
    dest: Vsew,
}

impl ExtensionSew {
    /// Returns `None` when `SEW/factor` is narrower than 8 bits
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new(sew: Vsew, factor: VsewFactor) -> Option<Self> {
        let Some(source) = sew.divide_by_factor(factor) else {
            return None;
        };
        Some(Self { source, dest: sew })
    }

    /// Element width of the source operand, `SEW/factor`
    #[inline(always)]
    pub const fn source(self) -> Vsew {
        self.source
    }

    /// Element width of the destination, `SEW`
    #[inline(always)]
    pub const fn dest(self) -> Vsew {
        self.dest
    }
}
