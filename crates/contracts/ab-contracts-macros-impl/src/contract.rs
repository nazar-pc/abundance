mod common;
mod init;
mod method;
mod update;
mod view;

use crate::contract::common::{derive_ident_metadata, extract_ident_from_type};
use crate::contract::init::process_init_fn;
use crate::contract::method::{ExtTraitComponents, MethodDetails};
use crate::contract::update::{process_update_fn, process_update_fn_definition};
use crate::contract::view::{process_view_fn, process_view_fn_definition};
use ident_case::RenameRule;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::{
    Error, ImplItem, ImplItemFn, ItemImpl, ItemTrait, Meta, TraitItem, TraitItemConst, TraitItemFn,
    Type, Visibility, parse_quote, parse2,
};

#[derive(Default)]
struct MethodOutput {
    guest_ffi: TokenStream,
    trait_ext_components: ExtTraitComponents,
}

struct Method {
    /// As authored in source code
    original_ident: Ident,
    methods_details: MethodDetails,
}

#[derive(Default)]
struct ContractDetails {
    methods: Vec<Method>,
}

pub(super) fn contract(item: TokenStream) -> Result<TokenStream, Error> {
    if let Ok(item_trait) = parse2::<ItemTrait>(item.clone()) {
        // Trait definition
        return process_trait_definition(item_trait);
    }

    let error_message = "`#[contract]` must be applied to struct implementation, trait definition or trait \
        implementation";

    let item_impl =
        parse2::<ItemImpl>(item).map_err(|error| Error::new(error.span(), error_message))?;

    if let Some((_not, path, _for)) = &item_impl.trait_ {
        let trait_name = path
            .get_ident()
            .ok_or_else(|| Error::new(path.span(), error_message))?
            .clone();
        // Trait implementation
        process_trait_impl(item_impl, &trait_name)
    } else {
        // Implementation of a struct
        process_struct_impl(item_impl)
    }
}

fn process_trait_definition(mut item_trait: ItemTrait) -> Result<TokenStream, Error> {
    let trait_name = &item_trait.ident;

    if !item_trait.generics.params.is_empty() {
        return Err(Error::new(
            item_trait.generics.span(),
            "`#[contract]` does not support generics",
        ));
    }

    let mut guest_ffis = Vec::with_capacity(item_trait.items.len());
    let mut trait_ext_components = Vec::with_capacity(item_trait.items.len());
    let mut contract_details = ContractDetails::default();

    for item in &mut item_trait.items {
        if let TraitItem::Fn(trait_item_fn) = item {
            let method_output =
                process_fn_definition(trait_name, trait_item_fn, &mut contract_details)?;
            guest_ffis.push(method_output.guest_ffi);
            trait_ext_components.push(method_output.trait_ext_components);

            // This is needed to make trait itself object safe, which is in turn used as a hack for
            // some APIs
            if let Some(where_clause) = &mut trait_item_fn.sig.generics.where_clause {
                where_clause.predicates.push(parse_quote! {
                    Self: ::core::marker::Sized
                });
            } else {
                trait_item_fn
                    .sig
                    .generics
                    .where_clause
                    .replace(parse_quote! {
                        where
                            Self: ::core::marker::Sized
                    });
            }
        }
    }

    let metadata_const = generate_trait_metadata(&contract_details, trait_name, item_trait.span())?;
    let ext_trait = generate_extension_trait(trait_name, &trait_ext_components)?;

    Ok(quote! {
        #item_trait

        // `dyn ContractTrait` here is a bit of a hack that allows treating a trait as a type. These
        // constants specifically can't be implemented on a trait itself because that'll make trait
        // not object safe, which is needed for `ContractTrait` that uses a similar hack with
        // `dyn ContractTrait`.
        impl ::ab_contracts_macros::__private::ContractTraitDefinition for dyn #trait_name {
            #[cfg(feature = "guest")]
            #[doc(hidden)]
            const GUEST_FEATURE_ENABLED: () = ();
            #metadata_const
        }

        #ext_trait

        /// FFI code generated by procedural macro
        pub mod ffi {
            use super::*;

            #( #guest_ffis )*
        }
    })
}

fn process_trait_impl(mut item_impl: ItemImpl, trait_name: &Ident) -> Result<TokenStream, Error> {
    let struct_name = item_impl.self_ty.as_ref();

    if !item_impl.generics.params.is_empty() {
        return Err(Error::new(
            item_impl.generics.span(),
            "`#[contract]` does not support generics",
        ));
    }

    let mut guest_ffis = Vec::with_capacity(item_impl.items.len());
    let mut contract_details = ContractDetails::default();

    for item in &mut item_impl.items {
        match item {
            ImplItem::Fn(impl_item_fn) => {
                let method_output = process_fn(
                    struct_name.clone(),
                    Some(trait_name),
                    impl_item_fn,
                    &mut contract_details,
                )?;
                guest_ffis.push(method_output.guest_ffi);

                if let Some(where_clause) = &mut impl_item_fn.sig.generics.where_clause {
                    where_clause.predicates.push(parse_quote! {
                        Self: ::core::marker::Sized
                    });
                } else {
                    impl_item_fn
                        .sig
                        .generics
                        .where_clause
                        .replace(parse_quote! {
                            where
                                Self: ::core::marker::Sized
                        });
                }
            }
            ImplItem::Const(impl_item_const) => {
                if impl_item_const.ident == "METADATA" {
                    return Err(Error::new(
                        impl_item_const.span(),
                        "`#[contract]` doesn't allow overriding `METADATA` constant",
                    ));
                }
            }
            _ => {
                // Ignore
            }
        }
    }

    let static_name = format_ident!(
        "{}_METADATA",
        RenameRule::ScreamingSnakeCase.apply_to_variant(trait_name.to_string())
    );
    let ffi_mod_ident = format_ident!(
        "{}_ffi",
        RenameRule::SnakeCase.apply_to_variant(trait_name.to_string())
    );
    let metadata_const = generate_trait_metadata(&contract_details, trait_name, item_impl.span())?;
    let method_fn_pointers_const = {
        let methods = contract_details
            .methods
            .iter()
            .map(|method| &method.original_ident);

        quote! {
            #[doc(hidden)]
            const NATIVE_EXECUTOR_METHODS: &[::ab_contracts_macros::__private::NativeExecutorContactMethod] = &[
                #( #ffi_mod_ident::#methods::fn_pointer::METHOD_FN_POINTER, )*
            ];
        }
    };

    Ok(quote! {
        /// Contribute trait metadata to contract's metadata
        ///
        /// Enabled with `guest` feature to appear in the final binary.
        ///
        /// See [`Contract::MAIN_CONTRACT_METADATA`] for details.
        ///
        /// [`Contract::MAIN_CONTRACT_METADATA`]: ::ab_contracts_macros::__private::Contract::MAIN_CONTRACT_METADATA
        #[cfg(feature = "guest")]
        #[used]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = "CONTRACT_METADATA")]
        static #static_name: [u8; <dyn #trait_name as ::ab_contracts_macros::__private::ContractTraitDefinition>::METADATA.len()] = unsafe {
            *<dyn #trait_name as ::ab_contracts_macros::__private::ContractTraitDefinition>::METADATA.as_ptr().cast()
        };

        // Sanity check that trait implementation fully matches trait definition
        const _: () = {
            // Import as `ffi` for generated metadata constant to pick up a correct version
            use #ffi_mod_ident as ffi;
            #metadata_const

            // Comparing compact metadata to allow argument name differences and similar things
            // TODO: This two-step awkwardness because simple comparison doesn't work in const
            //  environment yet
            let (impl_compact_metadata, impl_compact_metadata_size) =
                ::ab_contracts_macros::__private::ContractMetadataKind::compact(METADATA)
                    .expect("Generated metadata is correct; qed");
            let (def_compact_metadata, def_compact_metadata_size) =
                ::ab_contracts_macros::__private::ContractMetadataKind::compact(
                    <dyn #trait_name as ::ab_contracts_macros::__private::ContractTraitDefinition>::METADATA,
                )
                    .expect("Generated metadata is correct; qed");
            assert!(
                impl_compact_metadata_size == def_compact_metadata_size,
                "Trait implementation must match trait definition exactly"
            );
            let mut i = 0;
            while impl_compact_metadata_size > i {
                assert!(
                    impl_compact_metadata[i] == def_compact_metadata[i],
                    "Trait implementation must match trait definition exactly"
                );
                i += 1;
            }
        };

        // Ensure `guest` feature is enabled for crate with trait definition
        #[cfg(feature = "guest")]
        const _: () = <dyn #trait_name as ::ab_contracts_macros::__private::ContractTraitDefinition>::GUEST_FEATURE_ENABLED;

        #item_impl

        // `dyn ContractTrait` here is a bit of a hack that allows treating a trait as a type for
        // convenient API in native execution environment
        impl ::ab_contracts_macros::__private::ContractTrait<dyn #trait_name> for #struct_name {
            #method_fn_pointers_const
        }

        /// FFI code generated by procedural macro
        pub mod #ffi_mod_ident {
            use super::*;


            #( #guest_ffis )*
        }
    })
}

fn generate_trait_metadata(
    contract_details: &ContractDetails,
    trait_name: &Ident,
    span: Span,
) -> Result<TraitItemConst, Error> {
    let num_methods = u8::try_from(contract_details.methods.len()).map_err(|_error| {
        Error::new(
            span,
            format!("Trait can't have more than {} methods", u8::MAX),
        )
    })?;
    let num_methods = Literal::u8_unsuffixed(num_methods);
    let methods = contract_details
        .methods
        .iter()
        .map(|method| &method.original_ident);
    let trait_name_metadata = derive_ident_metadata(trait_name)?;

    // Encodes the following:
    // * Type: trait definition
    // * Length of trait name in bytes (u8)
    // * Trait name as UTF-8 bytes
    // * Number of methods
    // * Metadata of methods
    Ok(parse_quote! {
        /// Trait metadata, see [`ContractMetadataKind`] for encoding details
        ///
        /// [`ContractMetadataKind`]: ::ab_contracts_macros::__private::ContractMetadataKind
        const METADATA: &[::core::primitive::u8] = {
            const fn metadata()
                -> ([u8; ::ab_contracts_macros::__private::MAX_METADATA_CAPACITY], usize)
            {
                ::ab_contracts_macros::__private::concat_metadata_sources(&[
                    &[::ab_contracts_macros::__private::ContractMetadataKind::Trait as ::core::primitive::u8],
                    #trait_name_metadata,
                    &[#num_methods],
                    #( ffi::#methods::METADATA, )*
                ])
            }

            // Strange syntax to allow Rust to extend the lifetime of metadata scratch
            // automatically
            metadata()
                .0
                .split_at(metadata().1)
                .0
        };
    })
}

fn process_struct_impl(mut item_impl: ItemImpl) -> Result<TokenStream, Error> {
    let struct_name = item_impl.self_ty.as_ref();

    if !item_impl.generics.params.is_empty() {
        return Err(Error::new(
            item_impl.generics.span(),
            "`#[contract]` does not support generics",
        ));
    }

    let mut guest_ffis = Vec::with_capacity(item_impl.items.len());
    let mut trait_ext_components = Vec::with_capacity(item_impl.items.len());
    let mut contract_details = ContractDetails::default();

    for item in &mut item_impl.items {
        if let ImplItem::Fn(impl_item_fn) = item {
            let method_output = process_fn(
                struct_name.clone(),
                None,
                impl_item_fn,
                &mut contract_details,
            )?;
            guest_ffis.push(method_output.guest_ffi);
            trait_ext_components.push(method_output.trait_ext_components);
        }
    }

    let maybe_slot_type = MethodDetails::slot_type(
        contract_details
            .methods
            .iter()
            .map(|method| &method.methods_details),
    );
    let Some(slot_type) = maybe_slot_type else {
        return Err(Error::new(
            item_impl.span(),
            "All `#[slot]` arguments must be of the same type",
        ));
    };

    let maybe_tmp_type = MethodDetails::tmp_type(
        contract_details
            .methods
            .iter()
            .map(|method| &method.methods_details),
    );

    let Some(tmp_type) = maybe_tmp_type else {
        return Err(Error::new(
            item_impl.span(),
            "All `#[tmp]` arguments must be of the same type",
        ));
    };

    let metadata_const = {
        let num_methods = u8::try_from(contract_details.methods.len()).map_err(|_error| {
            Error::new(
                item_impl.span(),
                format!("Struct can't have more than {} methods", u8::MAX),
            )
        })?;
        let num_methods = Literal::u8_unsuffixed(num_methods);
        let methods = contract_details
            .methods
            .iter()
            .map(|method| &method.original_ident);

        // Encodes the following:
        // * Type: contract
        // * Metadata of the state type
        // * Number of methods
        // * Metadata of methods
        quote! {
            const MAIN_CONTRACT_METADATA: &[::core::primitive::u8] = {
                const fn metadata()
                    -> ([u8; ::ab_contracts_macros::__private::MAX_METADATA_CAPACITY], usize)
                {
                    ::ab_contracts_macros::__private::concat_metadata_sources(&[
                        &[::ab_contracts_macros::__private::ContractMetadataKind::Contract as ::core::primitive::u8],
                        <#struct_name as ::ab_contracts_macros::__private::IoType>::METADATA,
                        <#slot_type as ::ab_contracts_macros::__private::IoType>::METADATA,
                        <#tmp_type as ::ab_contracts_macros::__private::IoType>::METADATA,
                        &[#num_methods],
                        #( ffi::#methods::METADATA, )*
                    ])
                }

                // Strange syntax to allow Rust to extend the lifetime of metadata scratch
                // automatically
                metadata()
                    .0
                    .split_at(metadata().1)
                    .0
            };
        }
    };
    let method_fn_pointers_const = {
        let methods = contract_details
            .methods
            .iter()
            .map(|method| &method.original_ident);

        quote! {
            #[doc(hidden)]
            const NATIVE_EXECUTOR_METHODS: &[::ab_contracts_macros::__private::NativeExecutorContactMethod] = &[
                #( ffi::#methods::fn_pointer::METHOD_FN_POINTER, )*
            ];
        }
    };

    let struct_name_ident = extract_ident_from_type(struct_name).ok_or_else(|| {
        Error::new(
            struct_name.span(),
            "`#[contract]` must be applied to simple struct implementation",
        )
    })?;

    let ext_trait = generate_extension_trait(struct_name_ident, &trait_ext_components)?;

    let struct_name_str = struct_name_ident.to_string();
    Ok(quote! {
        /// Main contract metadata
        ///
        /// Enabled with `guest` feature to appear in the final binary, also prevents from
        /// `guest` feature being enabled in dependencies at the same time since that'll cause
        /// duplicated symbols.
        ///
        /// See [`Contract::MAIN_CONTRACT_METADATA`] for details.
        ///
        /// [`Contract::MAIN_CONTRACT_METADATA`]: ::ab_contracts_macros::__private::Contract::MAIN_CONTRACT_METADATA
        #[cfg(feature = "guest")]
        #[used]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = "CONTRACT_METADATA")]
        static MAIN_CONTRACT_METADATA: [
            u8;
            <#struct_name as ::ab_contracts_macros::__private::Contract>::MAIN_CONTRACT_METADATA
                .len()
        ] = unsafe {
            *<#struct_name as ::ab_contracts_macros::__private::Contract>::MAIN_CONTRACT_METADATA
                .as_ptr()
                .cast()
        };

        impl ::ab_contracts_macros::__private::Contract for #struct_name {
            #metadata_const
            #method_fn_pointers_const
            #[doc(hidden)]
            const CODE: &::core::primitive::str = ::ab_contracts_macros::__private::concatcp!(
                #struct_name_str,
                '[',
                ::core::env!("CARGO_PKG_NAME"),
                '/',
                ::core::file!(),
                ':',
                ::core::line!(),
                ':',
                ::core::column!(),
                ']',
            );
            // Ensure `guest` feature is enabled for `ab-contracts-common` crate
            #[cfg(feature = "guest")]
            #[doc(hidden)]
            const GUEST_FEATURE_ENABLED: () = ();
            type Slot = #slot_type;
            type Tmp = #tmp_type;

            fn code() -> impl ::core::ops::Deref<
                Target = ::ab_contracts_macros::__private::VariableBytes<
                    { ::ab_contracts_macros::__private::MAX_CODE_SIZE },
                >,
            > {
                const fn code_bytes() -> &'static [::core::primitive::u8] {
                    <#struct_name as ::ab_contracts_macros::__private::Contract>::CODE.as_bytes()
                }

                const fn code_size() -> ::core::primitive::u32 {
                    code_bytes().len() as ::core::primitive::u32
                }

                static CODE_SIZE: ::core::primitive::u32 = code_size();

                ::ab_contracts_macros::__private::VariableBytes::from_buffer(
                    code_bytes(),
                    &CODE_SIZE
                )
            }
        }

        #item_impl

        #ext_trait

        /// FFI code generated by procedural macro
        pub mod ffi {
            use super::*;

            #( #guest_ffis )*
        }
    })
}

fn process_fn_definition(
    trait_name: &Ident,
    trait_item_fn: &mut TraitItemFn,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let supported_attrs = HashMap::<_, fn(_, _, _) -> _>::from_iter([
        (format_ident!("update"), process_update_fn_definition as _),
        (format_ident!("view"), process_view_fn_definition as _),
    ]);
    let mut attrs = trait_item_fn.attrs.extract_if(.., |attr| match &attr.meta {
        Meta::Path(path) => {
            path.leading_colon.is_none()
                && path.segments.len() == 1
                && supported_attrs.contains_key(&path.segments[0].ident)
        }
        Meta::List(_meta_list) => false,
        Meta::NameValue(_meta_name_value) => false,
    });

    let Some(attr) = attrs.next() else {
        drop(attrs);

        // Return unmodified original if no recognized arguments are present
        return Ok(MethodOutput::default());
    };

    if let Some(next_attr) = attrs.take(1).next() {
        return Err(Error::new(
            next_attr.span(),
            "Function can only have one of `#[update]` or `#[view]` attributes specified",
        ));
    }

    // Make sure function doesn't have customized ABI
    if let Some(abi) = &trait_item_fn.sig.abi {
        return Err(Error::new(
            abi.span(),
            format!(
                "Function with `#[{}]` attribute must have default ABI",
                attr.meta.path().segments[0].ident
            ),
        ));
    }

    if trait_item_fn.default.is_some() {
        return Err(Error::new(
            trait_item_fn.span(),
            "`#[contract]` does not support `#[update]` or `#[view]` methods with default implementation \
            in trait definition",
        ));
    }

    let processor = supported_attrs
        .get(&attr.path().segments[0].ident)
        .expect("Matched above to be one of the supported attributes; qed");
    processor(trait_name, &mut trait_item_fn.sig, contract_details)
}

fn process_fn(
    struct_name: Type,
    trait_name: Option<&Ident>,
    impl_item_fn: &mut ImplItemFn,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let supported_attrs = HashMap::<_, fn(_, _, _, _) -> _>::from_iter([
        (format_ident!("init"), process_init_fn as _),
        (format_ident!("update"), process_update_fn as _),
        (format_ident!("view"), process_view_fn as _),
    ]);
    let mut attrs = impl_item_fn.attrs.extract_if(.., |attr| match &attr.meta {
        Meta::Path(path) => {
            path.leading_colon.is_none()
                && path.segments.len() == 1
                && supported_attrs.contains_key(&path.segments[0].ident)
        }
        Meta::List(_meta_list) => false,
        Meta::NameValue(_meta_name_value) => false,
    });

    let Some(attr) = attrs.next() else {
        drop(attrs);

        // Return unmodified original if no recognized arguments are present
        return Ok(MethodOutput::default());
    };

    if let Some(next_attr) = attrs.take(1).next() {
        return Err(Error::new(
            next_attr.span(),
            "Function can only have one of `#[init]`, `#[update]` or `#[view]` attributes specified",
        ));
    }

    // Make sure function is public if not a trait impl
    if !(matches!(impl_item_fn.vis, Visibility::Public(_)) || trait_name.is_some()) {
        return Err(Error::new(
            impl_item_fn.sig.span(),
            format!(
                "Function with `#[{}]` attribute must be public",
                attr.meta.path().segments[0].ident
            ),
        ));
    }

    // Make sure function doesn't have customized ABI
    if let Some(abi) = &impl_item_fn.sig.abi {
        return Err(Error::new(
            abi.span(),
            format!(
                "Function with `#[{}]` attribute must have default ABI",
                attr.meta.path().segments[0].ident
            ),
        ));
    }

    let processor = supported_attrs
        .get(&attr.path().segments[0].ident)
        .expect("Matched above to be one of the supported attributes; qed");
    processor(
        struct_name,
        trait_name,
        &mut impl_item_fn.sig,
        contract_details,
    )
}

fn generate_extension_trait(
    ident: &Ident,
    trait_ext_components: &[ExtTraitComponents],
) -> Result<TokenStream, Error> {
    let trait_name = format_ident!("{ident}Ext");
    let trait_doc = format!(
        "Extension trait that provides helper methods for calling [`{ident}`]'s methods on \
        [`Env`](::ab_contracts_macros::__private::Env) for convenience purposes"
    );
    let definitions = trait_ext_components
        .iter()
        .map(|components| &components.definitions);
    let impls = trait_ext_components
        .iter()
        .map(|components| &components.impls);

    Ok(quote! {
        use ffi::*;

        #[doc = #trait_doc]
        pub trait #trait_name {
            #( #definitions )*
        }

        impl #trait_name for ::ab_contracts_macros::__private::Env<'_> {
            #( #impls )*
        }
    })
}
