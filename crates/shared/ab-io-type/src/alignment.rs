//! Alignment of a type or value, in its own module so that only its constructors can create it

use crate::trivial_type::TrivialType;
use core::hint;

/// Alignment of a type or value.
///
/// It is stored as its power of two, so the compiler knows that the alignment in bytes is a power
/// of two, and code that uses it can compute padding with plain `%` that compiles to bit masks.
/// Alignments compare like their values in bytes.
#[derive(Debug, Copy, Clone)]
#[derive_const(PartialEq, Eq, PartialOrd, Ord)]
pub struct Alignment {
    /// Power of two of the alignment, at most the power of [`Alignment::MAX`]
    pub mut(self) power: u8,
}

impl Alignment {
    /// The largest alignment supported
    pub const MAX: Self = {
        let Ok(power) = u8::try_from(16u8.ilog2()) else {
            unreachable!();
        };
        Self { power }
    };

    /// Alignment from its power of two, `None` if it would be larger than [`Alignment::MAX`]
    #[inline(always)]
    pub const fn from_power(power: u8) -> Option<Self> {
        if power > Self::MAX.power {
            return None;
        }

        Some(Self { power })
    }

    /// Alignment of `T`.
    ///
    /// A call doesn't compile for a `T` aligned to more than [`Alignment::MAX`].
    #[inline(always)]
    pub const fn of<T>() -> Self
    where
        T: TrivialType,
    {
        const {
            let Ok(power) = u8::try_from(align_of::<T>().ilog2()) else {
                unreachable!();
            };
            Self::from_power(power).expect("Alignment of the type is larger than `Alignment::MAX`")
        }
    }

    /// The alignment in bytes
    #[inline(always)]
    pub const fn get(self) -> u8 {
        // SAFETY: The field is only set in this module (`mut(self)`), where `MAX` holds the power
        // of `Alignment::MAX` and `from_power()` only accepts powers up to it. This tells the
        // compiler that the alignment is at most `Alignment::MAX`, also when it was read from
        // memory.
        unsafe {
            hint::assert_unchecked(self.power <= Self::MAX.power);
        }

        1 << self.power
    }
}
