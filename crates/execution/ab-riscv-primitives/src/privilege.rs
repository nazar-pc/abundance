//! RISC-V privilege levels

/// Privilege level of the hart.
///
/// Variants are assigned their architectural 2-bit encoding as discriminants
/// so that `level as u8` yields the value that appears in CSR address bits
/// `[9:8]` and in `mstatus`/`sstatus` privilege fields.
///
/// The encoding `0b10` is architecturally reserved and is therefore absent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PrivilegeLevel {
    /// User / application mode (least privileged)
    User = 0b00,
    /// Supervisor mode
    Supervisor = 0b01,
    /// Machine mode (most privileged)
    #[default]
    Machine = 0b11,
}

impl PrivilegeLevel {
    /// Create a privilege level from its bit representation
    #[inline(always)]
    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0b00 => Some(Self::User),
            0b01 => Some(Self::Supervisor),
            0b11 => Some(Self::Machine),
            _ => None,
        }
    }
}
