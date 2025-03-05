use crate::contract::method::{MethodDetails, MethodType};
use crate::contract::{ContractDetails, Method, MethodOutput};
use proc_macro2::Ident;
use quote::format_ident;
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::{Error, FnArg, Meta, Signature, Type, parse_quote};

pub(super) fn process_view_fn(
    self_type: Type,
    trait_name: Option<&Ident>,
    fn_sig: &mut Signature,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let mut methods_details = MethodDetails::new(MethodType::View, self_type);

    for input in &mut fn_sig.inputs {
        let input_span = input.span();
        // TODO: Moving this outside of the loop causes confusing lifetime issues
        // TODO: Simplify `-> Result<(), Error>` to `-> _` once https://github.com/rust-lang/rust/issues/135864 is resolved
        let supported_attrs = HashMap::<_, fn(_, _, _) -> Result<(), Error>>::from_iter([
            (format_ident!("env"), MethodDetails::process_env_arg_ro as _),
            (
                format_ident!("slot"),
                MethodDetails::process_slot_arg_ro as _,
            ),
            (
                format_ident!("input"),
                MethodDetails::process_input_arg as _,
            ),
            (
                format_ident!("output"),
                MethodDetails::process_output_arg as _,
            ),
        ]);

        match input {
            FnArg::Receiver(receiver) => {
                if receiver.reference.is_none() {
                    return Err(Error::new(
                        fn_sig.span(),
                        "`#[view]` can't consume `Self` or `&mut self`, use `&self` instead",
                    ));
                }

                methods_details.process_state_arg_ro(input_span, &receiver.ty)?;
            }
            FnArg::Typed(pat_type) => {
                let mut attrs = pat_type.attrs.extract_if(.., |attr| match &attr.meta {
                    Meta::Path(path) => {
                        path.leading_colon.is_none()
                            && path.segments.len() == 1
                            && supported_attrs.contains_key(&path.segments[0].ident)
                    }
                    Meta::List(_meta_list) => false,
                    Meta::NameValue(_meta_name_value) => false,
                });

                let Some(attr) = attrs.next() else {
                    return Err(Error::new(
                        input_span,
                        "Each `#[view]` argument (except `&self`) must be annotated with \
                        exactly one of: `#[env]`, `#[slot]`, `#[input]` or `#[output]`, in that \
                        order",
                    ));
                };

                if let Some(next_attr) = attrs.take(1).next() {
                    return Err(Error::new(
                        next_attr.span(),
                        "Each `#[view]` argument (except `&self`) must be annotated with \
                        exactly one of: `#[env]`, `#[slot]`, `#[input]` or `#[output]`, in that \
                        order",
                    ));
                }

                let processor = supported_attrs
                    .get(&attr.path().segments[0].ident)
                    .expect("Matched above to be one of the supported attributes; qed");

                processor(&mut methods_details, input_span, &*pat_type)?;
            }
        }
    }

    methods_details.process_return(&fn_sig.output)?;

    let guest_ffi = methods_details.generate_guest_ffi(fn_sig, trait_name)?;
    let trait_ext_components = methods_details.generate_trait_ext_components(fn_sig, trait_name)?;

    contract_details.methods.push(Method {
        original_ident: fn_sig.ident.clone(),
        methods_details,
    });

    Ok(MethodOutput {
        guest_ffi,
        trait_ext_components,
    })
}

pub(super) fn process_view_fn_definition(
    trait_name: &Ident,
    fn_sig: &mut Signature,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let mut methods_details =
        MethodDetails::new(MethodType::View, Type::Path(parse_quote! { #trait_name }));

    for input in &mut fn_sig.inputs {
        let input_span = input.span();
        // TODO: Moving this outside of the loop causes confusing lifetime issues
        // TODO: Simplify `-> Result<(), Error>` to `-> _` once https://github.com/rust-lang/rust/issues/135864 is resolved
        let supported_attrs = HashMap::<_, fn(_, _, _) -> Result<(), Error>>::from_iter([
            (format_ident!("env"), MethodDetails::process_env_arg_ro as _),
            (
                format_ident!("input"),
                MethodDetails::process_input_arg as _,
            ),
            (
                format_ident!("output"),
                MethodDetails::process_output_arg as _,
            ),
        ]);

        match input {
            FnArg::Receiver(_receiver) => {
                return Err(Error::new(
                    fn_sig.span(),
                    "`#[view]` in trait definition can't access state through `Self` directly, \
                    call implementation method through `Env` instead if necessary",
                ));
            }
            FnArg::Typed(pat_type) => {
                let mut attrs = pat_type.attrs.extract_if(.., |attr| match &attr.meta {
                    Meta::Path(path) => {
                        path.leading_colon.is_none()
                            && path.segments.len() == 1
                            && supported_attrs.contains_key(&path.segments[0].ident)
                    }
                    Meta::List(_meta_list) => false,
                    Meta::NameValue(_meta_name_value) => false,
                });

                let Some(attr) = attrs.next() else {
                    return Err(Error::new(
                        input_span,
                        "Each `#[view]` argument must be annotated with exactly one of: \
                        `#[env]`, `#[input]` or `#[output]`, in that order",
                    ));
                };

                if let Some(next_attr) = attrs.take(1).next() {
                    return Err(Error::new(
                        next_attr.span(),
                        "Each `#[view]` argument must be annotated with exactly one of: \
                        `#[env]`, `#[input]` or `#[output]`, in that order",
                    ));
                }

                let processor = supported_attrs
                    .get(&attr.path().segments[0].ident)
                    .expect("Matched above to be one of the supported attributes; qed");

                processor(&mut methods_details, input_span, &*pat_type)?;
            }
        }
    }

    methods_details.process_return(&fn_sig.output)?;

    let guest_ffi = methods_details.generate_guest_trait_ffi(fn_sig, Some(trait_name))?;
    let trait_ext_components =
        methods_details.generate_trait_ext_components(fn_sig, Some(trait_name))?;

    contract_details.methods.push(Method {
        original_ident: fn_sig.ident.clone(),
        methods_details,
    });

    Ok(MethodOutput {
        guest_ffi,
        trait_ext_components,
    })
}
