use derive_more::Display;

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct UnknownContractErrorCode(u8);

impl UnknownContractErrorCode {
    /// Get the inner error code
    #[inline]
    pub const fn code(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct CustomContractErrorCode(u64);

impl CustomContractErrorCode {
    /// Get the inner error code
    #[inline]
    pub const fn code(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub enum ContractError {
    BadInput,
    BadOutput,
    Forbidden,
    NotFound,
    Conflict,
    InternalError,
    NotImplemented,
    Unknown(UnknownContractErrorCode),
    Custom(CustomContractErrorCode),
}

impl From<CustomContractErrorCode> for ContractError {
    #[inline]
    fn from(error: CustomContractErrorCode) -> Self {
        Self::Custom(error)
    }
}

impl ContractError {
    /// Create contract error with a custom error code.
    ///
    /// Code must be larger than `u8::MAX` or `None` will be returned.
    #[inline]
    pub const fn new_custom_code(code: u64) -> Option<Self> {
        if code > u8::MAX as u64 {
            Some(Self::Custom(CustomContractErrorCode(code)))
        } else {
            None
        }
    }

    /// Convert contact error into contract exit code.
    ///
    /// Mosty useful for low-level code.
    #[inline]
    pub const fn exit_code(self) -> ExitCode {
        ExitCode(match self {
            Self::BadInput => 1,
            Self::BadOutput => 2,
            Self::Forbidden => 3,
            Self::NotFound => 4,
            Self::Conflict => 5,
            Self::InternalError => 6,
            Self::NotImplemented => 7,
            Self::Unknown(unknown) => unknown.code() as u64,
            Self::Custom(custom) => custom.code(),
        })
    }
}

#[derive(Debug, Display, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
#[repr(C)]
#[must_use = "Code can be Ok or one of the errors, consider converting to Result<(), ContractCode>"]
pub struct ExitCode(u64);

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
            Ok(()) => Self(0),
            Err(error) => error.exit_code(),
        }
    }
}

impl From<ExitCode> for Result<(), ContractError> {
    #[inline]
    fn from(value: ExitCode) -> Self {
        Err(match value.0 {
            0 => {
                return Ok(());
            }
            1 => ContractError::BadInput,
            2 => ContractError::BadOutput,
            3 => ContractError::Forbidden,
            4 => ContractError::NotFound,
            5 => ContractError::Conflict,
            6 => ContractError::InternalError,
            7 => ContractError::NotImplemented,
            8..=255 => ContractError::Unknown(UnknownContractErrorCode(value.0 as u8)),
            code => ContractError::Custom(CustomContractErrorCode(code)),
        })
    }
}

impl ExitCode {
    /// Exit code indicating success
    #[inline]
    pub fn ok() -> Self {
        Self(0)
    }

    /// Convert into `u64`
    #[inline]
    pub const fn into_u64(self) -> u64 {
        self.0
    }
}
