#![no_std]

pub mod env;
pub mod metadata;
pub mod method;

use ab_contracts_io_type::trivial_type::TrivialType;
use core::fmt;
use derive_more::{
    Add, AddAssign, Display, Div, DivAssign, From, Into, Mul, MulAssign, Sub, SubAssign,
};

/// A trait that indicates the struct is a contact definition.
///
/// NOTE: This trait is required, but not sufficient for contract implementation, do not implement
/// this trait manually, use `#[contract]` attribute macro instead.
pub trait Contract {
    /// Main contract metadata, see [`ContractMetadataKind`] for encoding details.
    ///
    /// More metadata can be contributed by trait implementations.
    ///
    /// [`ContractMetadataKind`]: crate::metadata::ContractMetadataKind
    const MAIN_CONTRACT_METADATA: &[u8];
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum ContractError {
    InvalidState = 1,
    InvalidInput,
    AccessDenied,
}

impl ContractError {
    /// Convert contact error into contract exit code.
    ///
    /// Mosty useful for low-level code.
    #[inline]
    pub const fn exit_code(self) -> ExitCode {
        match self {
            Self::InvalidState => ExitCode::InvalidState,
            Self::InvalidInput => ExitCode::InvalidInput,
            Self::AccessDenied => ExitCode::BadOrigin,
        }
    }
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
#[must_use = "Code can be Ok or one of the errors, consider converting to Result<(), ContractCode>"]
pub enum ExitCode {
    Ok = 0,
    InvalidState = 1,
    InvalidInput,
    BadOrigin,
}

impl From<ContractError> for ExitCode {
    #[inline]
    fn from(error: ContractError) -> Self {
        error.exit_code()
    }
}

impl From<Result<(), ContractError>> for ExitCode {
    #[inline]
    fn from(error: Result<(), ContractError>) -> Self {
        match error {
            Ok(()) => Self::Ok,
            Err(error) => error.exit_code(),
        }
    }
}

impl From<ExitCode> for Result<(), ContractError> {
    #[inline]
    fn from(value: ExitCode) -> Self {
        match value {
            ExitCode::Ok => Ok(()),
            ExitCode::InvalidState => Err(ContractError::InvalidState),
            ExitCode::InvalidInput => Err(ContractError::InvalidInput),
            ExitCode::BadOrigin => Err(ContractError::AccessDenied),
        }
    }
}

#[derive(
    Debug,
    Display,
    Default,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Mul,
    MulAssign,
    Div,
    DivAssign,
    From,
    Into,
    TrivialType,
)]
#[repr(transparent)]
pub struct Balance(u128);

impl Balance {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(u128::MAX);
}

/// Shard index
#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, TrivialType)]
#[repr(transparent)]
pub struct ShardIndex(
    // Essentially 32-bit number, but using an array reduces alignment requirement to 1 byte
    [u8; 4],
);

impl fmt::Display for ShardIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        u32::from_le_bytes(self.0).fmt(f)
    }
}

impl ShardIndex {
    /// Max possible shard index
    pub const MAX_SHARD_INDEX: u32 = Self::MAX_SHARDS - 1;
    /// Max possible number of shards
    pub const MAX_SHARDS: u32 = 2u32.pow(20);

    // TODO: Remove once traits work in const environment and `From` could be used
    /// Convert shard index to `u32`.
    ///
    /// This is typically only necessary for low-level code.
    pub const fn to_u32(self) -> u32 {
        u32::from_le_bytes(self.0)
    }

    // TODO: Remove once traits work in const environment and `From` could be used
    /// Create shard index from `u32`.
    ///
    /// Returns `None` if `shard_index > ShardIndex::MAX_SHARD_INDEX`
    ///
    /// This is typically only necessary for low-level code.
    pub const fn from_u32(shard_index: u32) -> Option<Self> {
        if shard_index > Self::MAX_SHARD_INDEX {
            return None;
        }

        Some(Self(shard_index.to_le_bytes()))
    }
}

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, TrivialType)]
#[repr(transparent)]
pub struct Address(
    // Essentially 64-bit number, but using an array reduces alignment requirement to 1 byte
    [u8; 8],
);

impl From<u64> for Address {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value.to_le_bytes())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Would it be better to represent address as something non-decimal that is shorter?
        u64::from_le_bytes(self.0).fmt(f)
    }
}

impl Address {
    // TODO: Various system contracts
    /// Sentinel contract address, inaccessible and not owned by anyone
    pub const NULL: Self = Self(0u64.to_le_bytes());
    /// System contract for managing code of other contracts
    pub const SYSTEM_CODE: Self = Self(1u64.to_le_bytes());
    /// System contract for managing state of other contracts
    pub const SYSTEM_STATE: Self = Self(2u64.to_le_bytes());

    /// System contract for address allocation on a particular shard index
    pub const fn system_address_allocator(shard_index: ShardIndex) -> Address {
        // Shard `0` doesn't have its own allocator because there are no user-deployable contracts
        // there, so address `0` is `NULL`, the rest up to `ShardIndex::MAX_SHARD_INDEX` correspond
        // to address allocators of respective shards
        Address((shard_index.to_u32() as u64).to_le_bytes())
    }
}
