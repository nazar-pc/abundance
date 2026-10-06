//! Vector configuration

#[cfg(test)]
mod tests;

use ab_riscv_primitives::prelude::*;
use core::fmt;
use core::hint::{assert_unchecked, cold_path};
use core::marker::PhantomData;

/// Vector length of a [`VectorConfig`], which never exceeds `VLEN`.
///
/// `vl <= VLMAX = LMUL * VLEN / SEW <= VLEN`, since `LMUL <= 8` and `SEW >= 8`. Constructors
/// enforce the bound, so unsafe code can rely on it for accessing a single vector register, like
/// mask registers with one bit per element.
#[derive(Debug, Clone, Copy)]
pub struct BoundedVl<Hart>(Vl, PhantomData<Hart>)
where
    Hart: VectorHartConfig;

const impl<Hart> PartialEq for BoundedVl<Hart>
where
    Hart: VectorHartConfig,
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

const impl<Hart> Eq for BoundedVl<Hart> where Hart: VectorHartConfig {}

impl<Hart> fmt::Display for BoundedVl<Hart>
where
    Hart: VectorHartConfig,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

const impl<Hart> From<BoundedVl<Hart>> for Vl
where
    Hart: VectorHartConfig,
{
    #[inline(always)]
    fn from(value: BoundedVl<Hart>) -> Self {
        value.get()
    }
}

impl<Hart> BoundedVl<Hart>
where
    Hart: VectorHartConfig,
{
    /// Create a new instance, returns `None` if `vl` exceeds `VLEN`
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new(vl: Vl) -> Option<Self> {
        if u32::from(vl) > u32::from(Hart::VECTOR_LENGTHS.vlen) {
            cold_path();
            return None;
        }
        Some(Self(vl, PhantomData))
    }

    /// Vector length, which is `<= VLEN`
    #[inline(always)]
    pub const fn get(self) -> Vl {
        // SAFETY: Guaranteed by construction in `VectorConfig`
        unsafe {
            assert_unchecked(u32::from(self.0) <= u32::from(Hart::VECTOR_LENGTHS.vlen));
        }
        self.0
    }

    /// Element indices `0..vl`.
    ///
    /// Unlike a range of `u16`, which can't represent `vl = 65536` as an exclusive bound, this
    /// lets the compiler see that every index is below `vl` and `VLEN`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub fn indices(self) -> impl Iterator<Item = u16> {
        // `vl <= VLEN <= 65536`, so every index below it fits into `u16`
        (0..u32::from(self.get())).map(u32::truncate)
    }

    /// Vector length in bytes, `ceil(vl / 8)`, which is `<= VLEN.bytes()`
    #[inline(always)]
    pub const fn bytes(self) -> u16 {
        let bytes = self.get().bytes();
        // SAFETY: `vl <= VLEN` and `VLEN` is a multiple of 8
        unsafe {
            assert_unchecked(u32::from(bytes) <= Hart::VECTOR_LENGTHS.vlen.bytes());
        }
        bytes
    }
}

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
pub struct VectorConfig<Hart>
where
    Hart: VectorHartConfig,
{
    vtype: Vtype<Hart>,
    vl: Vl,
}

const impl<Hart> PartialEq for VectorConfig<Hart>
where
    Hart: VectorHartConfig,
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.vtype == other.vtype && self.vl == other.vl
    }
}

const impl<Hart> Eq for VectorConfig<Hart> where Hart: VectorHartConfig {}

impl<Hart> VectorConfig<Hart>
where
    Hart: VectorHartConfig,
{
    /// Create a new configuration.
    ///
    /// Returns `None` if `vl` exceeds `VLMAX` of `vtype`.
    #[inline(always)]
    #[cfg_attr(feature = "no-panic", no_panic_const::no_panic)]
    pub const fn new(vtype: Vtype<Hart>, vl: Vl) -> Option<Self> {
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
    pub const fn from_avl(vtype: Vtype<Hart>, avl: Vl) -> Self {
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
    pub const fn vtype(self) -> Vtype<Hart> {
        self.vtype
    }

    /// `vl`, which never exceeds [`Self::vlmax()`] and hence `VLEN`
    #[inline(always)]
    pub const fn vl(self) -> BoundedVl<Hart> {
        BoundedVl(self.vl, PhantomData)
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
