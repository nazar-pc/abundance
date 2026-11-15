//! `#[contract]` puts generated code into the module of the contract and into modules that import
//! everything from it, where items of the user shadow the prelude. This compiles only if generated
//! code doesn't depend on such items.

use ab_contracts_common::ContractError;
use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_io_type::trivial_type::TrivialType;
use shadowing_trait::ShadowingTrait;

#[expect(unused_macros, reason = "Only shadows `assert!` of the prelude")]
macro_rules! assert {
    ($($tokens:tt)*) => {
        ::core::compile_error!("Generated code uses `assert!` that shadows the prelude")
    };
}

#[expect(unused_macros, reason = "Only shadows `debug_assert!` of the prelude")]
macro_rules! debug_assert {
    ($($tokens:tt)*) => {
        ::core::compile_error!("Generated code uses `debug_assert!` that shadows the prelude")
    };
}

#[expect(dead_code, reason = "Only shadows `Ok` of the prelude")]
struct Ok;

#[expect(dead_code, reason = "Only shadows `Err` of the prelude")]
struct Err;

#[expect(dead_code, reason = "Only shadows `Some` of the prelude")]
struct Some;

#[expect(dead_code, reason = "Only shadows `None` of the prelude")]
struct None;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Shadowing {
    pub value: u8,
}

#[contract]
impl Shadowing {
    #[init]
    pub fn new(#[input] &value: &u8) -> Self {
        Self { value }
    }

    #[update]
    pub fn fallible(&self) -> Result<u8, ContractError> {
        Result::Ok(self.value)
    }

    #[update]
    pub fn fallible_unit(&mut self, #[input] &value: &u8) -> Result<(), ContractError> {
        self.value = value;
        Result::Ok(())
    }
}

#[expect(
    clippy::inline_modules,
    reason = "`#[contract]` generates an `ffi` module for both a contract and a trait \
    definition, so the trait needs a module of its own"
)]
pub mod shadowing_trait {
    use ab_contracts_common::env::Env;
    use ab_contracts_macros::contract;

    // TODO: Shadow `usize` at the top level too once `const_format::concatcp!()`, which generated
    //  code of a contract uses, refers to `usize` with a path
    #[expect(
        dead_code,
        non_camel_case_types,
        reason = "Only shadows the primitive type"
    )]
    type usize = u8;

    #[contract]
    pub trait ShadowingTrait {
        #[view]
        fn trait_view(#[env] env: &Env<'_>) -> u8;
    }
}

#[contract]
impl ShadowingTrait for Shadowing {
    #[view]
    fn trait_view(#[env] _env: &Env<'_>) -> u8 {
        0
    }
}
