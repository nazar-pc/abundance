//! Opaque helpers for ZveXx extension

#[cfg(test)]
mod tests;

use ab_riscv_primitives::prelude::*;

/// Size of an instruction in bytes.
///
/// All instructions here are the same size.
#[doc(hidden)]
pub const INSTRUCTION_SIZE: u8 = size_of::<u32>() as u8;

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
