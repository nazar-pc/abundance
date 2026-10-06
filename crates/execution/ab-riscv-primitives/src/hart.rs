//! Hart configuration of an implementation

#[cfg(test)]
mod tests;

use crate::instructions::v::{Elen, Vlen};
use crate::registers::general_purpose::Register;
use core::fmt;
use core::marker::PhantomData;

/// Vector lengths of an implementation with vector extensions
#[derive(Debug, Clone, Copy)]
#[derive_const(PartialEq, Eq)]
pub struct VectorLengths {
    /// Maximum vector element width `ELEN` in bits
    pub mut(self) elen: Elen,
    /// Vector register width `VLEN` in bits
    pub mut(self) vlen: Vlen,
}

impl VectorLengths {
    /// Create new vector lengths, returns `None` if `ELEN > VLEN`
    #[inline(always)]
    pub const fn new(elen: Elen, vlen: Vlen) -> Option<Self> {
        if u32::from(elen) > u32::from(vlen) {
            return None;
        }
        Some(Self { elen, vlen })
    }
}

/// Hart configuration of an implementation that instruction enums are generic over.
///
/// It is implemented by marker types, describing properties of the implementation that
/// instructions depend on, like the register type.
pub const trait HartConfig: fmt::Debug + Copy + Eq + Send + Sync + 'static {
    /// General purpose register type
    type Reg: [const] Register;
}

/// Hart configuration of an implementation with vector extensions.
///
/// Vector instructions require it instead of [`HartConfig`].
pub const trait VectorHartConfig: [const] HartConfig {
    /// Vector lengths
    const VECTOR_LENGTHS: VectorLengths;
}

/// Basic hart configuration with a given register type and no other properties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasicHart<Reg>(PhantomData<Reg>);

const impl<Reg> HartConfig for BasicHart<Reg>
where
    Reg: [const] Register,
{
    type Reg = Reg;
}

/// Basic hart configuration with a given register type and vector lengths
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BasicVectorHart<Reg, const ELEN: Elen, const VLEN: Vlen>(PhantomData<Reg>);

const impl<Reg, const ELEN: Elen, const VLEN: Vlen> HartConfig for BasicVectorHart<Reg, ELEN, VLEN>
where
    Reg: [const] Register,
{
    type Reg = Reg;
}

const impl<Reg, const ELEN: Elen, const VLEN: Vlen> VectorHartConfig
    for BasicVectorHart<Reg, ELEN, VLEN>
where
    Reg: [const] Register,
{
    const VECTOR_LENGTHS: VectorLengths =
        VectorLengths::new(ELEN, VLEN).expect("`ELEN` must be <= `VLEN`");
}
