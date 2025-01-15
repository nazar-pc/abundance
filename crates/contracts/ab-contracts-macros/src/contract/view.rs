use crate::contract::methods::{MethodDetails, MethodType};
use crate::contract::{ContractDetails, Method, MethodOutput};
use quote::format_ident;
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItemFn, Meta, Type};

pub(super) fn process_view_fn(
    struct_name: Type,
    impl_item_fn: &mut ImplItemFn,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let mut methods_details = MethodDetails::new(MethodType::View, struct_name);

    methods_details.process_output(&impl_item_fn.sig.output)?;

    for input in impl_item_fn.sig.inputs.iter_mut() {
        let input_span = input.span();
        // TODO: Moving this outside of the loop causes confusing lifetime issues
        let supported_attrs = HashMap::<_, fn(_, _, _) -> _>::from_iter([
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
            (
                format_ident!("result"),
                MethodDetails::process_result_arg as _,
            ),
        ]);

        match input {
            FnArg::Receiver(receiver) => {
                if receiver.reference.is_none() {
                    return Err(Error::new(
                        impl_item_fn.sig.span(),
                        "`#[view]` can't consume `Self` or `&mut self`, use `&self` instead",
                    ));
                }

                MethodDetails::process_state_arg_ro(
                    input_span,
                    &receiver.ty,
                    &mut methods_details,
                )?;
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

                processor(input_span, &*pat_type, &mut methods_details)?;
            }
        }
    }

    let guest_ffi = methods_details.generate_guest_ffi(impl_item_fn)?;
    let trait_ext_components = methods_details.generate_trait_ext_components(impl_item_fn);

    contract_details.methods.push(Method {
        original_ident: impl_item_fn.sig.ident.clone(),
        methods_details,
    });

    Ok(MethodOutput {
        guest_ffi,
        trait_ext_components,
    })
}
