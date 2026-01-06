#[cfg(test)]
mod tests;

use core::fmt;
use core::marker::Destruct;

/// Generic 64-bit register
pub const trait GenericRegister64:
    fmt::Display + fmt::Debug + [const] Destruct + Copy + Sized
{
    /// Create a register from its bit representation
    fn from_bits(bits: u8) -> Option<Self>;
}

/// A generic set of 64-bit registers
pub const trait GenericRegisters64<Reg>
where
    Reg: GenericRegister64,
{
    /// Read register value
    fn read(&self, reg: Reg) -> u64;

    /// Write register value
    fn write(&mut self, reg: Reg, value: u64);
}

/// A set of registers for RISC-V RV32E/RV64E
#[derive(Debug, Default, Clone, Copy)]
pub struct ERegisters64 {
    regs: [u64; 16],
}

impl const GenericRegisters64<EReg64> for ERegisters64 {
    #[inline(always)]
    fn read(&self, reg: EReg64) -> u64 {
        if matches!(reg, EReg64::Zero) {
            // Always zero
            return 0;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(reg.offset()) }
    }

    #[inline(always)]
    fn write(&mut self, reg: EReg64, value: u64) {
        if matches!(reg, EReg64::Zero) {
            // Writes are ignored
            return;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(reg.offset()) } = value;
    }
}

/// RISC-V register for RV64E.
///
/// For RV64I see [`Reg64`].
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum EReg64 {
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
}

impl fmt::Display for EReg64 {
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
        }
    }
}

impl fmt::Debug for EReg64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl const GenericRegister64 for EReg64 {
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

impl EReg64 {
    #[inline(always)]
    const fn offset(self) -> usize {
        usize::from(self as u8)
    }
}

/// A set of registers for RISC-V RV32I/RV64I
#[derive(Debug, Default, Clone, Copy)]
pub struct Registers64 {
    regs: [u64; 32],
}

impl const GenericRegisters64<Reg64> for Registers64 {
    #[inline(always)]
    fn read(&self, reg: Reg64) -> u64 {
        if matches!(reg, Reg64::Zero) {
            // Always zero
            return 0;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(reg.offset()) }
    }

    #[inline(always)]
    fn write(&mut self, reg: Reg64, value: u64) {
        if matches!(reg, Reg64::Zero) {
            // Writes are ignored
            return;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(reg.offset()) } = value;
    }
}

/// RISC-V register for RV64I.
///
/// For RV64E see [`EReg64`].
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Reg64 {
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
}

impl const From<EReg64> for Reg64 {
    #[inline(always)]
    fn from(reg: EReg64) -> Self {
        match reg {
            EReg64::Zero => Self::Zero,
            EReg64::Ra => Self::Ra,
            EReg64::Sp => Self::Sp,
            EReg64::Gp => Self::Gp,
            EReg64::Tp => Self::Tp,
            EReg64::T0 => Self::T0,
            EReg64::T1 => Self::T1,
            EReg64::T2 => Self::T2,
            EReg64::S0 => Self::S0,
            EReg64::S1 => Self::S1,
            EReg64::A0 => Self::A0,
            EReg64::A1 => Self::A1,
            EReg64::A2 => Self::A2,
            EReg64::A3 => Self::A3,
            EReg64::A4 => Self::A4,
            EReg64::A5 => Self::A5,
        }
    }
}

impl fmt::Display for Reg64 {
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
        }
    }
}

impl fmt::Debug for Reg64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl const GenericRegister64 for Reg64 {
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

impl Reg64 {
    #[inline(always)]
    const fn offset(self) -> usize {
        usize::from(self as u8)
    }
}
