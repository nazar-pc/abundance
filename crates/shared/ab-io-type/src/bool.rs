use crate::metadata::IoTypeMetadataKind;
use crate::trivial_type::TrivialType;
use core::ops::Not;

/// Just like `bool`, but any bit pattern is valid.
///
/// For `bool` only `0` and `1` are valid bit patterns out of 256 possible, anything else is
/// undefined behavior. This type changes that by treating `0` as `false` and everything else as
/// `true`, making it safer to use and allowing it to implement `TrivialType`.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Bool {
    byte: u8,
}

impl Not for Bool {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self::Output {
        Self {
            byte: (self.byte == 0) as u8,
        }
    }
}

impl From<bool> for Bool {
    #[inline(always)]
    fn from(value: bool) -> Self {
        Self::new(value)
    }
}

impl From<Bool> for bool {
    #[inline(always)]
    fn from(value: Bool) -> Self {
        value.get()
    }
}

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl TrivialType for Bool {
    const METADATA: &[u8] = &[IoTypeMetadataKind::Bool as u8];
}

impl Bool {
    /// Create a new instance from existing boolean value
    #[inline(always)]
    pub const fn new(value: bool) -> Self {
        Self { byte: value as u8 }
    }

    /// Get the value
    #[inline(always)]
    pub const fn get(&self) -> bool {
        self.byte != 0
    }

    /// Set new value
    #[inline(always)]
    pub const fn set(&mut self, value: bool) {
        self.byte = value as u8;
    }
}
