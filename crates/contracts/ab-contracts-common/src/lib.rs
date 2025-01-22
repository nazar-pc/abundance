#![feature(non_null_from_ref)]
#![no_std]

pub mod env;
pub mod metadata;
pub mod method;

use crate::method::MethodFingerprint;
use ab_contracts_io_type::IoType;
use ab_contracts_io_type::trivial_type::TrivialType;
use core::ffi::c_void;
use core::fmt;
use core::ptr::NonNull;
use derive_more::{
    Add, AddAssign, Display, Div, DivAssign, From, Into, Mul, MulAssign, Sub, SubAssign,
};

/// Pointers to methods of all contracts.
///
/// `fn_pointer`'s argument is actually `NonNull<InternalArgs>` of corresponding method and must
/// have corresponding ABI.
///
/// NOTE: It is unlikely to be necessary to interact with this directly.
#[derive(Debug, Copy, Clone)]
#[cfg(any(unix, windows))]
pub struct ContractsMethodsFnPointer {
    pub crate_name: &'static str,
    pub main_contract_metadata: &'static [u8],
    pub method_fingerprint: &'static MethodFingerprint,
    pub method_metadata: &'static [u8],
    pub ffi_fn: unsafe extern "C" fn(NonNull<NonNull<c_void>>) -> ExitCode,
}

#[cfg(any(unix, windows))]
inventory::collect!(ContractsMethodsFnPointer);

// TODO: Add `Slot` and `Tmp` associated types such that it is not necessary to repeat them in
//  arguments
/// A trait that indicates the struct is a contact definition.
///
/// **Do not implement this trait explicitly!** Implementation is automatically generated by the
/// macro which generates smart contract implementation. This trait is required, but not sufficient
/// for proper contract implementation, use `#[contract]` attribute macro instead.
pub trait Contract: IoType {
    /// Main contract metadata, see [`ContractMetadataKind`] for encoding details.
    ///
    /// More metadata can be contributed by trait implementations.
    ///
    /// [`ContractMetadataKind`]: crate::metadata::ContractMetadataKind
    const MAIN_CONTRACT_METADATA: &[u8];
    /// Name of the crate where contact is located.
    ///
    /// NOTE: It is unlikely to be necessary to interact with this directly.
    #[cfg(any(unix, windows))]
    const CRATE_NAME: &str;
    // Default value is provided to only fail to compile when contract that uses
    // `ab-contracts-common` has feature specified, but `ab-contracts-common` does not, but not the
    // other way around (as will be the case with dependencies where `guest` feature must not be
    // enabled)
    #[cfg(feature = "guest")]
    #[doc(hidden)]
    const GUEST_FEATURE_ENABLED: () = ();
    /// Slot type used by this contract
    type Slot: IoType;
    /// Tmp type used by this contract
    type Tmp: IoType;
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
pub enum ContractError {
    NotFound = 1,
    InvalidState,
    InvalidInput,
    InvalidOutput,
    AccessDenied,
}

impl ContractError {
    /// Convert contact error into contract exit code.
    ///
    /// Mosty useful for low-level code.
    #[inline]
    pub const fn exit_code(self) -> ExitCode {
        match self {
            Self::NotFound => ExitCode::NotFound,
            Self::InvalidState => ExitCode::InvalidState,
            Self::InvalidInput => ExitCode::InvalidInput,
            Self::InvalidOutput => ExitCode::InvalidOutput,
            Self::AccessDenied => ExitCode::AccessDenied,
        }
    }
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(u8)]
#[must_use = "Code can be Ok or one of the errors, consider converting to Result<(), ContractCode>"]
pub enum ExitCode {
    Ok = 0,
    NotFound = 1,
    InvalidState,
    InvalidInput,
    InvalidOutput,
    AccessDenied,
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
            ExitCode::NotFound => Err(ContractError::NotFound),
            ExitCode::InvalidState => Err(ContractError::InvalidState),
            ExitCode::InvalidInput => Err(ContractError::InvalidInput),
            ExitCode::InvalidOutput => Err(ContractError::InvalidOutput),
            ExitCode::AccessDenied => Err(ContractError::AccessDenied),
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

impl PartialEq<&Address> for Address {
    #[inline]
    fn eq(&self, other: &&Address) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<Address> for &Address {
    #[inline]
    fn eq(&self, other: &Address) -> bool {
        self.0 == other.0
    }
}

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

// TODO: Method for getting creation shard out of the address
// TODO: There should be a notion of global address
impl Address {
    // TODO: Various system contracts
    /// Sentinel contract address, inaccessible and not owned by anyone
    pub const NULL: Self = Self(0u64.to_le_bytes());
    /// System contract for managing code of other contracts
    pub const SYSTEM_CODE: Self = Self(1u64.to_le_bytes());
    /// System contract for managing state of other contracts
    pub const SYSTEM_STATE: Self = Self(2u64.to_le_bytes());

    /// System contract for address allocation on a particular shard index
    #[inline]
    pub const fn system_address_allocator(shard_index: ShardIndex) -> Address {
        // Shard `0` doesn't have its own allocator because there are no user-deployable contracts
        // there, so address `0` is `NULL`, the rest up to `ShardIndex::MAX_SHARD_INDEX` correspond
        // to address allocators of respective shards
        Address((shard_index.to_u32() as u64 * ShardIndex::MAX_SHARDS as u64).to_le_bytes())
    }
}
