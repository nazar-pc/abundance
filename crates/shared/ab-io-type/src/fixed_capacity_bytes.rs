use crate::metadata::{IoTypeMetadataKind, MAX_METADATA_CAPACITY, concat_metadata_sources};
use crate::trivial_type::TrivialType;

/// Ensures supported capacity of [`FixedCapacityBytesU8`] and [`FixedCapacityStringU8`]
///
/// [`FixedCapacityStringU8`]: crate::fixed_capacity_string::FixedCapacityStringU8
pub(crate) const SUPPORTED_CAPACITY_U8<const CAPACITY: usize>: usize = {
    assert!(
        CAPACITY <= u8::MAX as usize,
        "Capacity must not exceed `u8::MAX`"
    );
    0
};
/// Ensures supported capacity of [`FixedCapacityBytesU16`] and [`FixedCapacityStringU16`]
///
/// [`FixedCapacityStringU16`]: crate::fixed_capacity_string::FixedCapacityStringU16
pub(crate) const SUPPORTED_CAPACITY_U16<const CAPACITY: usize>: usize = {
    assert!(
        CAPACITY <= u16::MAX as usize,
        "Capacity must not exceed `u16::MAX`"
    );
    // Odd capacity results in a padding byte after the contents, which `TrivialType` doesn't allow
    assert!(
        CAPACITY.is_multiple_of(2),
        "Capacity must be a multiple of 2"
    );
    0
};

/// Container for storing a number of bytes limited by the specified fixed capacity as `u8`.
///
/// See also [`FixedCapacityBytesU16`] if you need to store more bytes.
///
/// `CAPACITY` must not exceed `u8::MAX`.
///
/// In contrast to [`VariableBytes`], which can store arbitrary amount of data and can change the
/// capacity, this container has fixed predefined capacity and occupies it regardless of how many
/// bytes are actually stored inside. This might seem limiting but allows implementing
/// [`TrivialType`] trait, enabling its use for fields in data structures that derive
/// [`TrivialType`] themselves, which isn't the case with [`VariableBytes`].
///
/// [`VariableBytes`]: crate::variable_bytes::VariableBytes
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityBytesU8<const CAPACITY: usize> {
    len: u8,
    bytes: [u8; CAPACITY],
}

impl<const CAPACITY: usize> Default for FixedCapacityBytesU8<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U8::<CAPACITY>]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            len: 0,
            bytes: [0; CAPACITY],
        }
    }
}

// SAFETY: Any bit pattern is valid, so it is safe to implement `TrivialType` for this type
unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityBytesU8<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U8::<CAPACITY>]:,
{
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: usize) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[&[
                IoTypeMetadataKind::FixedCapacityBytes8b as u8,
                capacity as u8,
            ]])
        }
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };
}

impl<const CAPACITY: usize> FixedCapacityBytesU8<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U8::<CAPACITY>]:,
{
    /// Try to create an instance from provided bytes.
    ///
    /// Returns `None` if provided bytes do not fit into the capacity.
    #[inline(always)]
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > CAPACITY {
            return None;
        }

        Some(Self {
            len: bytes.len() as u8,
            bytes: {
                let mut buffer = [0; CAPACITY];
                buffer[..bytes.len()].copy_from_slice(bytes);
                buffer
            },
        })
    }

    /// Access to stored bytes
    #[inline(always)]
    pub fn get_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// Exclusive access to stored bytes
    #[inline(always)]
    pub fn get_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.len as usize]
    }

    /// Number of stored bytes
    #[inline(always)]
    pub const fn len(&self) -> u8 {
        self.len
    }

    /// Returns `true` if length is zero
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append some bytes.
    ///
    /// `true` is returned on success, but if there isn't enough capacity left, `false` is.
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let len = self.len();
        if bytes.len() + len as usize > CAPACITY {
            return false;
        }

        self.bytes[..bytes.len()].copy_from_slice(bytes);

        true
    }

    /// Truncate stored bytes to this length.
    ///
    /// Returns `true` on success or `false` if `new_len` is larger than [`Self::len()`].
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn truncate(&mut self, new_len: u8) -> bool {
        if new_len > self.len() {
            return false;
        }

        self.len = new_len;

        true
    }

    /// Copy from specified bytes.
    ///
    /// Returns `false` if capacity is not enough to copy contents of `src`
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn copy_from<T>(&mut self, src: &[u8]) -> bool {
        if src.len() > CAPACITY {
            return false;
        }

        self.bytes[..src.len()].copy_from_slice(src);
        self.len = src.len() as u8;

        true
    }
}

/// Container for storing a number of bytes limited by the specified fixed capacity as `u16`.
///
/// See also [`FixedCapacityBytesU8`] if you need to store fewer bytes.
///
/// `CAPACITY` must not exceed `u16::MAX` and must be a multiple of 2 to avoid padding.
///
/// In contrast to [`VariableBytes`], which can store arbitrary amount of data and can change the
/// capacity, this container has fixed predefined capacity and occupies it regardless of how many
/// bytes are actually stored inside. This might seem limiting but allows implementing
/// [`TrivialType`] trait, enabling its use for fields in data structures that derive
/// [`TrivialType`] themselves, which isn't the case with [`VariableBytes`].
///
/// [`VariableBytes`]: crate::variable_bytes::VariableBytes
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct FixedCapacityBytesU16<const CAPACITY: usize> {
    len: u16,
    bytes: [u8; CAPACITY],
}

impl<const CAPACITY: usize> Default for FixedCapacityBytesU16<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U16::<CAPACITY>]:,
{
    #[inline(always)]
    fn default() -> Self {
        Self {
            len: 0,
            bytes: [0; CAPACITY],
        }
    }
}

// SAFETY: Any bit pattern is valid and there is no padding since capacity is a multiple of 2, so it
// is safe to implement `TrivialType` for this type
unsafe impl<const CAPACITY: usize> TrivialType for FixedCapacityBytesU16<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U16::<CAPACITY>]:,
{
    const METADATA: &[u8] = {
        #[inline(always)]
        const fn metadata(capacity: usize) -> ([u8; MAX_METADATA_CAPACITY], usize) {
            concat_metadata_sources(&[
                &[IoTypeMetadataKind::FixedCapacityBytes16b as u8],
                &(capacity as u16).to_le_bytes(),
            ])
        }
        metadata(CAPACITY).0.split_at(metadata(CAPACITY).1).0
    };
}

impl<const CAPACITY: usize> FixedCapacityBytesU16<CAPACITY>
where
    [(); SUPPORTED_CAPACITY_U16::<CAPACITY>]:,
{
    /// Try to create an instance from provided bytes.
    ///
    /// Returns `None` if provided bytes do not fit into the capacity.
    #[inline(always)]
    pub fn try_from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > CAPACITY {
            return None;
        }

        Some(Self {
            len: bytes.len() as u16,
            bytes: {
                let mut buffer = [0; CAPACITY];
                buffer[..bytes.len()].copy_from_slice(bytes);
                buffer
            },
        })
    }

    /// Access to stored bytes
    #[inline(always)]
    pub fn get_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// Exclusive access to stored bytes
    #[inline(always)]
    pub fn get_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..self.len as usize]
    }

    /// Number of stored bytes
    #[inline(always)]
    pub const fn len(&self) -> u16 {
        self.len
    }

    /// Returns `true` if length is zero
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append some bytes.
    ///
    /// `true` is returned on success, but if there isn't enough capacity left, `false` is.
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn append(&mut self, bytes: &[u8]) -> bool {
        let len = self.len();
        if bytes.len() + len as usize > CAPACITY {
            return false;
        }

        self.bytes[..bytes.len()].copy_from_slice(bytes);

        true
    }

    /// Truncate stored bytes to this length.
    ///
    /// Returns `true` on success or `false` if `new_len` is larger than [`Self::len()`].
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn truncate(&mut self, new_len: u16) -> bool {
        if new_len > self.len() {
            return false;
        }

        self.len = new_len;

        true
    }

    /// Copy from specified bytes.
    ///
    /// Returns `false` if capacity is not enough to copy contents of `src`
    #[inline(always)]
    #[must_use = "Operation may fail"]
    pub fn copy_from<T>(&mut self, src: &[u8]) -> bool {
        if src.len() > CAPACITY {
            return false;
        }

        self.bytes[..src.len()].copy_from_slice(src);
        self.len = src.len() as u16;

        true
    }
}
