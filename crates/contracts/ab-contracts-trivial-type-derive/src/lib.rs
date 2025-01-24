use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::iter;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::{
    Attribute, Data, DataEnum, DataStruct, DeriveInput, Error, Fields, LitInt, parenthesized,
    parse_macro_input,
};

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

    let maybe_repr_attr = input.attrs.iter().find(|attr| attr.path().is_ident("repr"));

    let Some(repr_attr) = maybe_repr_attr else {
        return Error::new(input.ident.span(), "`TrivialType` requires `#[repr(..)]`")
            .to_compile_error()
            .into();
    };

    let (repr_c, repr_transparent, repr_numeric, repr_align, repr_packed) =
        match parse_repr(repr_attr) {
            Ok(result) => result,
            Err(error) => {
                return error.to_compile_error().into();
            }
        };

    if repr_align.is_some() || repr_packed.is_some() {
        return Error::new(
            input.ident.span(),
            "`TrivialType` doesn't allow `#[repr(align(N))]` or `#[repr(packed(N))]",
        )
        .to_compile_error()
        .into();
    }

    let type_name = &input.ident;

    let output = match &input.data {
        Data::Struct(data_struct) => {
            if !(repr_c || repr_transparent) {
                return Error::new(
                    input.ident.span(),
                    "`TrivialType` on structs requires `#[repr(C)]` or `#[repr(transparent)]",
                )
                .into_compile_error()
                .into();
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
                    assert!(
                        0 == (
                            ::core::mem::size_of::<#type_name>()
                            #(- ::core::mem::size_of::<#field_types>() )*
                        ),
                        "Struct must not have implicit padding"
                    );

                    // Assert that type doesn't exceed 32-bit size limit
                    assert!(
                        u32::MAX as usize >= ::core::mem::size_of::<#type_name>(),
                        "Type size must be smaller than 2^32"
                    );
                };

                #[automatically_derived]
                unsafe impl ::ab_contracts_io_type::trivial_type::TrivialType for #type_name
                where
                    #( #field_types: ::ab_contracts_io_type::trivial_type::TrivialType, )*
                {
                    const METADATA: &[u8] = #struct_metadata;
                }
            }
        }
        Data::Enum(data_enum) => {
            // Require defined size of the discriminant instead of allowing compiler to guess
            if repr_numeric != Some(8) {
                return Error::new(
                    input.generics.span(),
                    "`TrivialType` derive for enums only supports `#[repr(u8)]`, ambiguous \
                    or larger discriminant size is not allowed",
                )
                .to_compile_error()
                .into();
            }

            let repr_numeric = format_ident!("u8");

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
                    assert!(
                        0 == (
                            ::core::mem::size_of::<#type_name>()
                            - ::core::mem::size_of::<#repr_numeric>()
                            #(- ::core::mem::size_of::<#field_types>() )*
                        ),
                        "Enum must not have implicit padding"
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
                const _: () = {
                    // Assert that type doesn't exceed 32-bit size limit
                    assert!(
                        u32::MAX as usize >= ::core::mem::size_of::<#type_name>(),
                        "Type size must be smaller than 2^32"
                    );

                    #( #padding_assertions )*;
                };

                //#[automatically_derived]
                unsafe impl ::ab_contracts_io_type::trivial_type::TrivialType for #type_name
                where
                    #( #field_types: ::ab_contracts_io_type::trivial_type::TrivialType, )*
                {
                    const METADATA: &[u8] = #enum_metadata;
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

#[allow(clippy::type_complexity, reason = "Private one-off function")]
fn parse_repr(
    repr_attr: &Attribute,
) -> Result<(bool, bool, Option<u8>, Option<usize>, Option<usize>), Error> {
    let mut repr_c = false;
    let mut repr_transparent = false;
    let mut repr_numeric = None::<u8>;
    let mut repr_align = None::<usize>;
    let mut repr_packed = None::<usize>;

    // Based on https://docs.rs/syn/2.0.93/syn/struct.Attribute.html#method.parse_nested_meta
    repr_attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("C") {
            repr_c = true;
            return Ok(());
        }
        if meta.path.is_ident("u8") {
            repr_numeric.replace(8);
            return Ok(());
        }
        if meta.path.is_ident("u16") {
            repr_numeric.replace(16);
            return Ok(());
        }
        if meta.path.is_ident("u32") {
            repr_numeric.replace(32);
            return Ok(());
        }
        if meta.path.is_ident("u64") {
            repr_numeric.replace(64);
            return Ok(());
        }
        if meta.path.is_ident("u128") {
            repr_numeric.replace(128);
            return Ok(());
        }
        if meta.path.is_ident("transparent") {
            repr_transparent = true;
            return Ok(());
        }

        // #[repr(align(N))]
        if meta.path.is_ident("align") {
            let content;
            parenthesized!(content in meta.input);
            let lit = content.parse::<LitInt>()?;
            let n = lit.base10_parse::<usize>()?;
            repr_align = Some(n);
            return Ok(());
        }

        // #[repr(packed)] or #[repr(packed(N))], omitted N means 1
        if meta.path.is_ident("packed") {
            if meta.input.peek(Paren) {
                let content;
                parenthesized!(content in meta.input);
                let lit = content.parse::<LitInt>()?;
                let n = lit.base10_parse::<usize>()?;
                repr_packed = Some(n);
            } else {
                repr_packed = Some(1);
            }
            return Ok(());
        }

        Err(meta.error("Unsupported `#[repr(..)]`"))
    })?;

    Ok((
        repr_c,
        repr_transparent,
        repr_numeric,
        repr_align,
        repr_packed,
    ))
}

fn generate_struct_metadata(ident: &Ident, data_struct: &DataStruct) -> Result<TokenStream, Error> {
    let num_fields = data_struct.fields.len();
    let struct_with_fields = data_struct
        .fields
        .iter()
        .next()
        .map(|field| field.ident.is_some())
        .unwrap_or_default();
    let (io_type_metadata, with_num_fields) = if struct_with_fields {
        match num_fields {
            0..=16 => (format_ident!("Struct{num_fields}"), false),
            _ => (format_ident!("Struct"), true),
        }
    } else {
        match num_fields {
            1..=16 => (format_ident!("TupleStruct{num_fields}"), false),
            _ => (format_ident!("TupleStruct"), true),
        }
    };
    let inner_struct_metadata =
        generate_inner_struct_metadata(ident, &data_struct.fields, with_num_fields)
            .collect::<Result<Vec<_>, _>>()?;

    // Encodes the following:
    // * Type: struct
    // * The rest as inner struct metadata
    Ok(quote! {{
        const fn metadata()
            -> ([u8; ::ab_contracts_io_type::metadata::MAX_METADATA_CAPACITY], usize)
        {
            ::ab_contracts_io_type::metadata::concat_metadata_sources(&[
                &[::ab_contracts_io_type::metadata::IoTypeMetadataKind::#io_type_metadata as u8],
                #( #inner_struct_metadata )*
            ])
        }

        // Strange syntax to allow Rust to extend lifetime of metadata scratch automatically
        metadata()
            .0
            .split_at(metadata().1)
            .0
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
    let variant_has_fields = data_enum
        .variants
        .iter()
        .next()
        .map(|variant| !variant.fields.is_empty())
        .unwrap_or_default();
    let enum_type = if variant_has_fields {
        "Enum"
    } else {
        "EnumNoFields"
    };
    let (io_type_metadata, with_num_variants) = match num_variants {
        1..=16 => (format_ident!("{enum_type}{num_variants}"), false),
        _ => (format_ident!("{enum_type}"), true),
    };

    // Encodes the following:
    // * Type: enum
    // * Length of enum name in bytes (u8)
    // * Enum name as UTF-8 bytes
    // * Number of variants (u8, if requested)
    let enum_metadata_header = {
        let enum_metadata_header = [Literal::u8_unsuffixed(type_name_bytes_len)]
            .into_iter()
            .chain(
                type_name_bytes
                    .iter()
                    .map(|&char| Literal::byte_character(char)),
            )
            .chain(with_num_variants.then_some(Literal::u8_unsuffixed(num_variants)));

        quote! {
            &[
                ::ab_contracts_io_type::metadata::IoTypeMetadataKind::#io_type_metadata as u8,
                #( #enum_metadata_header, )*
            ]
        }
    };

    // Encodes each variant as inner struct
    let inner = data_enum
        .variants
        .iter()
        .flat_map(|variant| {
            variant
                .fields
                .iter()
                .find_map(|field| {
                    if field.ident.is_none() {
                        Some(Err(Error::new(
                            field.span(),
                            "Variant must have named fields",
                        )))
                    } else {
                        None
                    }
                })
                .into_iter()
                .chain(generate_inner_struct_metadata(
                    &variant.ident,
                    &variant.fields,
                    true,
                ))
        })
        .collect::<Result<Vec<TokenStream>, Error>>()?;

    Ok(quote! {{
        const fn metadata()
            -> ([u8; ::ab_contracts_io_type::metadata::MAX_METADATA_CAPACITY], usize)
        {
            ::ab_contracts_io_type::metadata::concat_metadata_sources(&[
                #enum_metadata_header,
                #( #inner )*
            ])
        }

        // Strange syntax to allow Rust to extend lifetime of metadata scratch automatically
        metadata()
            .0
            .split_at(metadata().1)
            .0
    }})
}

fn generate_inner_struct_metadata<'a>(
    ident: &'a Ident,
    fields: &'a Fields,
    with_num_fields: bool,
) -> impl Iterator<Item = Result<TokenStream, Error>> + 'a {
    iter::once_with(move || generate_inner_struct_metadata_header(ident, fields, with_num_fields))
        .chain(generate_fields_metadata(fields))
}

fn generate_inner_struct_metadata_header(
    ident: &Ident,
    fields: &Fields,
    with_num_fields: bool,
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
            format!("Must not have more than {} field", u8::MAX),
        )
    })?;

    // Encodes the following:
    // * Length of identifier in bytes (u8)
    // * Identifier as UTF-8 bytes
    // * Number of fields (u8, if requested)
    Ok({
        let struct_metadata_header = [Literal::u8_unsuffixed(ident_bytes_len)]
            .into_iter()
            .chain(
                ident_bytes
                    .iter()
                    .map(|&char| Literal::byte_character(char)),
            )
            .chain(with_num_fields.then_some(Literal::u8_unsuffixed(num_fields)));

        quote! {
            &[#( #struct_metadata_header, )*],
        }
    })
}

fn generate_fields_metadata(
    fields: &Fields,
) -> impl Iterator<Item = Result<TokenStream, Error>> + '_ {
    // Encodes the following for each field:
    // * Length of the field name in bytes (u8, if not tuple)
    // * Field name as UTF-8 bytes (if not tuple)
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

            Some(quote! { #( #field_metadata, )* })
        } else {
            None
        };
        let field_type = &field.ty;

        Ok(quote! {
            &[ #field_metadata ],
            <#field_type as ::ab_contracts_io_type::trivial_type::TrivialType>::METADATA,
        })
    })
}
