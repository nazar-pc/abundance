//! V extension

pub mod zvexx;

use crate::registers::general_purpose::{RegType, Register};
use core::fmt;
use core::hint::cold_path;

/// `mstatus.VS` / `sstatus.VS` / `vsstatus.VS` field encoding.
///
/// Context status for the vector extension, analogous to `mstatus.FS`.
/// Located at bits `[10:9]` in the respective status registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VsStatus {
    /// Vector unit is off; any vector instruction or CSR access raises illegal instruction
    Off = 0,
    /// Vector state is known to be in its initial state
    Initial = 1,
    /// Vector state is potentially modified but matches the last saved state
    Clean = 2,
    /// Vector state has been modified since the last save
    Dirty = 3,
}

impl VsStatus {
    /// Decode from a 2-bit field value
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0 => Self::Off,
            1 => Self::Initial,
            2 => Self::Clean,
            _ => Self::Dirty,
        }
    }

    /// Encode to a 2-bit field value
    #[inline(always)]
    pub const fn to_bits(self) -> u8 {
        self as u8
    }
}

/// Vector length multiplier (LMUL) setting.
///
/// Encoded in `vtype[2:0]` as a signed 3-bit value.
/// `LMUL = 2^vlmul` where `vlmul` is sign-extended. Positive values give integer multipliers,
/// negative values give fractional.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Vlmul {
    /// LMUL = 1 (`vlmul` encoding 0b000)
    M1 = 0b000,
    /// LMUL = 2 (`vlmul` encoding 0b001)
    M2 = 0b001,
    /// LMUL = 4 (`vlmul` encoding 0b010)
    M4 = 0b010,
    /// LMUL = 8 (`vlmul` encoding 0b011)
    M8 = 0b011,
    /// LMUL = 1/8 (`vlmul` encoding 0b101)
    Mf8 = 0b101,
    /// LMUL = 1/4 (`vlmul` encoding 0b110)
    Mf4 = 0b110,
    /// LMUL = 1/2 (`vlmul` encoding 0b111)
    Mf2 = 0b111,
}

impl Vlmul {
    /// Decode from the 3-bit `vlmul` field. Returns `None` for reserved encoding 0b100.
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0b111 {
            0b000 => Some(Self::M1),
            0b001 => Some(Self::M2),
            0b010 => Some(Self::M4),
            0b011 => Some(Self::M8),
            0b101 => Some(Self::Mf8),
            0b110 => Some(Self::Mf4),
            0b111 => Some(Self::Mf2),
            _ => None,
        }
    }

    /// Encode to the 3-bit `vlmul` field
    #[inline(always)]
    pub const fn to_bits(self) -> u8 {
        self as u8
    }

    /// Compute `VLMAX = LMUL * VLEN / SEW`.
    ///
    /// For fractional LMUL, this is `VLEN / (SEW * denominator)`.
    /// Returns 0 when the result would be less than 1 (insufficient bits).
    #[inline(always)]
    pub const fn vlmax<const VLEN: u32>(self, sew: Vsew) -> u32 {
        let sew_bits = u32::from(sew.bits_width());
        match self {
            Self::M1 => VLEN / sew_bits,
            Self::M2 => (VLEN * 2) / sew_bits,
            Self::M4 => (VLEN * 4) / sew_bits,
            Self::M8 => (VLEN * 8) / sew_bits,
            Self::Mf2 => VLEN / (sew_bits * 2),
            Self::Mf4 => VLEN / (sew_bits * 4),
            Self::Mf8 => VLEN / (sew_bits * 8),
        }
    }

    /// Number of vector registers occupied by one register group at this `LMUL`.
    ///
    /// Fractional `LMUL` values (`Mf2`, `Mf4`, `Mf8`) each occupy exactly 1 register.
    /// Integer `LMUL` values occupy 1, 2, 4, or 8 registers respectively.
    #[inline(always)]
    pub const fn register_count(self) -> u8 {
        match self {
            Self::Mf8 | Self::Mf4 | Self::Mf2 | Self::M1 => 1,
            Self::M2 => 2,
            Self::M4 => 4,
            Self::M8 => 8,
        }
    }

    /// LMUL as a `(numerator, denominator)` fraction where `LMUL = num / den`.
    ///
    /// Both values are powers of two with exactly one equal to `1`. Useful for computing
    /// `EMUL = (EEW / SEW) * LMUL` without floating-point arithmetic.
    #[inline(always)]
    pub const fn as_fraction(self) -> (u8, u8) {
        match self {
            Self::Mf8 => (1, 8),
            Self::Mf4 => (1, 4),
            Self::Mf2 => (1, 2),
            Self::M1 => (1, 1),
            Self::M2 => (2, 1),
            Self::M4 => (4, 1),
            Self::M8 => (8, 1),
        }
    }

    /// Compute `EMUL` for an indexed load: `EMUL = (index_eew / sew) * LMUL`.
    ///
    /// Returns the register count for the index register group, or `None` when `EMUL` falls
    /// outside the legal range `[1/8, 8]`.
    #[inline(always)]
    pub const fn index_register_count(self, index_eew: Eew, sew: Vsew) -> Option<u8> {
        let (lmul_num, lmul_den) = self.as_fraction();
        let num = u16::from(index_eew.bits_width()) * u16::from(lmul_num);
        let den = u16::from(sew.bits_width()) * u16::from(lmul_den);
        // Both are products of powers of two; GCD equals the smaller value.
        let g = if num < den { num } else { den };
        let (n, d) = (num / g, den / g);
        // Legal EMUL fractions: 1/8, 1/4, 1/2, 1, 2, 4, 8
        let legal = matches!(
            (n, d),
            (1, 8) | (1, 4) | (1, 2) | (1, 1) | (2, 1) | (4, 1) | (8, 1)
        );
        if !legal {
            cold_path();
            return None;
        }
        // Register count is max(1, n/d) = n when d==1, else 1
        Some(if d > 1 { 1 } else { n as u8 })
    }

    /// Compute EMUL for a data operand of a memory instruction with a given effective element
    /// width: `EMUL = (eew / sew) * LMUL`.
    ///
    /// Mathematically identical to [`Self::index_register_count`], but exposed under a distinct
    /// name for call sites where the EEW describes the *data* being loaded or stored rather than an
    /// index. Keeping the two entry points separate avoids accidental semantic drift if
    /// one of them is later specialised.
    ///
    /// Returns `None` when the resulting EMUL falls outside the legal range `[1/8, 8]`.
    #[inline(always)]
    pub const fn data_register_count(self, eew: Eew, sew: Vsew) -> Option<u8> {
        self.index_register_count(eew, sew)
    }
}

impl fmt::Display for Vlmul {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::M1 => write!(f, "m1"),
            Self::M2 => write!(f, "m2"),
            Self::M4 => write!(f, "m4"),
            Self::M8 => write!(f, "m8"),
            Self::Mf8 => write!(f, "mf8"),
            Self::Mf4 => write!(f, "mf4"),
            Self::Mf2 => write!(f, "mf2"),
        }
    }
}

/// Factor by which Vsew width is divided
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VsewFactor {
    /// Divide width by 2
    F2 = 2,
    /// Divide width by 4
    F4 = 4,
    /// Divide width by 8
    F8 = 8,
}

impl VsewFactor {
    /// Return the numeric divisor used to scale down a [`Vsew`] bit-width
    #[inline(always)]
    pub const fn factor(self) -> u8 {
        self as u8
    }
}

/// Selected element width (SEW).
///
/// Encoded in `vtype[5:3]` as `vsew`. `SEW = 8 * 2^vsew`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Vsew {
    /// SEW = 8 bits (vsew = 0b000)
    E8 = 8,
    /// SEW = 16 bits (vsew = 0b001)
    E16 = 16,
    /// SEW = 32 bits (vsew = 0b010)
    E32 = 32,
    /// SEW = 64 bits (vsew = 0b011)
    E64 = 64,
}

impl Vsew {
    /// Decode from the 3-bit vsew field. Returns `None` for reserved encodings.
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits & 0b111 {
            0b000 => Some(Self::E8),
            0b001 => Some(Self::E16),
            0b010 => Some(Self::E32),
            0b011 => Some(Self::E64),
            _ => {
                cold_path();
                None
            }
        }
    }

    /// Encode to the 3-bit vsew field
    #[inline(always)]
    pub const fn to_bits(self) -> u8 {
        match self {
            Vsew::E8 => 0b000,
            Vsew::E16 => 0b001,
            Vsew::E32 => 0b010,
            Vsew::E64 => 0b011,
        }
    }

    /// Get the double element width, if available
    #[inline(always)]
    pub const fn double_width(self) -> Option<Self> {
        match self {
            Self::E8 => Some(Self::E16),
            Self::E16 => Some(Self::E32),
            Self::E32 => Some(Self::E64),
            Self::E64 => {
                cold_path();
                None
            }
        }
    }

    /// Divide Vsew width by a given factor
    #[inline(always)]
    pub const fn divide_by_factor(self, factor: VsewFactor) -> Option<Self> {
        let Some(divide_by_factor) = self.bits_width().div_exact(factor.factor()) else {
            cold_path();
            return None;
        };
        match divide_by_factor {
            8 => Some(Self::E8),
            16 => Some(Self::E16),
            32 => Some(Self::E32),
            _ => {
                cold_path();
                None
            }
        }
    }

    /// Element width in bits
    #[inline(always)]
    pub const fn bits_width(self) -> u8 {
        match self {
            Self::E8 => 8,
            Self::E16 => 16,
            Self::E32 => 32,
            Self::E64 => 64,
        }
    }

    /// Element width in bytes
    #[inline(always)]
    pub const fn bytes_width(self) -> u8 {
        match self {
            Self::E8 => 1,
            Self::E16 => 2,
            Self::E32 => 4,
            Self::E64 => 8,
        }
    }

    /// Convert to the corresponding `Eew` variant.
    ///
    /// Every valid `Vsew` value has a directly corresponding `Eew` value because both
    /// enumerate the same set of widths (8/16/32/64 bits). The conversion is always
    /// successful.
    #[inline(always)]
    pub const fn as_eew(self) -> Eew {
        match self {
            Self::E8 => Eew::E8,
            Self::E16 => Eew::E16,
            Self::E32 => Eew::E32,
            Self::E64 => Eew::E64,
        }
    }
}

impl fmt::Display for Vsew {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::E8 => write!(f, "e8"),
            Self::E16 => write!(f, "e16"),
            Self::E32 => write!(f, "e32"),
            Self::E64 => write!(f, "e64"),
        }
    }
}

/// Effective element width for vector memory operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Eew {
    /// 8-bit elements
    E8 = 1,
    /// 16-bit elements
    E16 = 2,
    /// 32-bit elements
    E32 = 4,
    /// 64-bit elements
    E64 = 8,
}

impl Eew {
    /// Max element width in bytes
    pub const MAX_BYTES: u8 = Self::E64.bytes_width();

    /// Decode the width field into an element width
    #[inline(always)]
    pub const fn from_width(width: u8) -> Option<Self> {
        match width {
            0b000 => Some(Self::E8),
            0b101 => Some(Self::E16),
            0b110 => Some(Self::E32),
            0b111 => Some(Self::E64),
            _ => {
                cold_path();
                None
            }
        }
    }

    /// Encode to the 3-bit Eew field
    #[inline(always)]
    pub const fn to_bits(self) -> u8 {
        match self {
            Eew::E8 => 0b000,
            Eew::E16 => 0b101,
            Eew::E32 => 0b110,
            Eew::E64 => 0b111,
        }
    }

    /// Element width in bits
    #[inline(always)]
    pub const fn bits_width(self) -> u8 {
        match self {
            Self::E8 => 8,
            Self::E16 => 16,
            Self::E32 => 32,
            Self::E64 => 64,
        }
    }

    /// Element width in bytes.
    ///
    /// Guaranteed to be `<= Self::MAX_BYTES`.
    #[inline(always)]
    pub const fn bytes_width(self) -> u8 {
        match self {
            Self::E8 => 1,
            Self::E16 => 2,
            Self::E32 => 4,
            Self::E64 => 8,
        }
    }
}

impl fmt::Display for Eew {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.bits_width(), f)
    }
}

/// Vector fixed-point rounding mode.
///
/// Encoded in the `vxrm` CSR bits `[1:0]` and mirrored in `vcsr[2:1]`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Vxrm {
    /// Round-to-nearest-up (rnu)
    #[default]
    Rnu = 0b00,
    /// Round-to-nearest-even (rne)
    Rne = 0b01,
    /// Round-down / truncate (rdn)
    Rdn = 0b10,
    /// Round-to-odd (rod)
    Rod = 0b11,
}

impl Vxrm {
    /// Decode from a 2-bit field
    #[inline(always)]
    pub const fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => Self::Rnu,
            0b01 => Self::Rne,
            0b10 => Self::Rdn,
            _ => Self::Rod,
        }
    }

    /// Encode to a 2-bit field
    #[inline(always)]
    pub const fn to_bits(self) -> u8 {
        self as u8
    }
}

/// Decoded `vtype` register contents.
///
/// The vtype CSR controls the interpretation of the vector register file: element width, register
/// grouping, and tail/mask agnostic policies.
///
/// The raw encoding is XLEN-dependent (vill is at bit XLEN-1), but this decoded form is
/// XLEN-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vtype<const ELEN: u32, const VLEN: u32> {
    /// Vector mask agnostic policy (bit `7`)
    vma: bool,
    /// Vector tail agnostic policy (bit `6`)
    vta: bool,
    /// Selected element width (bits `[5:3]`)
    vsew: Vsew,
    /// Vector length multiplier (bits `[2:0]`)
    vlmul: Vlmul,
}

impl<const ELEN: u32, const VLEN: u32> Vtype<ELEN, VLEN> {
    /// Vector mask agnostic policy (bit `7`)
    pub const fn vma(&self) -> bool {
        self.vma
    }

    /// Vector tail agnostic policy (bit `6`)
    pub const fn vta(&self) -> bool {
        self.vta
    }

    /// Selected element width (bits `[5:3]`)
    pub const fn vsew(&self) -> Vsew {
        self.vsew
    }

    /// Vector length multiplier (bits `[2:0]`)
    pub const fn vlmul(&self) -> Vlmul {
        self.vlmul
    }

    /// Decode from raw register value.
    ///
    /// The `XLEN` is taken from `Reg::XLEN` and must be 32 for RV32 or 64 for RV64. The `vill` bit
    /// is placed at bit position `Reg::XLEN - 1`.
    ///
    /// All bits in `[Reg::XLEN-1:8]` must be zero; non-zero bits indicate an unrecognized
    /// encoding and cause `None` to be returned (this includes `vill`).
    #[inline(always)]
    pub const fn from_raw<Reg>(raw: Reg::Type) -> Option<Self>
    where
        Reg: [const] Register,
    {
        let raw = raw.as_u64();

        // All bits in [XLEN-1:8] must be zero
        if (raw >> 8u8) != 0 {
            cold_path();
            return None;
        }

        let vlmul_bits = (raw & 0b111) as u8;
        let vsew_bits = ((raw >> 3u8) & 0b111) as u8;
        let vta = ((raw >> 6u8) & 1) != 0;
        let vma = ((raw >> 7u8) & 1) != 0;

        let Some(vlmul) = Vlmul::from_bits(vlmul_bits) else {
            cold_path();
            return None;
        };
        let Some(vsew) = Vsew::from_bits(vsew_bits) else {
            cold_path();
            return None;
        };

        let sew = vsew.bits_width();
        if u32::from(sew) > ELEN {
            cold_path();
            return None;
        }

        if vlmul.vlmax::<VLEN>(vsew) == 0 {
            cold_path();
            return None;
        }

        Some(Self {
            vma,
            vta,
            vsew,
            vlmul,
        })
    }

    /// Encode to a raw `vtype` register value of type `Reg::Type`.
    ///
    /// The encoded value contains `vlmul`, `vsew`, `vta`, and `vma` in bits `[7:0]` with
    /// `vill = 0`. To construct a raw value with `vill = 1` (illegal configuration), use
    /// [`Self::illegal_raw`].
    #[inline(always)]
    pub const fn to_raw<Reg>(self) -> Reg::Type
    where
        Reg: [const] Register,
    {
        let mut raw = 0u8;
        raw |= self.vlmul.to_bits();
        raw |= self.vsew.to_bits() << 3u8;

        if self.vta {
            raw |= 1 << 6u8;
        }

        if self.vma {
            raw |= 1 << 7u8;
        }

        Reg::Type::from(raw)
    }

    /// Construct a raw value for `vtype` with `vill=1` (illegal configuration).
    ///
    /// Per spec: when `vill` is set, the remaining bits are zero and `vl` is also set to zero. Any
    /// subsequent vector instruction that depends on `vtype` will raise an illegal-instruction
    /// exception.
    #[inline(always)]
    pub const fn illegal_raw<Reg>() -> Reg::Type
    where
        Reg: [const] Register,
    {
        let vill_bit = Reg::XLEN - 1;
        Reg::Type::from(1u8) << vill_bit
    }
}
