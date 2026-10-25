use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::{Error, Lifetime, Type};

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

struct ReplaceLifetimes<'a> {
    lifetime: &'a Lifetime,
    keep_static: bool,
    replaced: bool,
}

impl VisitMut for ReplaceLifetimes<'_> {
    #[expect(
        clippy::renamed_function_params,
        reason = "`syn` calls it `i`, which says nothing about what it is"
    )]
    fn visit_lifetime_mut(&mut self, lifetime: &mut Lifetime) {
        if self.keep_static && lifetime.ident == "static" {
            return;
        }
        let mut ident = self.lifetime.ident.clone();
        // Errors about the lifetime point to where it was written
        ident.set_span(lifetime.ident.span());
        lifetime.ident = ident;
        self.replaced = true;
    }
}

/// Replace every lifetime in `ty` with `lifetime` and return whether there were any.
///
/// Containers like `VariableBytes<'_, 1024>` have a lifetime parameter. Generated code can't use
/// the lifetimes of the method signature, since they are not in scope there.
pub(super) fn replace_lifetimes(ty: &mut Type, lifetime: &Lifetime) -> bool {
    let mut visitor = ReplaceLifetimes {
        lifetime,
        keep_static: false,
        replaced: false,
    };
    visitor.visit_type_mut(ty);
    visitor.replaced
}

/// `ty` with every lifetime replaced with `lifetime`, like `'static` or `'_`
pub(super) fn with_lifetime(ty: &Type, lifetime: &str) -> Type {
    let mut ty = ty.clone();
    replace_lifetimes(&mut ty, &Lifetime::new(lifetime, Span::call_site()));
    ty
}

/// `ty` with every lifetime except `'static` replaced with `'_`.
///
/// Generated code can't name the lifetimes of the method signature, so the compiler infers them
/// instead. `'static` stays, since it can be named anywhere.
pub(super) fn with_inferred_lifetimes(ty: &Type) -> Type {
    let mut ty = ty.clone();
    ReplaceLifetimes {
        lifetime: &Lifetime::new("'_", Span::call_site()),
        keep_static: true,
        replaced: false,
    }
    .visit_type_mut(&mut ty);
    ty
}
