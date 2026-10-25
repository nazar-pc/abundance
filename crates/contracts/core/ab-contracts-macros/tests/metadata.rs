//! Metadata that `#[contract]` generates must decode completely, both as it is and compacted.
//!
//! The tests of the decoder in `ab-contracts-common` use metadata written by hand. These tests
//! decode metadata that `#[contract]` generates for methods of every shape it supports, so a
//! decoder rule that rejects any of them fails here rather than in a contract.

use ab_contracts_common::env::Env;
use ab_contracts_common::metadata::ContractMetadataKind;
use ab_contracts_common::metadata::decode::{
    MetadataDecoder, MetadataItem, MethodMetadataDecoder, MethodsContainerKind,
};
use ab_contracts_common::{Contract, ContractError, ContractTrait, ContractTraitDefinition};
use ab_contracts_macros::contract;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::metadata::MAX_METADATA_CAPACITY;
use ab_io_type::trivial_type::TrivialType;
use shapes_trait::ShapesTrait;

/// Contract with methods of every shape that `#[contract]` generates
#[derive(Debug, Copy, Clone, TrivialType)]
#[repr(C)]
pub struct Shapes {
    pub value: u64,
}

#[contract]
impl Shapes {
    /// The state is the return value
    #[init]
    pub fn new(#[env] _env: &Env<'_>, #[input] &value: &u64) -> Self {
        Self { value }
    }

    /// The state is the return value in a `Result`
    #[init]
    pub fn new_fallible(#[input] &value: &u64) -> Result<Self, ContractError> {
        Ok(Self { value })
    }

    /// The state is the last `#[output]`
    #[init]
    pub fn new_with_outputs(
        #[env] _env: &mut Env<'_>,
        #[tmp] _tmp: &mut MaybeData<'_, u64>,
        #[slot] _slot: &mut MaybeData<'_, u64>,
        #[input] &value: &u64,
        #[output] other: &mut MaybeData<'_, u64>,
        #[output] state: &mut MaybeData<'_, Self>,
    ) {
        other.replace(value);
        state.replace(Self { value });
    }

    /// No arguments at all
    #[update]
    pub fn update_nothing() {}

    /// Only a return value
    #[update]
    pub fn update_only_return() -> u64 {
        0
    }

    /// Only the state, without a return value
    #[update]
    pub fn update_only_self(&mut self) {
        self.value = 0;
    }

    /// Read-only `#[env]`, `#[tmp]` and `#[slot]`, with a return value
    #[update]
    pub fn update_stateless(
        #[env] _env: &Env<'_>,
        #[tmp] _tmp: &MaybeData<'_, u64>,
        #[slot] _slot: &MaybeData<'_, u64>,
        #[input] &value: &u64,
    ) -> u64 {
        value
    }

    /// Several `#[input]`s and `#[output]`s, with a return value in a `Result`
    #[update]
    pub fn update_stateful_ro(
        &self,
        #[slot] _slot: &MaybeData<'_, u64>,
        #[input] &first: &u64,
        #[input] &second: &u64,
        #[output] first_output: &mut MaybeData<'_, u64>,
        #[output] second_output: &mut MaybeData<'_, u64>,
    ) -> Result<u64, ContractError> {
        first_output.replace(first);
        second_output.replace(second);
        Ok(self.value)
    }

    /// Read-write `#[env]`, `#[tmp]` and `#[slot]`, without a return value
    #[update]
    pub fn update_stateful_rw(
        &mut self,
        #[env] _env: &mut Env<'_>,
        #[tmp] _tmp: &mut MaybeData<'_, u64>,
        #[slot] _slot: &mut MaybeData<'_, u64>,
        #[input] &value: &u64,
    ) -> Result<(), ContractError> {
        self.value = value;
        Ok(())
    }

    /// The max number of arguments without `self`
    #[update]
    pub fn update_max_arguments_stateless(
        #[env] _env: &mut Env<'_>,
        #[tmp] _tmp: &mut MaybeData<'_, u64>,
        #[slot] _first_slot: &MaybeData<'_, u64>,
        #[slot] _second_slot: &mut MaybeData<'_, u64>,
        #[input] &first: &u64,
        #[input] &second: &u64,
        #[output] output: &mut MaybeData<'_, u64>,
    ) -> u64 {
        output.replace(first);
        second
    }

    /// The max number of arguments with `self`
    #[update]
    pub fn update_max_arguments_stateful(
        &mut self,
        #[env] _env: &mut Env<'_>,
        #[tmp] _tmp: &mut MaybeData<'_, u64>,
        #[slot] _first_slot: &MaybeData<'_, u64>,
        #[slot] _second_slot: &mut MaybeData<'_, u64>,
        #[input] &value: &u64,
        #[output] output: &mut MaybeData<'_, u64>,
    ) -> u64 {
        output.replace(self.value);
        self.value = value;
        value
    }

    /// No arguments at all, like the example in the docs of `#[contract]`
    #[view]
    pub fn view_nothing() {}

    /// Read-only `#[env]` and `#[slot]`, with an `#[output]` and a return value
    #[view]
    pub fn view_stateless(
        #[env] _env: &Env<'_>,
        #[slot] _slot: &MaybeData<'_, u64>,
        #[input] &value: &u64,
        #[output] output: &mut MaybeData<'_, u64>,
    ) -> u64 {
        output.replace(value);
        value
    }

    /// Only the state and a return value
    #[view]
    pub fn view_stateful(&self) -> u64 {
        self.value
    }
}

#[expect(
    clippy::inline_modules,
    reason = "`#[contract]` generates an `ffi` module for both a contract and a trait \
    definition, so the trait needs a module of its own"
)]
pub mod shapes_trait {
    use ab_contracts_common::ContractError;
    use ab_contracts_common::env::Env;
    use ab_contracts_macros::contract;
    use ab_io_type::maybe_data::MaybeData;

    /// Trait with methods of every shape that `#[contract]` generates in traits
    #[contract]
    pub trait ShapesTrait {
        /// Read-write `#[env]`, several `#[input]`s and `#[output]`s, with a return value in a
        /// `Result`
        #[update]
        fn trait_update(
            #[env] env: &mut Env<'_>,
            #[input] first: &u64,
            #[input] second: &u64,
            #[output] first_output: &mut MaybeData<'_, u64>,
            #[output] second_output: &mut MaybeData<'_, u64>,
        ) -> Result<u64, ContractError>;

        /// Read-only `#[env]`, without a return value
        #[update]
        fn trait_update_env_ro(#[env] env: &Env<'_>, #[output] output: &mut MaybeData<'_, u64>);

        /// The max number of arguments, methods of traits have no `self`
        #[update]
        fn trait_update_max_arguments(
            #[env] env: &mut Env<'_>,
            #[input] first: &u64,
            #[input] second: &u64,
            #[input] third: &u64,
            #[input] fourth: &u64,
            #[output] first_output: &mut MaybeData<'_, u64>,
            #[output] second_output: &mut MaybeData<'_, u64>,
        ) -> u64;

        /// No arguments at all, so nothing from the parent module is used
        #[update]
        fn trait_update_nothing();

        /// Read-only `#[env]`, with an `#[output]` and a return value
        #[view]
        fn trait_view(
            #[env] env: &Env<'_>,
            #[input] value: &u64,
            #[output] output: &mut MaybeData<'_, u64>,
        ) -> u64;
    }
}

#[contract]
impl ShapesTrait for Shapes {
    #[update]
    fn trait_update(
        #[env] _env: &mut Env<'_>,
        #[input] &first: &u64,
        #[input] &second: &u64,
        #[output] first_output: &mut MaybeData<'_, u64>,
        #[output] second_output: &mut MaybeData<'_, u64>,
    ) -> Result<u64, ContractError> {
        first_output.replace(first);
        second_output.replace(second);
        Ok(first)
    }

    #[update]
    fn trait_update_env_ro(#[env] _env: &Env<'_>, #[output] output: &mut MaybeData<'_, u64>) {
        output.replace(0);
    }

    #[update]
    fn trait_update_max_arguments(
        #[env] _env: &mut Env<'_>,
        #[input] &first: &u64,
        #[input] &second: &u64,
        #[input] &third: &u64,
        #[input] &fourth: &u64,
        #[output] first_output: &mut MaybeData<'_, u64>,
        #[output] second_output: &mut MaybeData<'_, u64>,
    ) -> u64 {
        first_output.replace(first);
        second_output.replace(second);
        third + fourth
    }

    #[update]
    fn trait_update_nothing() {}

    #[view]
    fn trait_view(
        #[env] _env: &Env<'_>,
        #[input] &value: &u64,
        #[output] output: &mut MaybeData<'_, u64>,
    ) -> u64 {
        output.replace(value);
        value
    }
}

/// Compact metadata with `compact_fn`, check that compacting it again changes nothing and return
/// it
fn compact<CompactFn>(metadata: &[u8], compact_fn: CompactFn) -> Vec<u8>
where
    CompactFn: Fn(&[u8]) -> Option<([u8; MAX_METADATA_CAPACITY], usize)>,
{
    let (compacted, size) = compact_fn(metadata).unwrap();
    let compacted = compacted[..size].to_vec();

    let (compacted_again, size_again) = compact_fn(&compacted).unwrap();
    assert_eq!(compacted_again[..size_again], compacted);

    compacted
}

/// Metadata as it is and both of its compact forms
fn all_forms(metadata: &[u8]) -> [Vec<u8>; 3] {
    [
        metadata.to_vec(),
        compact(metadata, ContractMetadataKind::compact),
        compact(metadata, ContractMetadataKind::compact_external_args),
    ]
}

/// Decode every method and argument of contract or trait metadata, then check that nothing is
/// left
fn decode_completely(metadata: &[u8], is_trait: bool) {
    let mut decoder = MetadataDecoder::new(metadata);
    let mut num_items = 0;

    while let Some(item) = decoder.decode_next() {
        let item = item.unwrap();
        #[expect(
            clippy::rest_pattern_accessible_field,
            reason = "Do not need other fields"
        )]
        let item_is_trait = matches!(item, MetadataItem::Trait { .. });
        assert_eq!(item_is_trait, is_trait);
        num_items += 1;

        let mut methods_decoder = item.into_decoder();
        while let Some(method_decoder) = methods_decoder.decode_next() {
            let (mut arguments_decoder, _method) = method_decoder.decode_next().unwrap();
            while let Some(argument) = arguments_decoder.decode_next() {
                argument.unwrap();
            }
        }
    }

    assert_eq!(num_items, 1);
    assert_eq!(decoder.remaining_metadata_bytes(), 0);
}

/// Decode every argument of method metadata the way executors do, without knowing whether the
/// method belongs to a contract or a trait, then check that nothing is left
fn decode_method_completely(mut metadata: &[u8]) {
    let (mut arguments_decoder, _method) =
        MethodMetadataDecoder::new(&mut metadata, MethodsContainerKind::Unknown)
            .decode_next()
            .unwrap();
    while let Some(argument) = arguments_decoder.decode_next() {
        argument.unwrap();
    }

    assert_eq!(arguments_decoder.remaining_metadata_bytes(), 0);
}

#[test]
fn contract_metadata_decodes() {
    for metadata in all_forms(Shapes::MAIN_CONTRACT_METADATA) {
        decode_completely(&metadata, false);
    }
}

#[test]
fn trait_metadata_decodes() {
    for metadata in all_forms(<dyn ShapesTrait as ContractTraitDefinition>::METADATA) {
        decode_completely(&metadata, true);
    }
}

#[test]
fn method_metadata_decodes() {
    let methods = <Shapes as Contract>::NATIVE_EXECUTOR_METHODS
        .iter()
        .chain(<Shapes as ContractTrait<dyn ShapesTrait>>::NATIVE_EXECUTOR_METHODS);
    for method in methods {
        for metadata in all_forms(method.method_metadata) {
            decode_method_completely(&metadata);
        }
    }
}
