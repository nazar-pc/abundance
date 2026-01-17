use ab_riscv_macros_common::code_utils::pre_process_rust_code;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{Error, ItemEnum, ItemImpl, Type, parse_str, parse2};

pub(super) fn instruction(attr: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    if let Ok(item_enum) = parse2::<ItemEnum>(item.clone()) {
        // Enum definition
        return process_enum_definition(attr, item_enum);
    }

    let mut code = item.to_string();
    pre_process_rust_code(&mut code);

    let item_impl =
        parse_str::<ItemImpl>(&code).map_err(|error| Error::new(error.span(), format!(
            "`#[instruction]` must be applied to enum definition or implementation: {error}: {}",
            item
        )))?;

    // Implementation of an enum
    process_enum_impl(item_impl)
}

fn process_enum_definition(_attr: TokenStream, item_enum: ItemEnum) -> Result<TokenStream, Error> {
    let enum_file_path = format!("/{}_definition.rs", item_enum.ident);

    // Replace enum definition with a processed enum stored in a Rust file
    Ok(quote! {
        include!(concat!(env!("OUT_DIR"), #enum_file_path));
    })
}

fn process_enum_impl(item_impl: ItemImpl) -> Result<TokenStream, Error> {
    let Type::Path(path) = item_impl.self_ty.as_ref() else {
        return Err(Error::new(
            item_impl.span(),
            format!(
                "Expected `impl` for `{}`, `#[instruction]` attribute must be added to a simple \
                instruction enum implementation",
                item_impl.self_ty.to_token_stream(),
            ),
        ));
    };
    let enum_name = path
        .path
        .segments
        .last()
        .expect("Path is never empty; qed")
        .ident
        .clone();
    let enum_file_path = format!("/{}_impl.rs", enum_name);

    // Replace enum implementation with a processed impl stored in a Rust file
    Ok(quote! {
        include!(concat!(env!("OUT_DIR"), #enum_file_path));
    })
}
