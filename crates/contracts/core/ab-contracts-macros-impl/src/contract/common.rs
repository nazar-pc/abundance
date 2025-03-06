use proc_macro2::{Ident, Literal, TokenStream};
use quote::quote;
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
