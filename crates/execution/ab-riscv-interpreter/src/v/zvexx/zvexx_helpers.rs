//! Opaque helpers for ZveXx extension

#[cfg(test)]
mod tests;

use crate::{ExecutionError, PackedAddress, ProgramCounter};
use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Size of an instruction in bytes.
///
/// All instructions here are the same size.
#[doc(hidden)]
pub const INSTRUCTION_SIZE: u8 = size_of::<u32>() as u8;

/// Check that two source register groups `[a, a + a_regs)` and `[b, b + b_regs)`, read with
/// different EEWs, do not overlap.
///
/// A vector register cannot provide source operands with more than one EEW in a single
/// instruction, including at different positions within the two groups, such encodings are
/// reserved (`norm:vreg_source_eew_rsv`). Sources with the same EEW may overlap freely.
#[inline(always)]
#[doc(hidden)]
#[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
pub fn check_sources_disjoint<Reg, Memory, PC>(
    program_counter: &PC,
    a: VReg,
    a_regs: u8,
    b: VReg,
    b_regs: u8,
) -> Result<(), ExecutionError<Reg::Type>>
where
    Reg: Register,
    PC: ProgramCounter<Reg::Type, Memory>,
{
    let a_start = u16::from(a.to_bits());
    let b_start = u16::from(b.to_bits());
    if a_start < b_start + u16::from(b_regs) && b_start < a_start + u16::from(a_regs) {
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
