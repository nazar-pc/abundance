#![no_std]

pub mod env;
mod error;
pub mod metadata;
pub mod method;

use crate::method::MethodFingerprint;
use ab_io_type::IoType;
use ab_io_type::variable_bytes::VariableBytes;
use core::ffi::c_void;
use core::ops::Deref;
use core::ptr::NonNull;
pub use error::{ContractError, CustomContractErrorCode, ExitCode};

/// Max allowed size of the contract code
pub const MAX_CODE_SIZE: u32 = 1024 * 1024;
/// Max number of arguments in a method.
///
/// NOTE: Both `self` and return type that is not `()` or `Result<(), ContractError>` count towards
/// the total number of method arguments.
pub const MAX_TOTAL_METHOD_ARGS: u8 = 8;

/// Method details used by native execution environment.
///
/// `ffi_fn`'s argument is actually `NonNull<InternalArgs>` of corresponding method and must have
/// corresponding ABI.
///
/// NOTE: It is unlikely to be necessary to interact with this directly.
#[derive(Debug, Copy, Clone)]
#[doc(hidden)]
pub struct NativeExecutorContactMethod {
    pub method_fingerprint: &'static MethodFingerprint,
    pub method_metadata: &'static [u8],
    pub ffi_fn: unsafe extern "C" fn(NonNull<NonNull<c_void>>) -> ExitCode,
}

/// A trait that indicates the struct is a contact.
///
/// **Do not implement this trait explicitly!** Implementation is automatically generated by the
/// macro which generates contract implementation. This trait is required, but not sufficient for
/// proper contract implementation, use `#[contract]` attribute macro instead.
pub trait Contract: IoType {
    /// Main contract metadata, see [`ContractMetadataKind`] for encoding details.
    ///
    /// More metadata can be contributed by trait implementations.
    ///
    /// [`ContractMetadataKind`]: crate::metadata::ContractMetadataKind
    const MAIN_CONTRACT_METADATA: &[u8];
    /// Something that can be used as "code" in native execution environment.
    ///
    /// NOTE: It is unlikely to be necessary to interact with this directly.
    #[doc(hidden)]
    const CODE: &str;
    /// Methods of a contract used in native execution environment.
    ///
    /// NOTE: It is unlikely to be necessary to interact with this directly.
    #[doc(hidden)]
    const NATIVE_EXECUTOR_METHODS: &[NativeExecutorContactMethod];
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
    /// Something that can be used as "code" in native execution environment and primarily used for
    /// testing.
    ///
    /// This is NOT the code compiled for guest architecture!
    // TODO: Make `const` when possible
    fn code() -> impl Deref<Target = VariableBytes<MAX_CODE_SIZE>>;
}

/// A trait that indicates the implementation of a contract trait by a contract.
///
/// `DynTrait` here is `dyn ContractTrait`, which is a bit of a hack that allows treating a trait as
/// a type for convenient API in native execution environment.
///
/// **Do not implement this trait explicitly!** Implementation is automatically generated by the
/// macro which generates contract trait implementation. This trait is required, but not sufficient
/// for proper trait implementation, use `#[contract]` attribute macro instead.
///
/// NOTE: It is unlikely to be necessary to interact with this directly.
pub trait ContractTrait<DynTrait>
where
    DynTrait: ?Sized,
{
    /// Methods of a trait used in native execution environment
    #[doc(hidden)]
    const NATIVE_EXECUTOR_METHODS: &[NativeExecutorContactMethod];
}

/// A trait that is implemented for `dyn ContractTrait` and includes constants related to trait
/// definition.
///
/// `dyn ContractTrait` here is a bit of a hack that allows treating a trait as a type. These
/// constants specifically can't be implemented on a trait itself because that'll make trait
/// not object safe, which is needed for [`ContractTrait`] that uses a similar hack with
/// `dyn ContractTrait`.
///
/// **Do not implement this trait explicitly!** Implementation is automatically generated by the
/// macro which generates trait definition. This trait is required, but not sufficient for
/// proper trait implementation, use `#[contract]` attribute macro instead.
///
/// NOTE: It is unlikely to be necessary to interact with this directly.
pub trait ContractTraitDefinition {
    // Default value is provided to only fail to compile when trait that uses
    // `ab-contracts-common` has feature specified, but `ab-contracts-common` does not, but not the
    // other way around (as will be the case with dependencies where `guest` feature must not be
    // enabled)
    #[cfg(feature = "guest")]
    #[doc(hidden)]
    const GUEST_FEATURE_ENABLED: () = ();
    /// Trait metadata, see [`ContractMetadataKind`] for encoding details"]
    /// Trait metadata, see [`ContractMetadataKind`] for encoding details"]
    ///
    /// [`ContractMetadataKind`]: crate::metadata::ContractMetadataKind
    const METADATA: &[::core::primitive::u8];
}
