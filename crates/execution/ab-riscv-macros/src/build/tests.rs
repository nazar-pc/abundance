use crate::build::enum_definition::process_enum_definition;
use crate::build::enum_impl::process_enum_impl;
use crate::build::execution_impl::process_execution_impl;
use crate::build::shared::attribute_arguments;
use crate::build::state::State;
use crate::build::{InstructionItem, InstructionItemsCollector, may_contain_instruction_macros};
use quote::ToTokens;
use std::fs;
use std::path::Path;
use syn::visit::Visit;
use syn::{Attribute, File, ItemEnum, ItemImpl, parse_quote};

#[test]
fn may_contain_instruction_macros_matches() {
    for file_contents in [
        "#[instruction]\nenum A {}",
        "#[instruction(inherit = [B])]\nenum A {}",
        "#[instruction_execution]\nimpl C for A {}",
        "mod b {\n    #[instruction]\n    enum A {}\n}",
        "#[ab_riscv_macros::instruction]\nenum A {}",
        "#[::ab_riscv_macros::instruction_execution]\nimpl C for A {}",
        "#[instruction[inherit = [B]]]\nenum A {}",
        "#[instruction{inherit = [B]}]\nenum A {}",
        "#[instruction (inherit = [B])]\nenum A {}",
        "#[ab_riscv_macros :: instruction]\nenum A {}",
        "#[instruction = \"B\"]\nenum A {}",
    ] {
        assert!(
            may_contain_instruction_macros(file_contents),
            "{file_contents}"
        );
    }
}

#[test]
fn may_contain_instruction_macros_skips() {
    for file_contents in [
        "enum A {}",
        "#[instructions]\nenum A {}",
        "#[ab_riscv_macros::instruction_set]\nenum A {}",
        "#[instruction_set[A]]\nenum A {}",
        "/// #[instruction]\nenum A {}",
        "#![instruction]",
        "use ab_riscv_macros::instruction;",
    ] {
        assert!(
            !may_contain_instruction_macros(file_contents),
            "{file_contents}"
        );
    }
}

#[test]
fn instruction_items_collector() {
    let file: File = parse_quote! {
        #[instruction]
        enum A {}

        enum NotAnnotated {}

        #[doc(hidden)]
        #[ab_riscv_macros::instruction_execution]
        impl C for A {}

        impl C for NotAnnotated {}

        mod b {
            mod c {
                #[ab_riscv_macros::instruction]
                enum D {}
            }

            #[instruction]
            impl C for D {}
        }

        fn e() {
            #[instruction]
            enum F {}
        }
    };

    let mut collector = InstructionItemsCollector::default();
    collector.visit_file(&file);

    let found = collector
        .items
        .iter()
        .map(|item| match item {
            InstructionItem::Enum(item_enum) => format!("enum {}", item_enum.ident),
            InstructionItem::Impl(item_impl) => {
                format!("impl {}", item_impl.self_ty.to_token_stream())
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(found, ["enum A", "impl A", "enum D", "impl D", "enum F"]);
}

#[test]
fn attribute_arguments_forms() {
    let attributes: [(Attribute, Option<&str>); 6] = [
        (parse_quote! { #[instruction] }, None),
        (parse_quote! { #[instruction()] }, None),
        (
            parse_quote! { #[ab_riscv_macros::instruction_execution] },
            None,
        ),
        (
            parse_quote! { #[instruction(ignore = [A])] },
            Some("ignore = [A]"),
        ),
        (
            parse_quote! { #[ab_riscv_macros::instruction_execution(ignore = [A])] },
            Some("ignore = [A]"),
        ),
        (parse_quote! { #[instruction = A] }, Some("A")),
    ];

    for (attribute, expected) in attributes {
        assert_eq!(
            attribute_arguments(&attribute).as_deref(),
            expected,
            "{}",
            attribute.to_token_stream()
        );
    }
}

/// Error of processing an annotated implementation of a supported trait.
///
/// Implementations below are otherwise well-formed for the checks done before dispatching on the
/// trait, so arguments are the only reason for the error to be about them.
fn impl_error<ProcessImpl>(item_impl: ItemImpl, process_impl: ProcessImpl) -> String
where
    ProcessImpl: FnOnce(ItemImpl, &Path, &mut State) -> Option<anyhow::Result<()>>,
{
    process_impl(item_impl, Path::new(""), &mut State::new())
        .expect("Annotated implementation must be processed")
        .expect_err("Implementation with arguments must be rejected")
        .to_string()
}

#[test]
fn enum_impl_arguments() {
    for item_impl in [
        parse_quote! { #[instruction(ignore = [A])] impl Instruction for A {} },
        parse_quote! { #[ab_riscv_macros::instruction(ignore = [A])] impl Display for A {} },
        parse_quote! { #[instruction = A] impl Instruction for A {} },
    ] {
        let error = impl_error(item_impl, process_enum_impl);
        assert!(error.contains("doesn't take arguments"), "{error}");
    }
}

#[test]
fn execution_impl_arguments() {
    for item_impl in [
        parse_quote! { #[instruction_execution(ignore = [A])] impl ExecutableInstruction for A {} },
        parse_quote! {
            #[ab_riscv_macros::instruction_execution(ignore = [A])]
            impl ExecutableInstructionOperands for A {}
        },
        parse_quote! { #[instruction_execution = A] impl ExecutableInstructionCsr for A {} },
    ] {
        let error = impl_error(item_impl, process_execution_impl);
        assert!(error.contains("doesn't take arguments"), "{error}");
    }
}

// TODO: Not supported on Miri on Windows yet: https://github.com/rust-lang/miri/issues/3482
#[cfg(not(all(miri, target_os = "windows")))]
#[test]
fn decoding_impl_dead_code_allow() {
    let out_dir = tempfile::tempdir().unwrap();
    let out_dir = out_dir.path();
    let mut state = State::new();

    let enums: [ItemEnum; 3] = [
        parse_quote! {
            #[instruction]
            enum A { X, Y }
        },
        parse_quote! {
            #[instruction(inherit = [A])]
            enum B {}
        },
        parse_quote! {
            #[instruction(inherit = [A], ignore = [Y])]
            enum C {}
        },
    ];
    for item_enum in enums {
        process_enum_definition(item_enum, out_dir, &mut state).unwrap();
    }

    let item_impls: [ItemImpl; 3] = [
        parse_quote! {
            #[instruction]
            impl Instruction for A {
                const ALIGNMENT: u8 = 4;

                fn try_decode(instruction: u32) -> Option<Self> {
                    fn is_x(instruction: u32) -> bool {
                        instruction == 0
                    }

                    if is_x(instruction) { Some(Self::X) } else { Some(Self::Y) }
                }

                fn size(&self) -> u8 {
                    4
                }
            }
        },
        parse_quote! {
            #[instruction]
            impl Instruction for B {
                const ALIGNMENT: u8 = 4;

                fn try_decode(instruction: u32) -> Option<Self> {
                    None
                }

                fn size(&self) -> u8 {
                    4
                }
            }
        },
        parse_quote! {
            #[instruction]
            impl Instruction for C {
                const ALIGNMENT: u8 = 4;

                fn try_decode(instruction: u32) -> Option<Self> {
                    None
                }

                fn size(&self) -> u8 {
                    4
                }
            }
        },
    ];
    for item_impl in item_impls {
        process_enum_impl(item_impl, out_dir, &mut state)
            .unwrap()
            .unwrap();
    }

    // Only `C` removes an instruction from decoding logic it is composed of
    for (enum_name, expected) in [("A", false), ("B", false), ("C", true)] {
        let code =
            fs::read_to_string(out_dir.join(format!("{enum_name}_decoding_impl.rs"))).unwrap();
        assert_eq!(code.contains("dead_code"), expected, "{code}");
    }
}
