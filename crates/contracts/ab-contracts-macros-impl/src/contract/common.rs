use proc_macro2::{Ident, Literal};
use syn::Error;

pub(super) fn derive_ident_metadata(ident: &Ident) -> Result<impl Iterator<Item = Literal>, Error> {
    let ident_string = ident.to_string();
    let ident_bytes = ident_string.as_bytes().to_vec();
    let ident_bytes_len = u8::try_from(ident_bytes.len()).map_err(|_error| {
        Error::new(
            ident.span(),
            format!(
                "Name of the field must not be more than {} bytes in length",
                u8::MAX
            ),
        )
    })?;

    Ok([Literal::u8_unsuffixed(ident_bytes_len)]
        .into_iter()
        .chain(ident_bytes.into_iter().map(Literal::byte_character)))
}
