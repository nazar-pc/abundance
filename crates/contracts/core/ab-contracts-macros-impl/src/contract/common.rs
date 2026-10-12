use proc_macro2::{Ident, Literal, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Error, Type};

pub(super) fn extract_ident_from_type(ty: &Type) -> Option<&Ident> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path.path.get_ident()
}

pub(super) fn derive_ident_metadata(ident: &Ident) -> Result<TokenStream, Error> {
    let ident_string = ident.to_string();
    let ident_bytes = ident_string.as_bytes();
    let ident_bytes_len = u8::try_from(ident_bytes.len()).map_err(|_error| {
        Error::new(
            ident.span(),
            format!(
                "Name of the field must not be more than {} bytes in length",
                u8::MAX
            ),
        )
    })?;
    let ident_bytes = Literal::byte_string(ident_bytes);
    let ident_bytes_len = Literal::u8_unsuffixed(ident_bytes_len);

    Ok(quote! {
        &[#ident_bytes_len], #ident_bytes
    })
}

/// Assertion that metadata of a type decodes with `MAX_METADATA_DEPTH` as the max nesting depth,
/// like contracts decode it
pub(super) fn max_depth_assertion(type_name: &Type, error_message: &str) -> TokenStream {
    quote_spanned! {type_name.span() =>
        assert!(
            ::ab_contracts_macros::__private::IoTypeMetadataKind::type_details::<{
                ::ab_contracts_macros::__private::MAX_METADATA_DEPTH
            }>(<#type_name as ::ab_contracts_macros::__private::IoType>::METADATA)
            .is_some(),
            #error_message
        );
    }
}
