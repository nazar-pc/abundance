//! Metadata that `#[contract]` generates for each form of a method it accepts.
//!
//! This is the counterpart of `tests/ui`, which shows the forms it rejects. Each test compares
//! decoded methods to a table of what the source turns into. `ExternalArgs::FINGERPRINT` is
//! derived from method metadata alone, so it is not checked separately.

use crate::contracts::{arguments, counter, init, kinds};
use ab_contracts_common::metadata::MAX_METADATA_DEPTH;
use ab_contracts_common::metadata::decode::{
    ArgumentKind, MetadataDecoder, MetadataItem, MethodKind,
};
use ab_contracts_common::{Contract, ContractTraitDefinition};
use ab_io_type::IoType;
use ab_io_type::alignment::Alignment;
use ab_io_type::maybe_data::MaybeData;
use ab_io_type::metadata::{IoTypeDetails, IoTypeMetadataKind};

/// Kind, name and whether there is a type in metadata
type Argument<'metadata> = (ArgumentKind, &'metadata str, bool);

/// Decode kind, name and arguments of each method of contract or trait metadata
fn decode_methods(metadata: &[u8]) -> Vec<(MethodKind, &str, Vec<Argument<'_>>)> {
    let mut decoder = MetadataDecoder::new(metadata);
    let mut methods_decoder = decoder.decode_next().unwrap().unwrap().into_decoder();

    let mut methods = Vec::new();
    while let Some(method_decoder) = methods_decoder.decode_next() {
        let (mut arguments_decoder, method) = method_decoder.decode_next().unwrap();
        let mut arguments = Vec::new();
        while let Some(argument) = arguments_decoder.decode_next() {
            let argument = argument.unwrap();
            arguments.push((
                argument.argument_kind,
                str::from_utf8(argument.argument_name).unwrap(),
                argument.type_details.is_some(),
            ));
        }
        methods.push((
            method.method_kind,
            str::from_utf8(method.method_name).unwrap(),
            arguments,
        ));
    }

    methods
}

/// Recommended capacity and alignment, which can be compared, unlike `IoTypeDetails`
fn details(type_details: IoTypeDetails) -> (u32, Alignment) {
    (type_details.recommended_capacity, type_details.alignment)
}

/// Details of a type, decoded from its metadata
fn details_of<T>() -> (u32, Alignment)
where
    T: IoType,
{
    let (type_details, _remaining) =
        IoTypeMetadataKind::type_details::<MAX_METADATA_DEPTH>(T::METADATA).unwrap();
    details(type_details)
}

#[expect(
    clippy::inline_modules,
    reason = "`#[contract]` generates an `ffi` module for each contract and trait definition, so \
    each needs a module of its own"
)]
pub mod contracts {
    pub mod kinds {
        use crate::contracts::counter::Counter;
        use ab_contracts_macros::contract;
        use ab_io_type::trivial_type::TrivialType;

        #[derive(Debug, Copy, Clone, TrivialType)]
        #[repr(C)]
        pub struct MethodKinds {
            pub value: u8,
        }

        #[contract]
        impl MethodKinds {
            #[init]
            pub fn create() -> Self {
                Self { value: 0 }
            }

            #[update]
            pub fn update_stateless() {}

            #[update]
            pub fn update_read_only(&self) {}

            #[update]
            pub fn update_read_write(&mut self) {
                self.value = self.next_value();
            }

            #[view]
            pub fn view_stateless() {}

            #[view]
            pub fn view_stateful(&self) {}

            /// Not a method of the contract, since it has none of the attributes
            fn next_value(&self) -> u8 {
                self.value.wrapping_add(1)
            }
        }

        #[contract]
        impl Counter for MethodKinds {
            // The name of an argument can differ from the definition, only the definition is used
            // by callers
            #[update]
            fn increment(#[input] _amount: &u8) {}

            #[view]
            fn get(#[output] count: &mut u8) {
                *count = 0;
            }
        }
    }

    pub mod counter {
        use ab_contracts_macros::contract;

        #[contract]
        pub trait Counter {
            #[update]
            fn increment(#[input] by: &u8);

            #[view]
            fn get(#[output] count: &mut u8);
        }
    }

    pub mod arguments {
        use ab_contracts_common::env::Env;
        use ab_contracts_macros::contract;
        use ab_io_type::maybe_data::MaybeData;
        use ab_io_type::trivial_type::TrivialType;

        #[derive(Debug, Copy, Clone, TrivialType)]
        #[repr(C)]
        pub struct Arguments {
            pub value: u8,
        }

        #[contract]
        impl Arguments {
            #[update]
            pub fn all_kinds(
                #[env] env: &mut Env<'_>,
                #[tmp] tmp: &mut MaybeData<'_, u32>,
                #[slot] read_only: &MaybeData<'_, u64>,
                #[slot] read_write: &mut MaybeData<'_, u64>,
                #[input] &value: &u64,
                #[input] other: &u64,
                #[output] output: &mut MaybeData<'_, u64>,
            ) -> u64 {
                let _context = env.context();
                tmp.remove();
                read_write.replace(read_only.get().copied().unwrap_or_default());
                output.replace(*other);
                value
            }

            #[update]
            pub fn without_return(#[output] output: &mut MaybeData<'_, u64>) {
                output.replace(0);
            }
        }
    }

    pub mod init {
        use ab_contracts_macros::contract;
        use ab_io_type::maybe_data::MaybeData;
        use ab_io_type::trivial_type::TrivialType;

        #[derive(Debug, Copy, Clone, TrivialType)]
        #[repr(C)]
        pub struct Init {
            pub value: u64,
        }

        #[contract]
        impl Init {
            #[init]
            pub fn returned() -> Self {
                Self { value: 0 }
            }

            #[init]
            pub fn last_output(
                #[output] other: &mut MaybeData<'_, u64>,
                #[output] state: &mut MaybeData<'_, Self>,
            ) {
                other.replace(0);
                state.replace(Self { value: 0 });
            }
        }
    }
}

/// The attribute and the receiver of a method determine its kind, and its name is prefixed with
/// the name of the struct or trait in snake case. Methods without an attribute are not part of the
/// contract. The implementation of the trait renames an argument, which compiles, since callers
/// only use the definition.
#[test]
fn method_kind_follows_attribute_and_receiver() {
    let methods = decode_methods(kinds::MethodKinds::MAIN_CONTRACT_METADATA)
        .into_iter()
        .map(|(method_kind, method_name, _arguments)| (method_kind, method_name))
        .collect::<Vec<_>>();
    assert_eq!(
        methods,
        [
            (MethodKind::Init, "method_kinds_create"),
            (MethodKind::UpdateStateless, "method_kinds_update_stateless"),
            (
                MethodKind::UpdateStatefulRo,
                "method_kinds_update_read_only"
            ),
            (
                MethodKind::UpdateStatefulRw,
                "method_kinds_update_read_write"
            ),
            (MethodKind::ViewStateless, "method_kinds_view_stateless"),
            (MethodKind::ViewStateful, "method_kinds_view_stateful"),
        ]
    );

    assert_eq!(
        decode_methods(<dyn counter::Counter as ContractTraitDefinition>::METADATA),
        [
            (
                MethodKind::UpdateStateless,
                "counter_increment",
                vec![(ArgumentKind::Input, "by", true)],
            ),
            (
                MethodKind::ViewStateless,
                "counter_get",
                vec![(ArgumentKind::Output, "count", true)],
            ),
        ]
    );
}

/// Arguments are recorded in the order of the signature. `#[env]` has a fixed name, which is not
/// stored but decoded as `env`. `#[tmp]` and `#[slot]` have no type, since it is the same for the
/// whole contract. The return value has no name and is only recorded when there is one. The name
/// of an `#[input]` bound with a pattern like `&value` is the name of the variable.
#[test]
fn arguments_are_recorded_in_order() {
    assert_eq!(
        decode_methods(arguments::Arguments::MAIN_CONTRACT_METADATA),
        [
            (
                MethodKind::UpdateStateless,
                "arguments_all_kinds",
                vec![
                    (ArgumentKind::EnvRw, "env", false),
                    (ArgumentKind::TmpRw, "tmp", false),
                    (ArgumentKind::SlotRo, "read_only", false),
                    (ArgumentKind::SlotRw, "read_write", false),
                    (ArgumentKind::Input, "value", true),
                    (ArgumentKind::Input, "other", true),
                    (ArgumentKind::Output, "output", true),
                    (ArgumentKind::Return, "", true),
                ],
            ),
            (
                MethodKind::UpdateStateless,
                "arguments_without_return",
                vec![(ArgumentKind::Output, "output", true)],
            ),
        ]
    );
}

/// The type of the state is known to callers of `#[init]`, which never pass it, so whether it is
/// the return value or the last `#[output]`, it has no type in metadata
#[test]
fn init_state_has_no_type() {
    assert_eq!(
        decode_methods(init::Init::MAIN_CONTRACT_METADATA),
        [
            (
                MethodKind::Init,
                "init_returned",
                vec![(ArgumentKind::Return, "", false)],
            ),
            (
                MethodKind::Init,
                "init_last_output",
                vec![
                    (ArgumentKind::Output, "other", true),
                    (ArgumentKind::Output, "state", false),
                ],
            ),
        ]
    );
}

/// Contract metadata records the state type and the `#[slot]` and `#[tmp]` types its methods use,
/// which are `()` when no method uses them
#[test]
fn contract_records_state_slot_and_tmp_types() {
    let mut decoder = MetadataDecoder::new(arguments::Arguments::MAIN_CONTRACT_METADATA);
    #[expect(
        clippy::rest_pattern_accessible_field,
        reason = "Do not need other fields"
    )]
    let MetadataItem::Contract {
        state_type_name,
        state_type_details,
        slot_type_details,
        tmp_type_details,
        ..
    } = decoder.decode_next().unwrap().unwrap()
    else {
        panic!("Expected contract metadata");
    };
    assert_eq!(state_type_name, b"Arguments");
    assert_eq!(
        details(state_type_details),
        details_of::<arguments::Arguments>()
    );
    assert_eq!(
        details(slot_type_details),
        details_of::<MaybeData<'static, u64>>()
    );
    assert_eq!(
        details(tmp_type_details),
        details_of::<MaybeData<'static, u32>>()
    );

    let mut decoder = MetadataDecoder::new(kinds::MethodKinds::MAIN_CONTRACT_METADATA);
    #[expect(
        clippy::rest_pattern_accessible_field,
        reason = "Do not need other fields"
    )]
    let MetadataItem::Contract {
        slot_type_details,
        tmp_type_details,
        ..
    } = decoder.decode_next().unwrap().unwrap()
    else {
        panic!("Expected contract metadata");
    };
    assert_eq!(details(slot_type_details), details_of::<()>());
    assert_eq!(details(tmp_type_details), details_of::<()>());
}
