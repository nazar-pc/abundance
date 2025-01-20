mod common;
mod init;
mod methods;
mod update;
mod view;

use crate::contract::init::process_init_fn;
use crate::contract::methods::{ExtTraitComponents, MethodDetails};
use crate::contract::update::process_update_fn;
use crate::contract::view::process_view_fn;
use proc_macro2::{Ident, Literal, TokenStream};
use quote::{format_ident, quote};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::{parse2, Error, ImplItem, ImplItemFn, ItemImpl, Meta, Type, Visibility};

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
    let item_impl = parse2::<ItemImpl>(item)?;

    process_struct_impl(item_impl)
}

fn process_struct_impl(mut item_impl: ItemImpl) -> Result<TokenStream, Error> {
    let struct_name = item_impl.self_ty.as_ref();

    if let Some(trait_) = item_impl.trait_ {
        return Err(Error::new(
            trait_.1.span(),
            "`#[contract]` is not applicable to trait implementations",
        ));
    }

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
            let method_output =
                process_fn(struct_name.clone(), impl_item_fn, &mut contract_details)?;
            guest_ffis.push(method_output.guest_ffi);
            trait_ext_components.push(method_output.trait_ext_components);
        }
    }

    let same_tmp_types = MethodDetails::same_tmp_types(
        contract_details
            .methods
            .iter()
            .map(|method| &method.methods_details),
    );
    if !same_tmp_types {
        return Err(Error::new(
            item_impl.span(),
            "All `#[tmp]` arguments must be of the same type",
        ));
    }

    let same_slot_types = MethodDetails::same_slot_types(
        contract_details
            .methods
            .iter()
            .map(|method| &method.methods_details),
    );
    if !same_slot_types {
        return Err(Error::new(
            item_impl.span(),
            "All `#[slot]` arguments must be of the same type",
        ));
    }

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
            /// Main contract metadata, see [`ContractMetadataKind`] for encoding details.
            ///
            /// More metadata can be contributed by trait implementations.
            ///
            /// [`ContractMetadataKind`]: ::ab_contracts_common::ContractMetadataKind
            pub const MAIN_CONTRACT_METADATA: &[u8] = {
                const fn metadata() -> ([u8; 4096], usize) {
                    ::ab_contracts_io_type::utils::concat_metadata_sources(&[
                        &[::ab_contracts_common::ContractMetadataKind::Contract as u8],
                        <#struct_name as ::ab_contracts_io_type::IoType>::METADATA,
                        &[#num_methods],
                        #( ffi::#methods::METADATA, )*
                    ])
                }

                // Strange syntax to allow Rust to extend lifetime of metadata scratch
                // automatically
                metadata()
                    .0
                    .split_at(metadata().1)
                    .0
            };
        }
    };

    item_impl
        .items
        .insert(0, ImplItem::Verbatim(metadata_const));

    let ext_trait = {
        let Type::Path(type_path) = struct_name else {
            return Err(Error::new(
                struct_name.span(),
                "`#[contract]` must be applied to simple struct implementation",
            ));
        };
        let Some(struct_name) = type_path.path.get_ident() else {
            return Err(Error::new(
                struct_name.span(),
                "`#[contract]` must be applied to simple struct implementation",
            ));
        };

        generate_extension_trait(struct_name, &trait_ext_components)?
    };

    Ok(quote! {
        /// Main contract metadata
        ///
        /// Enabled with `guest` feature to appear in the final binary, also prevents from
        /// `guest` feature being enabled in dependencies at the same time since that'll cause
        /// duplicated symbols.
        ///
        /// See [`#struct_name::MAIN_CONTRACT_METADATA`] for details.
        #[cfg(feature = "guest")]
        #[used]
        #[unsafe(no_mangle)]
        #[unsafe(link_section = "CONTRACT_METADATA")]
        static MAIN_CONTRACT_METADATA: [u8; #struct_name::MAIN_CONTRACT_METADATA.len()] =
            unsafe { *#struct_name::MAIN_CONTRACT_METADATA.as_ptr().cast() };

        #item_impl

        #ext_trait

        /// FFI code generated by procedural macro
        pub mod ffi {
            use super::*;

            #( #guest_ffis )*
        }
    })
}

fn process_fn(
    struct_name: Type,
    impl_item_fn: &mut ImplItemFn,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let supported_attrs = HashMap::<_, fn(_, _, _) -> _>::from_iter([
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

    // Make sure function is public
    if !matches!(impl_item_fn.vis, Visibility::Public(_)) {
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
    processor(struct_name, &mut impl_item_fn.sig, contract_details)
}

fn generate_extension_trait(
    ident: &Ident,
    trait_ext_components: &[ExtTraitComponents],
) -> Result<TokenStream, Error> {
    let trait_name = format_ident!("{ident}Ext");
    let trait_doc = format!(
        "Extension trait that provides helper methods for calling [`{ident}`]'s methods on \
        [`Env`](::ab_contracts_common::env::Env) for convenience purposes"
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

        impl #trait_name for ::ab_contracts_common::env::Env {
            #( #impls )*
        }
    })
}
