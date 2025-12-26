use core::fmt;

#[derive(Debug, Default, Clone, Copy)]
pub struct Registers {
    regs: [u64; 16],
}

impl Registers {
    pub const fn new(ra: u64, sp: u64, gp: u64, a0: u64) -> Self {
        let mut instance = Self { regs: [0; _] };

        instance.write(Reg::Ra, ra);
        instance.write(Reg::Sp, sp);
        instance.write(Reg::Gp, gp);
        instance.write(Reg::A0, a0);

        instance
    }

    #[inline(always)]
    pub const fn read(&self, reg: Reg) -> u64 {
        if matches!(reg, Reg::Zero) {
            // Always zero
            return 0;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked(reg.offset()) }
    }

    #[inline(always)]
    pub const fn write(&mut self, reg: Reg, value: u64) {
        if matches!(reg, Reg::Zero) {
            // Writes are ignored
            return;
        }

        // SAFETY: register offset is always within bounds
        *unsafe { self.regs.get_unchecked_mut(reg.offset()) } = value;
    }
}

// Define the RISC-V registers for RV64E
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
pub enum Reg {
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

impl fmt::Display for Reg {
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

impl Reg {
    #[inline(always)]
    const fn offset(self) -> usize {
        (self as u8) as usize
    }

    #[inline(always)]
    pub const fn from_bits(bits: u32) -> Option<Self> {
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
