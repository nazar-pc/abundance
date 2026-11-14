//! Arguments can have the same names as fields, locals and parameters of generated code. This
//! compiles only if generated code doesn't clash with them.

use ab_contracts_common::env::Env;
use ab_contracts_macros::contract;
use ab_core_primitives::address::Address;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::trivial_type::TrivialType;

#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct ArgumentNames {
    pub value: u8,
}

#[contract]
impl ArgumentNames {
    #[init]
    pub fn new(#[input] &value: &u8) -> Self {
        Self { value }
    }

    /// Names of parameters and locals of the extension method and of `ExternalArgs::new()`
    #[update]
    pub fn generated_names(
        #[env] env: &mut Env<'_>,
        #[slot] contract: &MaybeData<'_, u8>,
        #[input] &method_context: &u8,
        #[input] &ok_result: &u8,
        #[output] args: &mut MaybeData<'_, Address>,
    ) -> u8 {
        args.replace(env.own_address());
        contract
            .get()
            .copied()
            .unwrap_or(method_context)
            .max(ok_result)
    }

    /// Names of locals of `ExternalArgs::new()`
    #[update]
    pub fn field_names(
        &self,
        #[env] env: &Env<'_>,
        #[input] &output_ptr: &u8,
        #[output] output: &mut MaybeData<'_, Address>,
    ) -> u8 {
        output.replace(env.own_address());
        self.value.max(output_ptr)
    }

    /// Names of locals of the FFI function that are derived from argument names
    #[update]
    pub fn local_names(
        #[env] env: &Env<'_>,
        #[slot] slot: &mut MaybeData<'_, u8>,
        #[input] &shorten_env: &u8,
        #[output] result: &mut MaybeData<'_, Address>,
    ) {
        slot.replace(shorten_env);
        result.replace(env.own_address());
    }
}
