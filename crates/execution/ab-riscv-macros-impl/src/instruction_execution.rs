use ab_riscv_macros_common::code_utils::pre_process_rust_code;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{Error, ItemImpl, Type, parse_str};

pub(super) fn instruction_execution(
    _attr: TokenStream,
    item: TokenStream,
) -> Result<TokenStream, Error> {
    let mut code = item.to_string();
    pre_process_rust_code(&mut code);

    let item_impl = parse_str::<ItemImpl>(&code).map_err(|error| {
        Error::new(
            error.span(),
            format!(
                "`#[instruction_execution]` must be applied to enum implementation: {error}: {}",
                item
            ),
        )
    })?;

    let Type::Path(path) = item_impl.self_ty.as_ref() else {
        return Err(Error::new(
            item_impl.span(),
            format!(
                "Expected `impl` for `{}`, `#[instruction_execution]` attribute must be added to a \
                simple instruction enum implementation",
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
    let enum_file_path = format!("/{}_execution_impl.rs", enum_name);

    // Replace enum implementation with a processed impl stored in a Rust file
    Ok(quote! {
        include!(concat!(env!("OUT_DIR"), #enum_file_path));
    })
}
