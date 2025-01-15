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
pub enum ContractMethodMetadata {
    /// `#[init]` method with `1` argument.
    ///
    /// Initializers are encoded af follows:
    /// * Length of method name in bytes (u8)
    /// * Method name as UTF-8 bytes
    ///
    /// Each argument is encoded as follows:
    /// * Argument type as u8, one of:
    ///   * [`Self::EnvRo`]
    ///   * [`Self::EnvRw`]
    ///   * [`Self::SlotWithAddressRo`]
    ///   * [`Self::SlotWithAddressRw`]
    ///   * [`Self::SlotWithoutAddressRo`]
    ///   * [`Self::SlotWithoutAddressRw`]
    ///   * [`Self::InputRo`]
    ///   * [`Self::InputRw`]
    ///   * [`Self::Output`]
    ///   * [`Self::Result`]
    /// * Length of the argument name in bytes (u8)
    /// * Argument name as UTF-8 bytes
    /// * Recursive metadata of argument's type as described in
    ///   [`IoTypeMetadata`](ab_contracts_io_type::IoTypeMetadata).
    ///
    /// NOTE: Result, regardless of whether it is a return type or explicit `#[result]` argument is
    /// encoded as a separate argument and counts towards number of arguments. At the same time
    /// `self` doesn't count towards the number of arguments as it is implicitly defined by the
    /// variant of this struct.
    Init1,
    /// `#[init]` method with `2` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init2,
    /// `#[init]` method with `3` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init3,
    /// `#[init]` method with `4` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init4,
    /// `#[init]` method with `5` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init5,
    /// `#[init]` method with `6` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init6,
    /// `#[init]` method with `7` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init7,
    /// `#[init]` method with `8` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init8,
    /// `#[init]` method with `9` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init9,
    /// `#[init]` method with `10` arguments.
    ///
    /// Encoding is the same as [`Self::Init1`]
    Init10,

    /// Stateless `#[update]` method with `1` argument (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless1,
    /// Stateless `#[update]` method with `2` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless2,
    /// Stateless `#[update]` method with `3` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless3,
    /// Stateless `#[update]` method with `4` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless4,
    /// Stateless `#[update]` method with `5` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless5,
    /// Stateless `#[update]` method with `6` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless6,
    /// Stateless `#[update]` method with `7` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless7,
    /// Stateless `#[update]` method with `8` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless8,
    /// Stateless `#[update]` method with `9` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless9,
    /// Stateless `#[update]` method with `10` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStateless10,

    /// Stateful read-only `#[update]` method with `1` argument (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo1,
    /// Stateful read-only `#[update]` method with `2` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo2,
    /// Stateful read-only `#[update]` method with `3` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo3,
    /// Stateful read-only `#[update]` method with `4` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo4,
    /// Stateful read-only `#[update]` method with `5` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo5,
    /// Stateful read-only `#[update]` method with `6` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo6,
    /// Stateful read-only `#[update]` method with `7` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo7,
    /// Stateful read-only `#[update]` method with `8` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo8,
    /// Stateful read-only `#[update]` method with `9` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo9,
    /// Stateful read-only `#[update]` method with `10` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRo10,

    /// Stateful read-write `#[update]` method with `1` argument (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw1,
    /// Stateful read-write `#[update]` method with `2` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw2,
    /// Stateful read-write `#[update]` method with `3` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw3,
    /// Stateful read-write `#[update]` method with `4` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw4,
    /// Stateful read-write `#[update]` method with `5` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw5,
    /// Stateful read-write `#[update]` method with `6` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw6,
    /// Stateful read-write `#[update]` method with `7` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw7,
    /// Stateful read-write `#[update]` method with `8` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw8,
    /// Stateful read-write `#[update]` method with `9` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw9,
    /// Stateful read-write `#[update]` method with `10` arguments (has `&mut self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    CallStatefulRw10,

    /// Stateless `#[view]` method with `1` argument (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless1,
    /// Stateless `#[view]` method with `2` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless2,
    /// Stateless `#[view]` method with `3` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless3,
    /// Stateless `#[view]` method with `4` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless4,
    /// Stateless `#[view]` method with `5` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless5,
    /// Stateless `#[view]` method with `6` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless6,
    /// Stateless `#[view]` method with `7` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless7,
    /// Stateless `#[view]` method with `8` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless8,
    /// Stateless `#[view]` method with `9` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless9,
    /// Stateless `#[view]` method with `10` arguments (doesn't have `self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStateless10,

    /// Stateful read-only `#[view]` method with `1` argument (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo1,
    /// Stateful read-only `#[view]` method with `2` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo2,
    /// Stateful read-only `#[view]` method with `3` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo3,
    /// Stateful read-only `#[view]` method with `4` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo4,
    /// Stateful read-only `#[view]` method with `5` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo5,
    /// Stateful read-only `#[view]` method with `6` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo6,
    /// Stateful read-only `#[view]` method with `7` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo7,
    /// Stateful read-only `#[view]` method with `8` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo8,
    /// Stateful read-only `#[view]` method with `9` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo9,
    /// Stateful read-only `#[view]` method with `10` arguments (has `&self` in its arguments).
    ///
    /// Encoding is the same as [`Self::Init1`]
    ViewStatefulRo10,

    /// Read-only `#[env]` argument.
    ///
    /// Example: `#[env] env: &Env,`
    EnvRo,
    /// Read-write `#[env]` argument.
    ///
    /// Example: `#[env] env: &mut Env,`
    EnvRw,
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
    /// Read-only `#[input]` argument.
    ///
    /// Example: `#[input] balance: &Balance,`
    InputRo,
    /// Read-write `#[input]` argument.
    ///
    /// Example: `#[input] balance: &mut Balance,`
    InputRw,
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
    BadOrigin,
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
            Self::BadOrigin => ExitCode::BadOrigin,
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
            ExitCode::BadOrigin => Err(ContractError::BadOrigin),
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

#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, From, Into, TrivialType)]
#[repr(transparent)]
pub struct Address(
    // Essentially 64-bit number, but using an array reduces alignment requirement to 1 byte
    [u8; 8],
);

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Would it be better to represent address as something non-decimal that is shorter?
        u64::from_le_bytes(self.0).fmt(f)
    }
}

impl Address {
    // TODO: Various system contracts
    /// System address
    pub const SYSTEM: Self = Self([1; 8]);
    pub const NOBODY: Self = Self([0; 8]);
}
