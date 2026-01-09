#[cfg(test)]
mod tests;

use crate::registers::private::{GenericRegisterInternal, PhantomRegister};
use core::fmt;
use core::hint::unreachable_unchecked;
use core::marker::Destruct;
use core::ops::{Add, AddAssign, Sub, SubAssign};

mod private {
    use core::marker::PhantomData;

    pub const trait GenericRegisterInternal<Type> {
        /// Whether the register is a zero register
        fn is_zero(&self) -> bool;

        /// Offset in a set of registers
        fn offset(self) -> usize;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct PhantomRegister<Type>(PhantomData<Type>);
}

/// Generic register
pub const trait GenericRegister:
    fmt::Display
    + fmt::Debug
    + [const] Eq
    + [const] GenericRegisterInternal<Self::Type>
    + [const] Destruct
    + Copy
    + Sized
{
    /// The number of general purpose registers.
    ///
    /// Canonically 32 unless E extension is used, in which case 16.
    const N: usize;
    /// Register type.
    ///
    /// `u32` for RV32 and `u64` for RV64.
    type Type: [const] Default
        + [const] From<u8>
        + [const] Into<u64>
        + [const] Eq
        + [const] Add
        + [const] AddAssign
        + [const] Sub
        + [const] SubAssign
        + fmt::Display
        + fmt::Debug
        + Copy
        + Sized;

    /// Create a register from its bit representation
    fn from_bits(bits: u8) -> Option<Self>;
}

/// A set of RISC-V registers
#[derive(Debug, Clone, Copy)]
pub struct Registers<Reg>
where
    Reg: GenericRegister,
    [(); Reg::N]:,
{
    regs: [Reg::Type; Reg::N],
}

impl<Reg> Default for Registers<Reg>
where
    Reg: GenericRegister,
    [(); Reg::N]: Default,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            regs: [Reg::Type::default(); Reg::N],
        }
    }
}

const impl<Reg> Registers<Reg>
where
    Reg: GenericRegister + [const] Eq,
    [(); Reg::N]:,
{
    #[inline(always)]
    pub fn read(&self, reg: Reg) -> Reg::Type
    where
        Reg: [const] GenericRegister,
    {
        if reg.is_zero() {
            // Always zero
            return Reg::Type::default();
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(reg.offset()) }
    }

    #[inline(always)]
    pub fn write(&mut self, reg: Reg, value: Reg::Type)
    where
        Reg: [const] GenericRegister,
    {
        if reg.is_zero() {
            // Writes are ignored
            return;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(reg.offset()) } = value;
    }
}

/// RISC-V register for RV32E/RV64E.
///
/// Use `Type = u32` for RV32E and `Type = u64` for RV64E.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum EReg<Type> {
    /// Always zero: `x0`
    Zero = 0,
    /// Return address: `x1`
    Ra = 1,
    /// Stack pointer: `x2`
    Sp = 2,
    /// Global pointer: `x3`
    Gp = 3,
    /// Thread pointer: `x4`
    Tp = 4,
    /// Temporary/alternate return address: `x5`
    T0 = 5,
    /// Temporary: `x6`
    T1 = 6,
    /// Temporary: `x7`
    T2 = 7,
    /// Saved register/frame pointer: `x8`
    S0 = 8,
    /// Saved register: `x9`
    S1 = 9,
    /// Function argument/return value: `x10`
    A0 = 10,
    /// Function argument/return value: `x11`
    A1 = 11,
    /// Function argument: `x12`
    A2 = 12,
    /// Function argument: `x13`
    A3 = 13,
    /// Function argument: `x14`
    A4 = 14,
    /// Function argument: `x15`
    A5 = 15,
    /// Phantom register that is never constructed and is only used due to type system limitations
    #[doc(hidden)]
    Phantom(PhantomRegister<Type>),
}

impl<Type> fmt::Display for EReg<Type> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "zero"),
            Self::Ra => write!(f, "ra"),
            Self::Sp => write!(f, "sp"),
            Self::Gp => write!(f, "gp"),
            Self::Tp => write!(f, "tp"),
            Self::T0 => write!(f, "t0"),
            Self::T1 => write!(f, "t1"),
            Self::T2 => write!(f, "t2"),
            Self::S0 => write!(f, "s0"),
            Self::S1 => write!(f, "s1"),
            Self::A0 => write!(f, "a0"),
            Self::A1 => write!(f, "a1"),
            Self::A2 => write!(f, "a2"),
            Self::A3 => write!(f, "a3"),
            Self::A4 => write!(f, "a4"),
            Self::A5 => write!(f, "a5"),
            Self::Phantom(_) => {
                // SAFETY: Phantom register is never constructed
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

impl<Type> fmt::Debug for EReg<Type> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<Type> const PartialEq for EReg<Type> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        // This is quite ugly, but there doesn't seem to be a much better way with `Phantom` variant
        matches!(
            (self, other),
            (Self::Zero, Self::Zero)
                | (Self::Ra, Self::Ra)
                | (Self::Sp, Self::Sp)
                | (Self::Gp, Self::Gp)
                | (Self::Tp, Self::Tp)
                | (Self::T0, Self::T0)
                | (Self::T1, Self::T1)
                | (Self::T2, Self::T2)
                | (Self::S0, Self::S0)
                | (Self::S1, Self::S1)
                | (Self::A0, Self::A0)
                | (Self::A1, Self::A1)
                | (Self::A2, Self::A2)
                | (Self::A3, Self::A3)
                | (Self::A4, Self::A4)
                | (Self::A5, Self::A5)
                | (Self::Phantom(_), Self::Phantom(_))
        )
    }
}

impl<Type> const Eq for EReg<Type> {}

impl const GenericRegisterInternal<u32> for EReg<u32> {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        matches!(self, Self::Zero)
    }

    #[inline(always)]
    fn offset(self) -> usize {
        // NOTE: `transmute()` is requited here, otherwise performance suffers A LOT for unknown
        // reason
        // SAFETY: Enum is `#[repr(u8)]` and doesn't have any fields
        usize::from(unsafe { core::mem::transmute::<Self, u8>(self) })
        // match self {
        //     Self::Zero => 0,
        //     Self::Ra => 1,
        //     Self::Sp => 2,
        //     Self::Gp => 3,
        //     Self::Tp => 4,
        //     Self::T0 => 5,
        //     Self::T1 => 6,
        //     Self::T2 => 7,
        //     Self::S0 => 8,
        //     Self::S1 => 9,
        //     Self::A0 => 10,
        //     Self::A1 => 11,
        //     Self::A2 => 12,
        //     Self::A3 => 13,
        //     Self::A4 => 14,
        //     Self::A5 => 15,
        //     Self::Phantom(_) => {
        //         // SAFETY: Phantom register is never constructed
        //         unsafe { unreachable_unchecked() }
        //     },
        // }
    }
}

impl const GenericRegisterInternal<u64> for EReg<u64> {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        matches!(self, Self::Zero)
    }

    #[inline(always)]
    fn offset(self) -> usize {
        // NOTE: `transmute()` is requited here, otherwise performance suffers A LOT for unknown
        // reason
        // SAFETY: Enum is `#[repr(u8)]` and doesn't have any fields
        usize::from(unsafe { core::mem::transmute::<Self, u8>(self) })
        // match self {
        //     Self::Zero => 0,
        //     Self::Ra => 1,
        //     Self::Sp => 2,
        //     Self::Gp => 3,
        //     Self::Tp => 4,
        //     Self::T0 => 5,
        //     Self::T1 => 6,
        //     Self::T2 => 7,
        //     Self::S0 => 8,
        //     Self::S1 => 9,
        //     Self::A0 => 10,
        //     Self::A1 => 11,
        //     Self::A2 => 12,
        //     Self::A3 => 13,
        //     Self::A4 => 14,
        //     Self::A5 => 15,
        //     Self::Phantom(_) => {
        //         // SAFETY: Phantom register is never constructed
        //         unsafe { unreachable_unchecked() }
        //     },
        // }
    }
}

impl const GenericRegister for EReg<u32> {
    const N: usize = 16;
    type Type = u32;

    #[inline(always)]
    fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Zero),
            1 => Some(Self::Ra),
            2 => Some(Self::Sp),
            3 => Some(Self::Gp),
            4 => Some(Self::Tp),
            5 => Some(Self::T0),
            6 => Some(Self::T1),
            7 => Some(Self::T2),
            8 => Some(Self::S0),
            9 => Some(Self::S1),
            10 => Some(Self::A0),
            11 => Some(Self::A1),
            12 => Some(Self::A2),
            13 => Some(Self::A3),
            14 => Some(Self::A4),
            15 => Some(Self::A5),
            _ => None,
        }
    }
}

impl const GenericRegister for EReg<u64> {
    const N: usize = 16;
    type Type = u64;

    #[inline(always)]
    fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Zero),
            1 => Some(Self::Ra),
            2 => Some(Self::Sp),
            3 => Some(Self::Gp),
            4 => Some(Self::Tp),
            5 => Some(Self::T0),
            6 => Some(Self::T1),
            7 => Some(Self::T2),
            8 => Some(Self::S0),
            9 => Some(Self::S1),
            10 => Some(Self::A0),
            11 => Some(Self::A1),
            12 => Some(Self::A2),
            13 => Some(Self::A3),
            14 => Some(Self::A4),
            15 => Some(Self::A5),
            _ => None,
        }
    }
}

/// RISC-V register for RV32I/RV64I.
///
/// Use `Type = u32` for RV32I and `Type = u64` for RV64I.
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Reg<Type> {
    /// Always zero: `x0`
    Zero = 0,
    /// Return address: `x1`
    Ra = 1,
    /// Stack pointer: `x2`
    Sp = 2,
    /// Global pointer: `x3`
    Gp = 3,
    /// Thread pointer: `x4`
    Tp = 4,
    /// Temporary/alternate return address: `x5`
    T0 = 5,
    /// Temporary: `x6`
    T1 = 6,
    /// Temporary: `x7`
    T2 = 7,
    /// Saved register/frame pointer: `x8`
    S0 = 8,
    /// Saved register: `x9`
    S1 = 9,
    /// Function argument/return value: `x10`
    A0 = 10,
    /// Function argument/return value: `x11`
    A1 = 11,
    /// Function argument: `x12`
    A2 = 12,
    /// Function argument: `x13`
    A3 = 13,
    /// Function argument: `x14`
    A4 = 14,
    /// Function argument: `x15`
    A5 = 15,
    /// Function argument: `x16`
    A6 = 16,
    /// Function argument: `x17`
    A7 = 17,
    /// Saved register: `x18`
    S2 = 18,
    /// Saved register: `x19`
    S3 = 19,
    /// Saved register: `x20`
    S4 = 20,
    /// Saved register: `x21`
    S5 = 21,
    /// Saved register: `x22`
    S6 = 22,
    /// Saved register: `x23`
    S7 = 23,
    /// Saved register: `x24`
    S8 = 24,
    /// Saved register: `x25`
    S9 = 25,
    /// Saved register: `x26`
    S10 = 26,
    /// Saved register: `x27`
    S11 = 27,
    /// Temporary: `x28`
    T3 = 28,
    /// Temporary: `x29`
    T4 = 29,
    /// Temporary: `x30`
    T5 = 30,
    /// Temporary: `x31`
    T6 = 31,
    /// Phantom register that is never constructed and is only used due to type system limitations
    #[doc(hidden)]
    Phantom(PhantomRegister<Type>),
}

impl<Type> const From<EReg<u64>> for Reg<Type> {
    #[inline(always)]
    fn from(reg: EReg<u64>) -> Self {
        match reg {
            EReg::Zero => Self::Zero,
            EReg::Ra => Self::Ra,
            EReg::Sp => Self::Sp,
            EReg::Gp => Self::Gp,
            EReg::Tp => Self::Tp,
            EReg::T0 => Self::T0,
            EReg::T1 => Self::T1,
            EReg::T2 => Self::T2,
            EReg::S0 => Self::S0,
            EReg::S1 => Self::S1,
            EReg::A0 => Self::A0,
            EReg::A1 => Self::A1,
            EReg::A2 => Self::A2,
            EReg::A3 => Self::A3,
            EReg::A4 => Self::A4,
            EReg::A5 => Self::A5,
            EReg::Phantom(_) => {
                // SAFETY: Phantom register is never constructed
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

impl<Type> fmt::Display for Reg<Type> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero => write!(f, "zero"),
            Self::Ra => write!(f, "ra"),
            Self::Sp => write!(f, "sp"),
            Self::Gp => write!(f, "gp"),
            Self::Tp => write!(f, "tp"),
            Self::T0 => write!(f, "t0"),
            Self::T1 => write!(f, "t1"),
            Self::T2 => write!(f, "t2"),
            Self::S0 => write!(f, "s0"),
            Self::S1 => write!(f, "s1"),
            Self::A0 => write!(f, "a0"),
            Self::A1 => write!(f, "a1"),
            Self::A2 => write!(f, "a2"),
            Self::A3 => write!(f, "a3"),
            Self::A4 => write!(f, "a4"),
            Self::A5 => write!(f, "a5"),
            Self::A6 => write!(f, "a6"),
            Self::A7 => write!(f, "a7"),
            Self::S2 => write!(f, "s2"),
            Self::S3 => write!(f, "s3"),
            Self::S4 => write!(f, "s4"),
            Self::S5 => write!(f, "s5"),
            Self::S6 => write!(f, "s6"),
            Self::S7 => write!(f, "s7"),
            Self::S8 => write!(f, "s8"),
            Self::S9 => write!(f, "s9"),
            Self::S10 => write!(f, "s10"),
            Self::S11 => write!(f, "s11"),
            Self::T3 => write!(f, "t3"),
            Self::T4 => write!(f, "t4"),
            Self::T5 => write!(f, "t5"),
            Self::T6 => write!(f, "t6"),
            Self::Phantom(_) => {
                // SAFETY: Phantom register is never constructed
                unsafe { unreachable_unchecked() }
            }
        }
    }
}

impl<Type> fmt::Debug for Reg<Type> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<Type> const PartialEq for Reg<Type> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        // This is quite ugly, but there doesn't seem to be a much better way with `Phantom` variant
        matches!(
            (self, other),
            (Self::Zero, Self::Zero)
                | (Self::Ra, Self::Ra)
                | (Self::Sp, Self::Sp)
                | (Self::Gp, Self::Gp)
                | (Self::Tp, Self::Tp)
                | (Self::T0, Self::T0)
                | (Self::T1, Self::T1)
                | (Self::T2, Self::T2)
                | (Self::S0, Self::S0)
                | (Self::S1, Self::S1)
                | (Self::A0, Self::A0)
                | (Self::A1, Self::A1)
                | (Self::A2, Self::A2)
                | (Self::A3, Self::A3)
                | (Self::A4, Self::A4)
                | (Self::A5, Self::A5)
                | (Self::A6, Self::A6)
                | (Self::A7, Self::A7)
                | (Self::S2, Self::S2)
                | (Self::S3, Self::S3)
                | (Self::S4, Self::S4)
                | (Self::S5, Self::S5)
                | (Self::S6, Self::S6)
                | (Self::S7, Self::S7)
                | (Self::S8, Self::S8)
                | (Self::S9, Self::S9)
                | (Self::S10, Self::S10)
                | (Self::S11, Self::S11)
                | (Self::T3, Self::T3)
                | (Self::T4, Self::T4)
                | (Self::T5, Self::T5)
                | (Self::T6, Self::T6)
                | (Self::Phantom(_), Self::Phantom(_))
        )
    }
}

impl<Type> const Eq for Reg<Type> {}

impl const GenericRegisterInternal<u32> for Reg<u32> {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        matches!(self, Self::Zero)
    }

    #[inline(always)]
    fn offset(self) -> usize {
        // NOTE: `transmute()` is requited here, otherwise performance suffers A LOT for unknown
        // reason
        // SAFETY: Enum is `#[repr(u8)]` and doesn't have any fields
        usize::from(unsafe { core::mem::transmute::<Self, u8>(self) })
        // match self {
        //     Self::Zero => 0,
        //     Self::Ra => 1,
        //     Self::Sp => 2,
        //     Self::Gp => 3,
        //     Self::Tp => 4,
        //     Self::T0 => 5,
        //     Self::T1 => 6,
        //     Self::T2 => 7,
        //     Self::S0 => 8,
        //     Self::S1 => 9,
        //     Self::A0 => 10,
        //     Self::A1 => 11,
        //     Self::A2 => 12,
        //     Self::A3 => 13,
        //     Self::A4 => 14,
        //     Self::A5 => 15,
        //     Self::A6 => 16,
        //     Self::A7 => 17,
        //     Self::S2 => 18,
        //     Self::S3 => 19,
        //     Self::S4 => 20,
        //     Self::S5 => 21,
        //     Self::S6 => 22,
        //     Self::S7 => 23,
        //     Self::S8 => 24,
        //     Self::S9 => 25,
        //     Self::S10 => 26,
        //     Self::S11 => 27,
        //     Self::T3 => 28,
        //     Self::T4 => 29,
        //     Self::T5 => 30,
        //     Self::T6 => 31,
        //     Self::Phantom(_) => {
        //         // SAFETY: Phantom register is never constructed
        //         unsafe { unreachable_unchecked() }
        //     }
        // }
    }
}

impl const GenericRegisterInternal<u64> for Reg<u64> {
    #[inline(always)]
    fn is_zero(&self) -> bool {
        matches!(self, Self::Zero)
    }

    #[inline(always)]
    fn offset(self) -> usize {
        // NOTE: `transmute()` is requited here, otherwise performance suffers A LOT for unknown
        // reason
        // SAFETY: Enum is `#[repr(u8)]` and doesn't have any fields
        usize::from(unsafe { core::mem::transmute::<Self, u8>(self) })
        // match self {
        //     Self::Zero => 0,
        //     Self::Ra => 1,
        //     Self::Sp => 2,
        //     Self::Gp => 3,
        //     Self::Tp => 4,
        //     Self::T0 => 5,
        //     Self::T1 => 6,
        //     Self::T2 => 7,
        //     Self::S0 => 8,
        //     Self::S1 => 9,
        //     Self::A0 => 10,
        //     Self::A1 => 11,
        //     Self::A2 => 12,
        //     Self::A3 => 13,
        //     Self::A4 => 14,
        //     Self::A5 => 15,
        //     Self::A6 => 16,
        //     Self::A7 => 17,
        //     Self::S2 => 18,
        //     Self::S3 => 19,
        //     Self::S4 => 20,
        //     Self::S5 => 21,
        //     Self::S6 => 22,
        //     Self::S7 => 23,
        //     Self::S8 => 24,
        //     Self::S9 => 25,
        //     Self::S10 => 26,
        //     Self::S11 => 27,
        //     Self::T3 => 28,
        //     Self::T4 => 29,
        //     Self::T5 => 30,
        //     Self::T6 => 31,
        //     Self::Phantom(_) => {
        //         // SAFETY: Phantom register is never constructed
        //         unsafe { unreachable_unchecked() }
        //     }
        // }
    }
}

impl const GenericRegister for Reg<u32> {
    const N: usize = 32;
    type Type = u32;

    #[inline(always)]
    fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Zero),
            1 => Some(Self::Ra),
            2 => Some(Self::Sp),
            3 => Some(Self::Gp),
            4 => Some(Self::Tp),
            5 => Some(Self::T0),
            6 => Some(Self::T1),
            7 => Some(Self::T2),
            8 => Some(Self::S0),
            9 => Some(Self::S1),
            10 => Some(Self::A0),
            11 => Some(Self::A1),
            12 => Some(Self::A2),
            13 => Some(Self::A3),
            14 => Some(Self::A4),
            15 => Some(Self::A5),
            16 => Some(Self::A6),
            17 => Some(Self::A7),
            18 => Some(Self::S2),
            19 => Some(Self::S3),
            20 => Some(Self::S4),
            21 => Some(Self::S5),
            22 => Some(Self::S6),
            23 => Some(Self::S7),
            24 => Some(Self::S8),
            25 => Some(Self::S9),
            26 => Some(Self::S10),
            27 => Some(Self::S11),
            28 => Some(Self::T3),
            29 => Some(Self::T4),
            30 => Some(Self::T5),
            31 => Some(Self::T6),
            _ => None,
        }
    }
}

impl const GenericRegister for Reg<u64> {
    const N: usize = 32;
    type Type = u64;

    #[inline(always)]
    fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Self::Zero),
            1 => Some(Self::Ra),
            2 => Some(Self::Sp),
            3 => Some(Self::Gp),
            4 => Some(Self::Tp),
            5 => Some(Self::T0),
            6 => Some(Self::T1),
            7 => Some(Self::T2),
            8 => Some(Self::S0),
            9 => Some(Self::S1),
            10 => Some(Self::A0),
            11 => Some(Self::A1),
            12 => Some(Self::A2),
            13 => Some(Self::A3),
            14 => Some(Self::A4),
            15 => Some(Self::A5),
            16 => Some(Self::A6),
            17 => Some(Self::A7),
            18 => Some(Self::S2),
            19 => Some(Self::S3),
            20 => Some(Self::S4),
            21 => Some(Self::S5),
            22 => Some(Self::S6),
            23 => Some(Self::S7),
            24 => Some(Self::S8),
            25 => Some(Self::S9),
            26 => Some(Self::S10),
            27 => Some(Self::S11),
            28 => Some(Self::T3),
            29 => Some(Self::T4),
            30 => Some(Self::T5),
            31 => Some(Self::T6),
            _ => None,
        }
    }
}
