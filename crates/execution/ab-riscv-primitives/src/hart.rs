//! Hart configuration of an implementation

use crate::registers::general_purpose::Register;
use core::fmt;
use core::marker::PhantomData;

/// Hart configuration of an implementation that instruction enums are generic over.
///
/// It is implemented by marker types, describing properties of the implementation that
/// instructions depend on, like the register type.
pub const trait HartConfig: fmt::Debug + Copy + Eq + Send + Sync + 'static {
    /// General purpose register type
    type Reg: [const] Register;
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
