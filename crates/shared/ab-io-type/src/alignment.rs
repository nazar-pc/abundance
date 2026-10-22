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
    /// The largest alignment supported, 16 bytes.
    ///
    /// No [`TrivialType`] has a larger alignment.
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

    /// Alignment of `T` in metadata, see [`TrivialType::ALIGNMENT`].
    ///
    /// [`TrivialType`] requires an alignment of at most [`Alignment::MAX`], and a call doesn't
    /// compile for an implementation that breaks this requirement. It also doesn't compile when
    /// `T::ALIGNMENT` is smaller than `align_of::<T>()`.
    #[inline(always)]
    pub const fn of<T>() -> Self
    where
        T: TrivialType,
    {
        const {
            assert!(
                align_of::<T>() <= usize::from(T::ALIGNMENT.get()),
                "Alignment of the type is larger than `TrivialType::ALIGNMENT`"
            );
            T::ALIGNMENT
        }
    }

    /// The alignment in bytes
    #[inline(always)]
    pub const fn get(self) -> u8 {
        // SAFETY: The field is only set in this module (`mut(self)`), by the definition of `MAX`
        // and by `from_power()`, which rejects powers larger than that of `MAX`. This tells the
        // compiler that the alignment is at most `Alignment::MAX`, also when it was read from
        // memory.
        unsafe {
            hint::assert_unchecked(self.power <= Self::MAX.power);
        }

        1 << self.power
    }
}
