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
                "`#[instruction_execution]` must be applied to enum trait implementation: \
                {error}: {item}"
            ),
        )
    })?;

    let Type::Path(path) = item_impl.self_ty.as_ref() else {
        return Err(Error::new(
            item_impl.span(),
            format!(
                "Expected `impl` for `{}`, `#[instruction_execution]` attribute must be added to \
                enum trait implementation",
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

    let Some((_, trait_path, _)) = &item_impl.trait_ else {
        return Err(Error::new(
            item_impl.span(),
            format!(
                "Expected `#[instruction_execution] impl ExecutableInstructionOperands for {0}` or \
                `#[instruction_execution] impl ExecutableInstructionCsr for {0}` or \
                `#[instruction_execution] impl ExecutableInstruction for {0}`, but no trait was \
                found",
                item_impl.self_ty.to_token_stream()
            ),
        ));
    };

    let last_trait_segment_path = trait_path
        .segments
        .last()
        .expect("Path is never empty; qed");

    if last_trait_segment_path.ident == "ExecutableInstructionOperands" {
        let enum_file_path = format!("/{}_operands_impl.rs", enum_name);

        // Replace enum implementation with a processed impl stored in a Rust file
        Ok(quote! {
            include!(concat!(env!("OUT_DIR"), #enum_file_path));
        })
    } else if last_trait_segment_path.ident == "ExecutableInstructionCsr" {
        let enum_file_path = format!("/{}_csr_impl.rs", enum_name);

        // Replace enum implementation with a processed impl stored in a Rust file
        Ok(quote! {
            include!(concat!(env!("OUT_DIR"), #enum_file_path));
        })
    } else if last_trait_segment_path.ident == "ExecutableInstruction" {
        let enum_file_path = format!("/{}_execution_impl.rs", enum_name);

        // Replace enum implementation with a processed impl stored in a Rust file
        Ok(quote! {
            include!(concat!(env!("OUT_DIR"), #enum_file_path));
        })
    } else {
        Err(Error::new(
            item_impl.span(),
            format!(
                "Expected `impl` for `{}`, `#[instruction_execution]` attribute must be added to a \
                trait implementation, but trait `{}` is not supported",
                item_impl.self_ty.to_token_stream(),
                last_trait_segment_path.ident
            ),
        ))
    }
}
