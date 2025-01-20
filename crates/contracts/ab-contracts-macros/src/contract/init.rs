use crate::contract::methods::{MethodDetails, MethodType};
use crate::contract::{ContractDetails, Method, MethodOutput};
use quote::format_ident;
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::{Error, FnArg, Meta, Signature, Type};

pub(super) fn process_init_fn(
    self_type: Type,
    fn_sig: &mut Signature,
    contract_details: &mut ContractDetails,
) -> Result<MethodOutput, Error> {
    let mut methods_details = MethodDetails::new(MethodType::Init, self_type);

    methods_details.process_output(&fn_sig.output)?;

    for input in fn_sig.inputs.iter_mut() {
        let input_span = input.span();
        // TODO: Moving this outside of the loop causes confusing lifetime issues
        let supported_attrs = HashMap::<_, fn(_, _, _) -> _>::from_iter([
            (format_ident!("env"), MethodDetails::process_env_arg_rw as _),
            (format_ident!("tmp"), MethodDetails::process_tmp_arg as _),
            (
                format_ident!("slot"),
                MethodDetails::process_slot_arg_rw as _,
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
            FnArg::Receiver(_receiver) => {
                return Err(Error::new(
                    fn_sig.span(),
                    "`#[init]` must return `Self`, not take it as an argument",
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
                        "Each `#[init]` argument must be annotated with exactly one of: `#[env]` \
                        or `#[input]`, in that order",
                    ));
                };

                if let Some(next_attr) = attrs.take(1).next() {
                    return Err(Error::new(
                        next_attr.span(),
                        "Each `#[init]` argument must be annotated with exactly one of: `#[env]` \
                        or `#[input]`, in that order",
                    ));
                }

                let processor = supported_attrs
                    .get(&attr.path().segments[0].ident)
                    .expect("Matched above to be one of the supported attributes; qed");

                processor(&mut methods_details, input_span, &*pat_type)?;
            }
        }
    }

    let guest_ffi = methods_details.generate_guest_ffi(fn_sig)?;
    let trait_ext_components = methods_details.generate_trait_ext_components(fn_sig);

    contract_details.methods.push(Method {
        original_ident: fn_sig.ident.clone(),
        methods_details,
    });

    Ok(MethodOutput {
        guest_ffi,
        trait_ext_components,
    })
}
