use crate::build::{InstructionItem, InstructionItemsCollector, may_contain_instruction_macros};
use quote::ToTokens;
use syn::visit::Visit;
use syn::{File, parse_quote};

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
