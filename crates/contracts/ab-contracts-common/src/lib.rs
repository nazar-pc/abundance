#![no_std]

pub mod env;
pub mod method;

use ab_contracts_io_type::trivial_type::TrivialType;
use core::fmt;
use derive_more::{
    Add, AddAssign, Display, Div, DivAssign, From, Into, Mul, MulAssign, Sub, SubAssign,
};

/// Metadata for smart contact methods.
///
/// Metadata encoding consists of this enum variant treated as `u8` followed by optional metadata
/// encoding rules specific to metadata type variant (see variant's description).
///
/// This metadata is sufficient to fully reconstruct hierarchy of the type in order to generate
/// language bindings, auto-generate UI forms, etc.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ContractMetadataKind {
    /// `#[init]` method.
    ///
    /// Initializers are encoded af follows:
    /// * Length of method name in bytes (u8)
    /// * Method name as UTF-8 bytes
    /// * Number of arguments (u8)
    ///
    /// Each argument is encoded as follows:
    /// * Argument type as u8, one of:
    ///   * [`Self::EnvRo`]
    ///   * [`Self::EnvRw`]
    ///   * [`Self::TmpRo`]
    ///   * [`Self::TmpRw`]
    ///   * [`Self::SlotWithAddressRo`]
    ///   * [`Self::SlotWithAddressRw`]
    ///   * [`Self::SlotWithoutAddressRo`]
    ///   * [`Self::SlotWithoutAddressRw`]
    ///   * [`Self::Input`]
    ///   * [`Self::Output`]
    ///   * [`Self::Result`]
    /// * Length of the argument name in bytes (u8)
    /// * Argument name as UTF-8 bytes
    /// * Recursive metadata of argument's type as described in
    ///   [`IoTypeMetadataKind`](ab_contracts_io_type::IoTypeMetadataKind).
    ///
    /// NOTE: Result, regardless of whether it is a return type or explicit `#[result]` argument, is
    /// encoded as a separate argument and counts towards number of arguments. At the same time
    /// `self` doesn't count towards the number of arguments as it is implicitly defined by the
    /// variant of this struct.
    Init,

    /// Stateless `#[update]` method (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    UpdateStateless,

    /// Stateful read-only `#[update]` method (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    UpdateStatefulRo,

    /// Stateful read-write `#[update]` method (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    UpdateStatefulRw,

    /// Stateless `#[view]` method (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    ViewStateless,

    /// Stateful read-only `#[view]` method (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init`].
    ViewStatefulRo,

    /// Read-only `#[env]` argument.
    ///
    /// Example: `#[env] env: &Env,`
    EnvRo,
    /// Read-write `#[env]` argument.
    ///
    /// Example: `#[env] env: &mut Env,`
    EnvRw,
    /// Read-only `#[tmp]` argument.
    ///
    /// Example: `#[tmp] tmp: &MaybeData<Slot>,`
    TmpRo,
    /// Read-write `#[tmp]` argument.
    ///
    /// Example: `#[tmp] tmp: &mut MaybeData<Slot>,`
    TmpRw,
    /// Read-only `#[slot]` argument with an address.
    ///
    /// Example: `#[slot] (from_address, from): (&Address, &MaybeData<Slot>),`
    SlotWithAddressRo,
    /// Read-write `#[slot]` argument with an address.
    ///
    /// Example: `#[slot] (from_address, from): (&Address, &mut MaybeData<Slot>),`
    SlotWithAddressRw,
    /// Read-only `#[slot]` argument without an address.
    ///
    /// Example: `#[slot] from: &MaybeData<Slot>,`
    SlotWithoutAddressRo,
    /// Read-write `#[slot]` argument without an address.
    ///
    /// Example: `#[slot] from: &mut MaybeData<Slot>,`
    SlotWithoutAddressRw,
    /// `#[input]` argument.
    ///
    /// Example: `#[input] balance: &Balance,`
    Input,
    /// `#[output]` argument.
    ///
    /// Example: `#[output] out: &mut VariableBytes<1024>,`
    Output,
    /// Explicit `#[result`] argument or `T` of [`Result<T, ContractError>`] return type or simply
    /// return type if it is not fallible.
    ///
    /// Example: `#[result] result: &mut MaybeData<Balance>,`
    ///
    /// NOTE: There is always exactly one result in a method.
    Result,
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
    pub const NULL: Self = Self([0; 8]);
    /// System contract for managing code of other contracts
    pub const SYSTEM_CODE: Self = Self([1; 8]);

    /// System contract for address allocation on a particular shard index
    pub const fn system_address_allocator(shard_index: ShardIndex) -> Address {
        // Shard `0` doesn't have its own allocator because there are no user-deployable contracts
        // there, so address `0` is `NULL`, the rest up to `ShardIndex::MAX_SHARD_INDEX` correspond
        // to address allocators of respective shards
        Address((shard_index.to_u32() as u64).to_le_bytes())
    }
}
