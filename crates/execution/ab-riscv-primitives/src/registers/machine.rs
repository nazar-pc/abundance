//! Machine-mode registers

use crate::registers::general_purpose::{RegType, Register};

// TODO: CSR composition?
/// Machine CSR addresses (core mandatory registers from the Privileged Spec)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MCsr {
    /// Machine vendor ID register (MRO)
    Mvendorid = 0xF11,
    /// Machine architecture ID register (MRO)
    Marchid = 0xF12,
    /// Machine implementation ID register (MRO)
    Mimpid = 0xF13,
    /// Hart ID register (MRO)
    Mhartid = 0xF14,

    /// Machine status register (MRW)
    Mstatus = 0x300,
    /// Machine ISA and extensions register (MRW)
    Misa = 0x301,
    /// Machine interrupt-enable register (MRW)
    Mie = 0x304,
    /// Machine trap-vector base address register (MRW)
    Mtvec = 0x305,

    /// Machine scratch register (MRW)
    Mscratch = 0x340,
    /// Machine exception program counter (MRW)
    Mepc = 0x341,
    /// Machine trap cause (MRW)
    Mcause = 0x342,
    /// Machine trap value (MRW)
    Mtval = 0x343,
    /// Machine interrupt pending (MRW)
    Mip = 0x344,
}

impl MCsr {
    /// Try to match a CSR index to a machine CSR
    #[inline(always)]
    pub const fn from_index(index: u16) -> Option<Self> {
        match index {
            0xF11 => Some(Self::Mvendorid),
            0xF12 => Some(Self::Marchid),
            0xF13 => Some(Self::Mimpid),
            0xF14 => Some(Self::Mhartid),
            0x300 => Some(Self::Mstatus),
            0x301 => Some(Self::Misa),
            0x304 => Some(Self::Mie),
            0x305 => Some(Self::Mtvec),
            0x340 => Some(Self::Mscratch),
            0x341 => Some(Self::Mepc),
            0x342 => Some(Self::Mcause),
            0x343 => Some(Self::Mtval),
            0x344 => Some(Self::Mip),
            _ => None,
        }
    }
}

/// Machine exception causes (`mcause[XLEN‑1] = 0`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MCauseException {
    /// Instruction address misaligned
    InstructionAddressMisaligned = 0,
    /// Instruction access fault
    InstructionAccessFault = 1,
    /// Illegal instruction
    IllegalInstruction = 2,
    /// Breakpoint
    Breakpoint = 3,
    /// Load address misaligned
    LoadAddressMisaligned = 4,
    /// Load access fault
    LoadAccessFault = 5,
    /// Store/AMO address misaligned
    StoreAddressMisaligned = 6,
    /// Store/AMO access fault
    StoreAccessFault = 7,
    /// Environment call from U-mode
    UserEnvironmentCall = 8,
    /// Environment call from S-mode
    SupervisorEnvironmentCall = 9,
    /// Environment call from M-mode
    MachineEnvironmentCall = 11,
    /// Instruction page fault
    InstructionPageFault = 12,
    /// Load page fault
    LoadPageFault = 13,
    /// Store/AMO page fault
    StorePageFault = 15,
}

impl MCauseException {
    /// Try to match an exception code to an [`MCauseException`] (returns `None` for
    /// reserved/unknown codes)
    #[inline(always)]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::InstructionAddressMisaligned),
            1 => Some(Self::InstructionAccessFault),
            2 => Some(Self::IllegalInstruction),
            3 => Some(Self::Breakpoint),
            4 => Some(Self::LoadAddressMisaligned),
            5 => Some(Self::LoadAccessFault),
            6 => Some(Self::StoreAddressMisaligned),
            7 => Some(Self::StoreAccessFault),
            8 => Some(Self::UserEnvironmentCall),
            9 => Some(Self::SupervisorEnvironmentCall),
            11 => Some(Self::MachineEnvironmentCall),
            12 => Some(Self::InstructionPageFault),
            13 => Some(Self::LoadPageFault),
            15 => Some(Self::StorePageFault),
            _ => None,
        }
    }

    /// Convert this exception to its full raw `mcause` CSR value
    #[inline(always)]
    pub const fn to_raw<Reg>(self) -> Reg::Type
    where
        Reg: [const] Register,
    {
        Reg::Type::from(self as u32)
    }
}

/// Machine interrupt causes (`mcause[XLEN‑1] = 1`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MCauseInterrupt {
    /// User software interrupt
    UserSoftware = 0,
    /// Supervisor software interrupt
    SupervisorSoftware = 1,
    /// Machine software interrupt
    MachineSoftware = 3,
    /// User timer interrupt
    UserTimer = 4,
    /// Supervisor timer interrupt
    SupervisorTimer = 5,
    /// Machine timer interrupt
    MachineTimer = 7,
    /// User external interrupt
    UserExternal = 8,
    /// Supervisor external interrupt
    SupervisorExternal = 9,
    /// Machine external interrupt
    MachineExternal = 11,
}

impl MCauseInterrupt {
    /// Try to match an interrupt code to an `MInterrupt` (returns `None` for reserved/unknown
    /// codes)
    #[inline(always)]
    pub const fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::UserSoftware),
            1 => Some(Self::SupervisorSoftware),
            3 => Some(Self::MachineSoftware),
            4 => Some(Self::UserTimer),
            5 => Some(Self::SupervisorTimer),
            7 => Some(Self::MachineTimer),
            8 => Some(Self::UserExternal),
            9 => Some(Self::SupervisorExternal),
            11 => Some(Self::MachineExternal),
            _ => None,
        }
    }

    /// Convert this interrupt to its full raw `mcause` CSR value
    #[inline(always)]
    pub const fn to_raw<Reg>(self) -> Reg::Type
    where
        Reg: [const] Register,
    {
        Reg::Type::from(self as u32) | (Reg::Type::from(1u8) << (Reg::XLEN - 1))
    }
}

/// Combined `mcause` CSR value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MCause {
    Exception(MCauseException),
    Interrupt(MCauseInterrupt),
}

impl From<MCauseException> for MCause {
    #[inline(always)]
    fn from(cause: MCauseException) -> Self {
        Self::Exception(cause)
    }
}

impl From<MCauseInterrupt> for MCause {
    #[inline(always)]
    fn from(cause: MCauseInterrupt) -> Self {
        Self::Interrupt(cause)
    }
}

impl MCause {
    /// Try to create `MCause` from a raw `mcause` CSR value
    #[inline(always)]
    pub const fn from_raw<Reg>(raw: Reg::Type) -> Option<Self>
    where
        Reg: [const] Register,
    {
        let raw = raw.as_u64();
        let is_interrupt = (raw & (1u64 << (Reg::XLEN - 1))) != 0;
        let code = raw & !(1u64 << (Reg::XLEN - 1));

        if is_interrupt {
            MCauseInterrupt::from_code(code).map(Self::Interrupt)
        } else {
            MCauseException::from_code(code).map(Self::Exception)
        }
    }

    /// Convert this `MCause` back to the full raw `mcause` CSR value
    #[inline(always)]
    pub const fn to_raw<Reg>(self) -> Reg::Type
    where
        Reg: [const] Register,
    {
        match self {
            MCause::Exception(exception) => exception.to_raw::<Reg>(),
            MCause::Interrupt(interrupt) => interrupt.to_raw::<Reg>(),
        }
    }
}
