//! Vector configuration

#[cfg(test)]
mod tests;

use ab_riscv_primitives::prelude::*;
use core::hint::cold_path;

/// Vector configuration: `vtype` together with `vl`, where `vl` never exceeds `VLMAX` of `vtype`.
///
/// Neither `vtype` nor `vl` can be written by Zicsr instructions, they only ever change together
/// through `vset{i}vl{i}` and fault-only-first loads (which only shrink `vl`), so this is the
/// architectural state as a single value. Constructors enforce the invariant, which unsafe code
/// relies on to access vector register elements without bounds checks, so there must be no way
/// to construct an instance that violates it.
///
/// `vill` is represented as `None` in `Option<VectorConfig>`, `vl` is zero then.
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub struct VectorConfig<const ELEN: Elen, const VLEN: Vlen>
where
    [(); SUPPORTED_ELEN_VLEN::<ELEN, VLEN>]:,
{
    vtype: Vtype<ELEN, VLEN>,
    vl: Vl,
}

impl<const ELEN: Elen, const VLEN: Vlen> VectorConfig<ELEN, VLEN>
where
    [(); SUPPORTED_ELEN_VLEN::<ELEN, VLEN>]:,
{
    /// Create a new configuration.
    ///
    /// Returns `None` if `vl` exceeds `VLMAX` of `vtype`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new(vtype: Vtype<ELEN, VLEN>, vl: Vl) -> Option<Self> {
        if u32::from(vl) > u32::from(vtype.vlmax()) {
            cold_path();
            return None;
        }

        Some(Self { vtype, vl })
    }

    /// Create a new configuration the way `vset{i}vl{i}` does for application vector length
    /// `avl`, which is `min(AVL, VLMAX)`.
    ///
    /// This satisfies every constraint the specification places on `vl`, and with no choice left
    /// to the implementation, `vl` is the same on every implementation with the same `VLEN`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_avl(vtype: Vtype<ELEN, VLEN>, avl: Vl) -> Self {
        let vlmax = vtype.vlmax();
        let vl = if u32::from(avl) > u32::from(vlmax) {
            vlmax
        } else {
            avl
        };

        Self { vtype, vl }
    }

    /// Decode from raw `vtype` and `vl` register values.
    ///
    /// Returns `None` if `vtype` has `vill` set or is invalid otherwise, or if `vl` exceeds `VLMAX`
    /// of `vtype`. The latter is not a state the architecture can get into, it is treated as
    /// `vill` rather than corrected, so that whatever produced it does not go unnoticed.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn from_raw<Reg>(vtype: Reg::Type, vl: Reg::Type) -> Option<Self>
    where
        Reg: [const] Register,
    {
        let vtype = Vtype::from_raw::<Reg>(vtype)?;
        let Ok(vl) = u32::try_from(vl.as_u64()) else {
            cold_path();
            return None;
        };
        let vl = Vl::new(vl)?;

        Self::new(vtype, vl)
    }

    /// `vtype`
    #[inline(always)]
    pub const fn vtype(self) -> Vtype<ELEN, VLEN> {
        self.vtype
    }

    /// `vl`, which never exceeds [`Self::vlmax()`]
    #[inline(always)]
    pub const fn vl(self) -> Vl {
        self.vl
    }

    /// `VLMAX` of `vtype`
    #[inline(always)]
    pub const fn vlmax(self) -> Vl {
        self.vtype.vlmax()
    }

    /// The same configuration with `vl` reduced to `vl` if it is lower than the current one, which
    /// is what fault-only-first loads do
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn with_vl_at_most(self, vl: Vl) -> Self {
        let vl = if u32::from(vl) < u32::from(self.vl) {
            vl
        } else {
            self.vl
        };

        Self {
            vtype: self.vtype,
            vl,
        }
    }
}
