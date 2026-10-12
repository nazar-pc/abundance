use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use std::iter;
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Fields, parse_macro_input};

/// Derive macro for `TrivialType` trait of `ab-io-type` crate.
///
/// See the documentation of the trait for supported types and requirements, which are checked at
/// compile time.
#[proc_macro_derive(TrivialType)]
pub fn trivial_type_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    if !input.generics.params.is_empty() {
        return Error::new(
            input.ident.span(),
            "`TrivialType` can't be derived on generic types",
        )
        .into_compile_error()
        .into();
    }

    let type_name = &input.ident;

    let output = match &input.data {
        Data::Struct(data_struct) => {
            if let Err(error) = check_repr(
                &input,
                &["C", "transparent"],
                "`TrivialType` on structs requires `#[repr(C)]` or `#[repr(transparent)]` without \
                other options",
            ) {
                return error.to_compile_error().into();
            }
            let field_types = data_struct
                .fields
                .iter()
                .map(|field| &field.ty)
                .collect::<Vec<_>>();

            let struct_metadata = match generate_struct_metadata(type_name, data_struct) {
                Ok(struct_metadata) => struct_metadata,
                Err(error) => {
                    return error.to_compile_error().into();
                }
            };

            quote! {
                const _: () = {
                    // Assert statically that there is no unexpected padding that would be left
                    // uninitialized and unsound to access
                    ::core::assert!(
                        0 == (
                            ::core::mem::size_of::<#type_name>()
                            #(- ::core::mem::size_of::<#field_types>() )*
                        ),
                        "Struct must not have implicit padding. Consider reordering fields, adding \
                        `padding: [u8; N]` field where necessary or use `Unaligned<T>` wrapper for \
                        types with larger alignment to reduce it to one byte."
                    );

                    // Assert that type doesn't exceed 32-bit size limit
                    ::core::assert!(
                        ::core::primitive::u32::MAX as ::core::primitive::usize
                            >= ::core::mem::size_of::<#type_name>(),
                        "Type size must be smaller than 2^32"
                    );

                    // Ensure metadata decodes completely, and capacity and alignment are correctly
                    // decoded from it. Every level of nesting takes at least one byte of metadata,
                    // so its length is enough as the max nesting depth.
                    let (type_details, remaining_metadata) =
                        ::ab_io_type::metadata::IoTypeMetadataKind::type_details::<{
                            <#type_name as ::ab_io_type::trivial_type::TrivialType>::METADATA.len()
                        }>(
                            <#type_name as ::ab_io_type::trivial_type::TrivialType>::METADATA,
                        )
                            .expect(
                                "Metadata must decode, which is not the case for layouts with \
                                padding or misaligned fields"
                            );
                    ::core::assert!(remaining_metadata.is_empty());
                    ::core::assert!(
                        ::core::mem::size_of::<#type_name>()
                            == type_details.recommended_capacity as ::core::primitive::usize
                    );
                    ::core::assert!(
                        ::core::mem::align_of::<#type_name>()
                            == type_details.alignment.get() as ::core::primitive::usize
                    );
                };

                #[automatically_derived]
                unsafe impl ::ab_io_type::trivial_type::TrivialType for #type_name
                where
                    #( #field_types: ::ab_io_type::trivial_type::TrivialType, )*
                {
                    const METADATA: &[::core::primitive::u8] = #struct_metadata;
                }
            }
        }
        Data::Enum(data_enum) => {
            // Require defined size of the discriminant instead of allowing compiler to guess. With
            // `#[repr(u8)]` each variant is laid out like a `#[repr(C)]` struct with the
            // discriminant as the first field, which padding assertions rely on, `#[repr(C, u8)]`
            // has a different layout.
            if let Err(error) = check_repr(
                &input,
                &["u8"],
                "`TrivialType` on enums requires `#[repr(u8)]` without other options",
            ) {
                return error.to_compile_error().into();
            }

            // Metadata identifies variants by their index, so explicit discriminants must be equal
            // to it, which makes implicit ones equal to it too. Assertions are associated constants
            // of the enum, so `Self` in a discriminant means the same as in the enum definition.
            // They are only evaluated when used, so each is used by its own `const _`, which
            // reports all wrong discriminants.
            let discriminant_assertions = data_enum
                .variants
                .iter()
                // `#[repr(u8)]` allows at most 256 variants
                .zip(0..=u8::MAX)
                .filter_map(|(variant, index)| {
                    let (_eq_token, discriminant) = variant.discriminant.as_ref()?;
                    // Errors point at the discriminant
                    let span = Span::call_site().located_at(discriminant.span());
                    let name = format_ident!("__TRIVIAL_TYPE_DISCRIMINANT_{index}");
                    let message = format!(
                        "Discriminant of `{}` must be equal to its index {index}, variants are \
                        identified by their index in `TrivialType` metadata",
                        variant.ident
                    );

                    Some(quote_spanned! {span=>
                        impl #type_name {
                            // The index goes first, so the discriminant is inferred as `u8` like
                            // in the enum definition
                            #[doc(hidden)]
                            const #name: () = ::core::assert!(#index == (#discriminant), #message);
                        }

                        const _: () = #type_name::#name;
                    })
                })
                .collect::<Vec<_>>();

            let field_types = data_enum
                .variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .map(|field| &field.ty)
                .collect::<Vec<_>>();

            let padding_assertions = data_enum.variants.iter().map(|variant| {
                let field_types = variant.fields.iter().map(|field| &field.ty);

                quote! {
                    // Assert statically that there is no unexpected padding that would be left
                    // uninitialized and unsound to access
                    ::core::assert!(
                        0 == (
                            ::core::mem::size_of::<#type_name>()
                            - ::core::mem::size_of::<::core::primitive::u8>()
                            #(- ::core::mem::size_of::<#field_types>() )*
                        ),
                        "Enum must not have implicit padding. Consider reordering fields, adding \
                        `padding: [u8; N]` field where necessary or use `Unaligned<T>` wrapper for \
                        types with larger alignment to reduce it to one byte."
                    );
                }
            });

            let enum_metadata = match generate_enum_metadata(type_name, data_enum) {
                Ok(struct_metadata) => struct_metadata,
                Err(error) => {
                    return error.to_compile_error().into();
                }
            };

            quote! {
                #( #discriminant_assertions )*

                const _: () = {
                    #( #padding_assertions )*

                    // Assert that type doesn't exceed 32-bit size limit
                    ::core::assert!(
                        ::core::primitive::u32::MAX as ::core::primitive::usize
                            >= ::core::mem::size_of::<#type_name>(),
                        "Type size must be smaller than 2^32"
                    );

                    // Ensure metadata decodes completely, and capacity and alignment are correctly
                    // decoded from it. Every level of nesting takes at least one byte of metadata,
                    // so its length is enough as the max nesting depth.
                    let (type_details, remaining_metadata) =
                        ::ab_io_type::metadata::IoTypeMetadataKind::type_details::<{
                            <#type_name as ::ab_io_type::trivial_type::TrivialType>::METADATA.len()
                        }>(
                            <#type_name as ::ab_io_type::trivial_type::TrivialType>::METADATA,
                        )
                            .expect(
                                "Metadata must decode, which is not the case for layouts with \
                                padding or misaligned fields"
                            );
                    ::core::assert!(remaining_metadata.is_empty());
                    ::core::assert!(
                        ::core::mem::size_of::<#type_name>()
                            == type_details.recommended_capacity as ::core::primitive::usize
                    );
                    ::core::assert!(
                        ::core::mem::align_of::<#type_name>()
                            == type_details.alignment.get() as ::core::primitive::usize
                    );
                };

                #[automatically_derived]
                unsafe impl ::ab_io_type::trivial_type::TrivialType for #type_name
                where
                    #( #field_types: ::ab_io_type::trivial_type::TrivialType, )*
                {
                    const METADATA: &[::core::primitive::u8] = #enum_metadata;
                }
            }
        }
        Data::Union(data_union) => {
            return Error::new(
                data_union.union_token.span(),
                "`TrivialType` can be derived for structs and enums, but not unions",
            )
            .to_compile_error()
            .into();
        }
    };

    output.into()
}

/// Check that `#[repr(..)]` attributes of the type contain at least one of `allowed` options and no
/// other options, returns an error with `message` otherwise
fn check_repr(input: &DeriveInput, allowed: &[&str], message: &str) -> Result<(), Error> {
    let mut found_allowed = false;

    // All `#[repr(..)]` attributes are combined by the compiler
    let repr_attrs = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("repr"));
    for repr_attr in repr_attrs {
        repr_attr.parse_nested_meta(|meta| {
            if allowed.iter().any(|option| meta.path.is_ident(option)) {
                found_allowed = true;
                Ok(())
            } else {
                Err(meta.error(message))
            }
        })?;
    }

    if !found_allowed {
        return Err(Error::new(input.ident.span(), message));
    }

    Ok(())
}

fn generate_struct_metadata(ident: &Ident, data_struct: &DataStruct) -> Result<TokenStream, Error> {
    let inner_struct_metadata = generate_inner_struct_metadata(ident, &data_struct.fields)
        .collect::<Result<Vec<_>, _>>()?;

    // Encodes the following:
    // * Type: struct
    // * The rest as inner struct metadata
    Ok(quote! {{
        #[inline(always)]
        const fn metadata() -> (
            [::core::primitive::u8; ::ab_io_type::metadata::MAX_METADATA_CAPACITY],
            ::core::primitive::usize,
        )
        {
            ::ab_io_type::metadata::concat_metadata_sources(&[
                &[::ab_io_type::metadata::IoTypeMetadataKind::Struct as ::core::primitive::u8],
                #( #inner_struct_metadata )*
            ])
        }

        // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically.
        // `split_at()` is called by path, with method call syntax a method of a trait in scope that
        // is implemented for arrays would be called instead.
        <[::core::primitive::u8]>::split_at(&metadata().0, metadata().1).0
    }})
}

fn generate_enum_metadata(ident: &Ident, data_enum: &DataEnum) -> Result<TokenStream, Error> {
    let type_name_string = ident.to_string();
    let type_name_bytes = type_name_string.as_bytes();

    let type_name_bytes_len = u8::try_from(type_name_bytes.len()).map_err(|_error| {
        Error::new(
            ident.span(),
            format!(
                "Name of the enum must not be more than {} bytes in length",
                u8::MAX
            ),
        )
    })?;
    let num_variants = u8::try_from(data_enum.variants.len()).map_err(|_error| {
        Error::new(
            ident.span(),
            format!("Enum must not have more than {} variants", u8::MAX),
        )
    })?;
    // Encodes the following:
    // * Type: enum
    // * Length of enum name in bytes (u8)
    // * Enum name as UTF-8 bytes
    // * Number of variants (u8)
    let enum_metadata_header = {
        let enum_metadata_header = [Literal::u8_unsuffixed(type_name_bytes_len)]
            .into_iter()
            .chain(
                type_name_bytes
                    .iter()
                    .map(|&char| Literal::byte_character(char)),
            )
            .chain([Literal::u8_unsuffixed(num_variants)]);

        quote! {
            &[
                ::ab_io_type::metadata::IoTypeMetadataKind::Enum as ::core::primitive::u8,
                #( #enum_metadata_header, )*
            ]
        }
    };

    // Encodes each variant as inner struct, fields of tuple variants have zero-length names like
    // fields of tuple structs
    let inner = data_enum
        .variants
        .iter()
        .flat_map(|variant| generate_inner_struct_metadata(&variant.ident, &variant.fields))
        .collect::<Result<Vec<TokenStream>, Error>>()?;

    Ok(quote! {{
        #[inline(always)]
        const fn metadata() -> (
            [::core::primitive::u8; ::ab_io_type::metadata::MAX_METADATA_CAPACITY],
            ::core::primitive::usize,
        )
        {
            ::ab_io_type::metadata::concat_metadata_sources(&[
                #enum_metadata_header,
                #( #inner )*
            ])
        }

        // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically.
        // `split_at()` is called by path, with method call syntax a method of a trait in scope that
        // is implemented for arrays would be called instead.
        <[::core::primitive::u8]>::split_at(&metadata().0, metadata().1).0
    }})
}

fn generate_inner_struct_metadata<'a>(
    ident: &'a Ident,
    fields: &'a Fields,
) -> impl Iterator<Item = Result<TokenStream, Error>> + 'a {
    iter::once_with(move || generate_inner_struct_metadata_header(ident, fields))
        .chain(generate_fields_metadata(fields))
}

fn generate_inner_struct_metadata_header(
    ident: &Ident,
    fields: &Fields,
) -> Result<TokenStream, Error> {
    let ident_string = ident.to_string();
    let ident_bytes = ident_string.as_bytes();

    let ident_bytes_len = u8::try_from(ident_bytes.len()).map_err(|_error| {
        Error::new(
            ident.span(),
            format!(
                "Identifier must not be more than {} bytes in length",
                u8::MAX
            ),
        )
    })?;
    let num_fields = u8::try_from(fields.len()).map_err(|_error| {
        Error::new(
            fields.span(),
            format!("Must not have more than {} fields", u8::MAX),
        )
    })?;

    // Encodes the following:
    // * Length of identifier in bytes (u8)
    // * Identifier as UTF-8 bytes
    // * Number of fields (u8)
    Ok({
        let struct_metadata_header = [Literal::u8_unsuffixed(ident_bytes_len)]
            .into_iter()
            .chain(
                ident_bytes
                    .iter()
                    .map(|&char| Literal::byte_character(char)),
            )
            .chain([Literal::u8_unsuffixed(num_fields)]);

        quote! {
            &[#( #struct_metadata_header, )*],
        }
    })
}

fn generate_fields_metadata(
    fields: &Fields,
) -> impl Iterator<Item = Result<TokenStream, Error>> + '_ {
    // Encodes the following for each field:
    // * Length of the field name in bytes (u8, zero for tuple fields)
    // * Field name as UTF-8 bytes
    // * Recursive metadata of the field's type
    fields.iter().map(move |field| {
        let field_metadata = if let Some(field_name) = &field.ident {
            let field_name_string = field_name.to_string();
            let field_name_bytes = field_name_string.as_bytes();
            let field_name_len = u8::try_from(field_name_bytes.len()).map_err(|_error| {
                Error::new(
                    field.span(),
                    format!(
                        "Name of the field must not be more than {} bytes in length",
                        u8::MAX
                    ),
                )
            })?;

            let field_metadata = [Literal::u8_unsuffixed(field_name_len)].into_iter().chain(
                field_name_bytes
                    .iter()
                    .map(|&char| Literal::byte_character(char)),
            );

            quote! { #( #field_metadata, )* }
        } else {
            quote! { 0, }
        };
        let field_type = &field.ty;

        Ok(quote! {
            &[ #field_metadata ],
            <#field_type as ::ab_io_type::trivial_type::TrivialType>::METADATA,
        })
    })
}
