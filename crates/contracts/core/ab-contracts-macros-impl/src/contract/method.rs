use crate::contract::common::{derive_ident_metadata, extract_ident_from_type};
use ident_case::RenameRule;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    Error, GenericArgument, Pat, PatType, PathArguments, ReturnType, Signature, Token, Type,
    TypeTuple,
};

#[derive(Copy, Clone)]
pub(super) enum MethodType {
    Init,
    Update,
    View,
}

impl MethodType {
    fn attr_str(self) -> &'static str {
        match self {
            MethodType::Init => "init",
            MethodType::Update => "update",
            MethodType::View => "view",
        }
    }
}

#[derive(Clone)]
struct Env {
    arg_name: Ident,
    mutability: Option<Token![mut]>,
}

#[derive(Clone)]
struct Tmp {
    type_name: Type,
    arg_name: Ident,
    mutability: Option<Token![mut]>,
}

#[derive(Clone)]
struct Slot {
    with_address_arg: bool,
    type_name: Type,
    arg_name: Ident,
    mutability: Option<Token![mut]>,
}

#[derive(Clone)]
struct Input {
    type_name: Type,
    arg_name: Ident,
}

#[derive(Clone)]
struct Output {
    type_name: Type,
    arg_name: Ident,
    has_self: bool,
}

enum MethodReturnType {
    /// The function doesn't have any return type defined
    Unit(Type),
    /// Returns a type without [`Result`]
    Regular(Type),
    /// Returns [`Result`], but [`Ok`] variant is `()`
    ResultUnit(Type),
    /// Returns [`Result`], but [`Ok`] variant is not `()`
    Result(Type),
}

impl MethodReturnType {
    fn unit_type() -> Type {
        Type::Tuple(TypeTuple {
            paren_token: Default::default(),
            elems: Default::default(),
        })
    }

    fn unit_return_type(&self) -> bool {
        match self {
            Self::Unit(_) | Self::ResultUnit(_) => true,
            Self::Regular(_) | Self::Result(_) => false,
        }
    }

    fn return_type(&self) -> &Type {
        match self {
            Self::Unit(ty) | Self::Regular(ty) | Self::ResultUnit(ty) | Self::Result(ty) => ty,
        }
    }
}

#[derive(Default)]
pub(super) struct ExtTraitComponents {
    pub(super) definitions: TokenStream,
    pub(super) impls: TokenStream,
}

pub(super) struct MethodDetails {
    method_type: MethodType,
    self_type: Type,
    state: Option<Option<Token![mut]>>,
    env: Option<Env>,
    tmp: Option<Tmp>,
    slots: Vec<Slot>,
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    return_type: MethodReturnType,
}

impl MethodDetails {
    pub(super) fn new(method_type: MethodType, self_type: Type) -> Self {
        Self {
            method_type,
            self_type,
            state: None,
            env: None,
            tmp: None,
            slots: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            return_type: MethodReturnType::Unit(MethodReturnType::unit_type()),
        }
    }

    /// Returns `#[tmp]` type (or `()` if it is not used) if all methods have the same slots type
    pub(super) fn tmp_type<'a, I>(iter: I) -> Option<Type>
    where
        I: Iterator<Item = &'a Self> + 'a,
    {
        let mut tmp_type = None;
        for slot in iter.flat_map(|method_details| &method_details.tmp) {
            match &tmp_type {
                Some(tmp_type) => {
                    if tmp_type != &slot.type_name {
                        return None;
                    }
                }
                None => {
                    tmp_type.replace(slot.type_name.clone());
                }
            }
        }

        Some(tmp_type.unwrap_or_else(MethodReturnType::unit_type))
    }

    /// Returns `#[slot]` type (or `()` if it is not used) if all methods have the same slots type
    pub(super) fn slot_type<'a, I>(iter: I) -> Option<Type>
    where
        I: Iterator<Item = &'a Self> + 'a,
    {
        let mut slot_type = None;
        for slot in iter.flat_map(|method_details| &method_details.slots) {
            match &slot_type {
                Some(slot_type) => {
                    if slot_type != &slot.type_name {
                        return None;
                    }
                }
                None => {
                    slot_type.replace(slot.type_name.clone());
                }
            }
        }

        Some(slot_type.unwrap_or_else(MethodReturnType::unit_type))
    }

    pub(super) fn process_env_arg_ro(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        self.process_env_arg(input_span, pat_type, false)
    }

    pub(super) fn process_env_arg_rw(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        self.process_env_arg(input_span, pat_type, true)
    }

    fn process_env_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
        allow_mut: bool,
    ) -> Result<(), Error> {
        if self.env.is_some()
            || self.tmp.is_some()
            || !(self.inputs.is_empty() && self.outputs.is_empty())
        {
            return Err(Error::new(
                input_span,
                "`#[env]` must be the first non-Self argument and only appear once",
            ));
        }

        if let Type::Reference(type_reference) = &*pat_type.ty
            && let Type::Path(_type_path) = &*type_reference.elem
            && let Pat::Ident(pat_ident) = &*pat_type.pat
        {
            if type_reference.mutability.is_some() && !allow_mut {
                return Err(Error::new(
                    input_span,
                    "`#[env]` is not allowed to mutate data here",
                ));
            }

            self.env.replace(Env {
                arg_name: pat_ident.ident.clone(),
                mutability: type_reference.mutability,
            });
            Ok(())
        } else {
            Err(Error::new(
                pat_type.span(),
                "`#[env]` must be a reference to `Env` type (can be shared or exclusive)",
            ))
        }
    }

    pub(super) fn process_state_arg_ro(
        &mut self,
        input_span: Span,
        ty: &Type,
    ) -> Result<(), Error> {
        self.process_state_arg(input_span, ty, false)
    }

    pub(super) fn process_state_arg_rw(
        &mut self,
        input_span: Span,
        ty: &Type,
    ) -> Result<(), Error> {
        self.process_state_arg(input_span, ty, true)
    }

    fn process_state_arg(
        &mut self,
        input_span: Span,
        ty: &Type,
        allow_mut: bool,
    ) -> Result<(), Error> {
        // Only accept `&self` or `&mut self`
        if let Type::Reference(type_reference) = ty
            && let Type::Path(type_path) = &*type_reference.elem
            && type_path.path.is_ident("Self")
        {
            if type_reference.mutability.is_some() && !allow_mut {
                return Err(Error::new(
                    input_span,
                    "`#[arg]` is not allowed to mutate data here",
                ));
            }

            self.state.replace(type_reference.mutability);
            Ok(())
        } else {
            Err(Error::new(
                ty.span(),
                "Can't consume `Self`, use `&self` or `&mut self` instead",
            ))
        }
    }

    pub(super) fn process_tmp_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        if self.tmp.is_some() || !(self.inputs.is_empty() && self.outputs.is_empty()) {
            return Err(Error::new(
                input_span,
                "`#[tmp]` must appear only once before any `#[input]` or `#[output]`",
            ));
        }

        // Check if input looks like `&Type` or `&mut Type`
        if let Type::Reference(type_reference) = &*pat_type.ty {
            let Some(arg_name) = extract_arg_name(&pat_type.pat) else {
                return Err(Error::new(
                    pat_type.span(),
                    "`#[tmp]` argument name must be either a simple variable or a reference",
                ));
            };

            self.tmp.replace(Tmp {
                type_name: type_reference.elem.as_ref().clone(),
                arg_name,
                mutability: type_reference.mutability,
            });

            return Ok(());
        }

        Err(Error::new(
            pat_type.span(),
            "`#[tmp]` must be a reference to a type implementing `IoTypeOptional` (can be \
            shared or exclusive) like `&MaybeData<Slot>` or `&mut VariableBytes<1024>`",
        ))
    }

    pub(super) fn process_slot_arg_ro(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        self.process_slot_arg(input_span, pat_type, false)
    }

    pub(super) fn process_slot_arg_rw(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        self.process_slot_arg(input_span, pat_type, true)
    }

    fn process_slot_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
        allow_mut: bool,
    ) -> Result<(), Error> {
        if !(self.inputs.is_empty() && self.outputs.is_empty()) {
            return Err(Error::new(
                input_span,
                "`#[slot]` must appear before any `#[input]` or `#[output]`",
            ));
        }

        match &*pat_type.ty {
            // Check if input looks like `&Type` or `&mut Type`
            Type::Reference(type_reference) => {
                if type_reference.mutability.is_some() && !allow_mut {
                    return Err(Error::new(
                        input_span,
                        "`#[slot]` is not allowed to mutate data here",
                    ));
                }

                let Some(arg_name) = extract_arg_name(&pat_type.pat) else {
                    return Err(Error::new(
                        pat_type.span(),
                        "`#[slot]` argument name must be either a simple variable or a reference",
                    ));
                };

                self.slots.push(Slot {
                    with_address_arg: false,
                    type_name: type_reference.elem.as_ref().clone(),
                    arg_name,
                    mutability: type_reference.mutability,
                });
                return Ok(());
            }
            // Check if input looks like `(&Address, &Type)` or `(&Address, &mut Type)`
            Type::Tuple(type_tuple) => {
                if type_tuple.elems.len() == 2
                    && let Type::Reference(address_type) =
                        type_tuple.elems.first().expect("Checked above; qed")
                    && address_type.mutability.is_none()
                    && let Type::Reference(outer_slot_type) =
                        type_tuple.elems.last().expect("Checked above; qed")
                    && let Pat::Tuple(pat_tuple) = &*pat_type.pat
                    && pat_tuple.elems.len() == 2
                    && let Some(slot_arg) = extract_arg_name(&pat_tuple.elems[1])
                {
                    if outer_slot_type.mutability.is_some() && !allow_mut {
                        return Err(Error::new(
                            input_span,
                            "`#[slot]` is not allowed to mutate data here",
                        ));
                    }

                    self.slots.push(Slot {
                        with_address_arg: true,
                        type_name: outer_slot_type.elem.as_ref().clone(),
                        arg_name: slot_arg,
                        mutability: outer_slot_type.mutability,
                    });
                    return Ok(());
                }

                return Err(Error::new(
                    pat_type.span(),
                    "`#[slot]` with address must be a tuple of arguments, each of which is \
                        either a simple variable or a reference",
                ));
            }
            _ => {
                // Ignore
            }
        }

        Err(Error::new(
            pat_type.span(),
            "`#[slot]` must be a reference to a type implementing `IoTypeOptional` (can be \
            shared or exclusive) like `&MaybeData<Slot>` or a tuple of references to address and \
            to slot type like `(&Address, &mut VariableBytes<1024>)`",
        ))
    }

    pub(super) fn process_input_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        if !self.outputs.is_empty() {
            return Err(Error::new(
                input_span,
                "`#[input]` must appear before any `#[output]`",
            ));
        }

        // Ensure input looks like `&Type` or `&mut Type`, but not `Type`
        if let Type::Reference(type_reference) = &*pat_type.ty {
            let Some(arg_name) = extract_arg_name(&pat_type.pat) else {
                return Err(Error::new(
                    pat_type.span(),
                    "`#[input]` argument name must be either a simple variable or a reference",
                ));
            };
            if type_reference.mutability.is_some() {
                return Err(Error::new(
                    input_span,
                    "`#[input]` must be a shared reference",
                ));
            }

            self.inputs.push(Input {
                type_name: type_reference.elem.as_ref().clone(),
                arg_name,
            });

            Ok(())
        } else {
            Err(Error::new(
                pat_type.span(),
                "`#[input]` must be a shared reference to a type",
            ))
        }
    }

    pub(super) fn process_output_arg(
        &mut self,
        _input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        // Ensure input looks like `&mut Type`
        if let Type::Reference(type_reference) = &*pat_type.ty
            && type_reference.mutability.is_some()
        {
            let Pat::Ident(pat_ident) = &*pat_type.pat else {
                return Err(Error::new(
                    pat_type.span(),
                    "`#[output]` argument name must be an exclusive reference",
                ));
            };

            let mut type_name = type_reference.elem.as_ref().clone();
            let mut has_self = false;

            // Replace things like `MaybeData<Self>` with `MaybeData<#self_type>`
            if let Type::Path(type_path) = &mut type_name
                && let Some(path_segment) = type_path.path.segments.first_mut()
                && let PathArguments::AngleBracketed(generic_arguments) =
                    &mut path_segment.arguments
                && let Some(GenericArgument::Type(first_generic_argument)) =
                    generic_arguments.args.first_mut()
                && let Type::Path(type_path) = &first_generic_argument
                && type_path.path.is_ident("Self")
            {
                *first_generic_argument = self.self_type.clone();
                has_self = true;
            }

            self.outputs.push(Output {
                type_name,
                arg_name: pat_ident.ident.clone(),
                has_self,
            });
            Ok(())
        } else {
            Err(Error::new(
                pat_type.span(),
                "`#[output]` must be an exclusive reference to a type implementing \
                `IoTypeOptional`, likely `MaybeData` container",
            ))
        }
    }

    pub(super) fn process_return(&mut self, output: &ReturnType) -> Result<(), Error> {
        // Check if return type is `T` or `Result<T, ContractError>`
        let error_message = format!(
            "`#[{}]` must return `()` or `T` or `Result<T, ContractError>",
            self.method_type.attr_str()
        );
        match output {
            ReturnType::Default => {
                self.set_return_type(MethodReturnType::Unit(MethodReturnType::unit_type()));
            }
            ReturnType::Type(_r_arrow, return_type) => match return_type.as_ref() {
                Type::Array(_type_array) => {
                    self.set_return_type(MethodReturnType::Regular(return_type.as_ref().clone()));
                }
                Type::Path(type_path) => {
                    // Check something with generic rather than a simple type
                    let Some(last_path_segment) = type_path.path.segments.last() else {
                        self.set_return_type(MethodReturnType::Regular(
                            return_type.as_ref().clone(),
                        ));
                        return Ok(());
                    };

                    // Check for `-> Result<T, ContractError>`
                    if last_path_segment.ident == "Result" {
                        if let PathArguments::AngleBracketed(result_arguments) =
                            &last_path_segment.arguments
                            && result_arguments.args.len() == 2
                            && let GenericArgument::Type(ok_type) = &result_arguments.args[0]
                            && let GenericArgument::Type(error_type) = &result_arguments.args[1]
                            && let Type::Path(error_path) = error_type
                            && error_path
                                .path
                                .segments
                                .last()
                                .is_some_and(|s| s.ident == "ContractError")
                        {
                            if let Type::Path(ok_path) = ok_type
                                && ok_path
                                    .path
                                    .segments
                                    .first()
                                    .is_some_and(|s| s.ident == "Self")
                            {
                                // Swap `Self` for an actual struct name
                                self.set_return_type(MethodReturnType::Result(
                                    self.self_type.clone(),
                                ));
                            } else {
                                self.set_return_type(MethodReturnType::Result(ok_type.clone()));
                            }
                        } else {
                            return Err(Error::new(return_type.span(), error_message));
                        }
                    } else if last_path_segment.ident == "Self" {
                        // Swap `Self` for an actual struct name
                        self.set_return_type(MethodReturnType::Regular(self.self_type.clone()));
                    } else {
                        self.set_return_type(MethodReturnType::Regular(
                            return_type.as_ref().clone(),
                        ));
                    }
                }
                return_type => {
                    return Err(Error::new(return_type.span(), error_message));
                }
            },
        }

        Ok(())
    }

    fn set_return_type(&mut self, return_type: MethodReturnType) {
        let unit_type = MethodReturnType::unit_type();
        self.return_type = match return_type {
            MethodReturnType::Unit(ty) => MethodReturnType::Unit(ty),
            MethodReturnType::Regular(ty) => {
                if ty == unit_type {
                    MethodReturnType::Unit(ty)
                } else {
                    MethodReturnType::Regular(ty)
                }
            }
            MethodReturnType::ResultUnit(ty) => MethodReturnType::ResultUnit(ty),
            MethodReturnType::Result(ty) => {
                if ty == unit_type {
                    MethodReturnType::ResultUnit(ty)
                } else {
                    MethodReturnType::Result(ty)
                }
            }
        };
    }

    pub(super) fn generate_guest_ffi(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        let self_type = &self.self_type;
        if matches!(self.method_type, MethodType::Init) {
            let self_return_type = self.return_type.return_type() == self_type;
            let self_last_output_type = self.outputs.last().is_some_and(|output| output.has_self);

            if !(self_return_type || self_last_output_type) {
                return Err(Error::new(
                    fn_sig.span(),
                    "`#[init]` must have `Self` as either return type or last `#[output]` \
                    argument",
                ));
            }
        }

        let original_method_name = &fn_sig.ident;

        let guest_fn = self.generate_guest_fn(fn_sig, trait_name)?;
        let external_args_struct = self.generate_external_args_struct(fn_sig, trait_name)?;
        let metadata = self.generate_metadata(fn_sig, trait_name)?;

        Ok(quote! {
            pub mod #original_method_name {
                use super::*;

                #guest_fn
                #external_args_struct
                #metadata
            }
        })
    }

    pub(super) fn generate_guest_trait_ffi(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        let original_method_name = &fn_sig.ident;

        let external_args_struct = self.generate_external_args_struct(fn_sig, trait_name)?;
        let metadata = self.generate_metadata(fn_sig, trait_name)?;

        Ok(quote! {
            pub mod #original_method_name {
                use super::*;

                #external_args_struct
                #metadata
            }
        })
    }

    pub(super) fn generate_guest_fn(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        let self_type = &self.self_type;

        // `internal_args_pointers` will generate pointers in `InternalArgs` fields
        let mut internal_args_pointers = Vec::new();
        // `preparation` will generate code that is used before calling original function
        let mut preparation = Vec::new();
        // `original_fn_args` will generate arguments for calling original method implementation
        let mut original_fn_args = Vec::new();

        // Optional state argument with pointer and size (+ capacity if mutable)
        if let Some(mutability) = self.state {
            internal_args_pointers.push(quote! {
                pub self_ptr: ::core::ptr::NonNull<
                    <#self_type as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
            });

            if mutability.is_some() {
                internal_args_pointers.push(quote! {
                    /// Size of the contents `self_ptr` points to
                    pub self_size: *mut ::core::primitive::u32,
                    /// Capacity of the allocated memory following `self_ptr` points to
                    pub self_capacity: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                original_fn_args.push(quote! {&mut *{
                    // Ensure state type implements `IoType`, which is required for crossing
                    // host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoType,
                        {}
                        assert_impl_io_type::<#self_type>();
                    };

                    <#self_type as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                        &mut args.self_ptr,
                        &mut args.self_size,
                        args.self_capacity.read(),
                    )
                }});
            } else {
                internal_args_pointers.push(quote! {
                    /// Size of the contents `self_ptr` points to
                    pub self_size: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                original_fn_args.push(quote! {&*{
                    // Ensure state type implements `IoType`, which is required for crossing
                    // host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoType,
                        {}
                        assert_impl_io_type::<#self_type>();
                    };

                    <#self_type as ::ab_contracts_macros::__private::IoType>::from_ptr(
                        &args.self_ptr,
                        args.self_size.as_ref(),
                        // Size matches capacity for immutable inputs
                        args.self_size.read(),
                    )
                }});
            }
        }

        // Optional environment argument with just a pointer
        if let Some(env) = &self.env {
            let ptr_field = format_ident!("{}_ptr", env.arg_name);
            let assert_msg = format!("`{ptr_field}` pointer is misaligned");
            let mutability = env.mutability;

            internal_args_pointers.push(quote! {
                // Use `Env` to check if method argument had the correct type at compile time
                pub #ptr_field: ::core::ptr::NonNull<::ab_contracts_macros::__private::Env<'internal_args>>,
            });

            if mutability.is_some() {
                original_fn_args.push(quote! {{
                    debug_assert!(args.#ptr_field.is_aligned(), #assert_msg);
                    args.#ptr_field.as_mut()
                }});
            } else {
                original_fn_args.push(quote! {{
                    debug_assert!(args.#ptr_field.is_aligned(), #assert_msg);
                    args.#ptr_field.as_ref()
                }});
            }
        }

        // Optional tmp argument with pointer and size (+ capacity if mutable)
        //
        // Also asserting that type is safe for memory copying.
        if let Some(tmp) = &self.tmp {
            let type_name = &tmp.type_name;
            let mutability = tmp.mutability;
            let ptr_field = format_ident!("{}_ptr", tmp.arg_name);
            let size_field = format_ident!("{}_size", tmp.arg_name);
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", tmp.arg_name);
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <
                        // Make sure `#[tmp]` type matches expected type
                        <#self_type as ::ab_contracts_macros::__private::Contract>::Tmp as ::ab_contracts_macros::__private::IoType
                    >::PointerType,
                >,
            });

            if mutability.is_some() {
                internal_args_pointers.push(quote! {
                    #[doc = #size_doc]
                    pub #size_field: *mut ::core::primitive::u32,
                    #[doc = #capacity_doc]
                    pub #capacity_field: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                original_fn_args.push(quote! {&mut *{
                    // Ensure tmp type implements `IoTypeOptional`, which is required for handling
                    // of tmp that might be removed or not present and implies implementation of
                    // `IoType`, which is required for crossing host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type_optional<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoTypeOptional,
                        {}
                        assert_impl_io_type_optional::<#type_name>();
                    };

                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                        &mut args.#ptr_field,
                        &mut args.#size_field,
                        args.#capacity_field.read(),
                    )
                }});
            } else {
                internal_args_pointers.push(quote! {
                    #[doc = #size_doc]
                    pub #size_field: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                original_fn_args.push(quote! {&*{
                    // Ensure tmp type implements `IoTypeOptional`, which is required for handling
                    // of tmp that might be removed or not present and implies implementation of
                    // `IoType`, which is required for crossing host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type_optional<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoTypeOptional,
                        {}
                        assert_impl_io_type_optional::<#type_name>();
                    };

                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_ptr(
                        &args.#ptr_field,
                        args.#size_field.as_ref(),
                        // Size matches capacity for immutable inputs
                        args.#size_field.read(),
                    )
                }});
            }
        }

        // Slot arguments with:
        // * in case address is used: pointer to address, pointer to slot and size (+ capacity if
        //   mutable)
        // * in case address is not used: pointer to slot and size (+ capacity if mutable)
        //
        // Also asserting that type is safe for memory copying.
        for slot in &self.slots {
            let type_name = &slot.type_name;
            let mutability = slot.mutability;
            let address_ptr_field = format_ident!("{}_address_ptr", slot.arg_name);
            let ptr_field = format_ident!("{}_ptr", slot.arg_name);
            let size_field = format_ident!("{}_size", slot.arg_name);
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", slot.arg_name);
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            internal_args_pointers.push(quote! {
                // Use `Address` to check if method argument had the correct type at compile time
                pub #address_ptr_field: ::core::ptr::NonNull<::ab_contracts_macros::__private::Address>,
                pub #ptr_field: ::core::ptr::NonNull<
                    <
                        // Make sure `#[slot]` type matches expected type
                        <#self_type as ::ab_contracts_macros::__private::Contract>::Slot as ::ab_contracts_macros::__private::IoType
                    >::PointerType,
                >,
            });

            let arg_extraction = if mutability.is_some() {
                internal_args_pointers.push(quote! {
                    #[doc = #size_doc]
                    pub #size_field: *mut ::core::primitive::u32,
                    #[doc = #capacity_doc]
                    pub #capacity_field: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                quote! {&mut *{
                    // Ensure slot type implements `IoTypeOptional`, which is required for handling
                    // of slot that might be removed or not present and implies implementation of
                    // `IoType`, which is required for crossing host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type_optional<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoTypeOptional,
                        {}
                        assert_impl_io_type_optional::<#type_name>();
                    };

                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                        &mut args.#ptr_field,
                        &mut args.#size_field,
                        args.#capacity_field.read(),
                    )
                }}
            } else {
                internal_args_pointers.push(quote! {
                    #[doc = #size_doc]
                    pub #size_field: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                quote! {&*{
                    // Ensure slot type implements `IoTypeOptional`, which is required for handling
                    // of slot that might be removed or not present and implies implementation of
                    // `IoType`, which is required for crossing host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type_optional<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoTypeOptional,
                        {}
                        assert_impl_io_type_optional::<#type_name>();
                    };

                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_ptr(
                        &args.#ptr_field,
                        args.#size_field.as_ref(),
                        // Size matches capacity for immutable inputs
                        args.#size_field.read(),
                    )
                }}
            };

            if slot.with_address_arg {
                original_fn_args.push(quote! {
                    (
                        &<::ab_contracts_macros::__private::Address as ::ab_contracts_macros::__private::IoType>::from_ptr(
                            &args.#address_ptr_field,
                            &<::ab_contracts_macros::__private::Address as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                            <::ab_contracts_macros::__private::Address as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                        ),
                        #arg_extraction,
                    )
                });
            } else {
                original_fn_args.push(arg_extraction);
            }
        }

        // Inputs with a pointer and size.
        // Also asserting that type is safe for memory copying.
        for input in &self.inputs {
            let type_name = &input.type_name;
            let arg_name = &input.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: ::core::ptr::NonNull<::core::primitive::u32>,
            });

            original_fn_args.push(quote! {&*{
                // Ensure input type implements `IoType`, which is required for crossing host/guest
                // boundary
                const _: () = {
                    const fn assert_impl_io_type<T>()
                    where
                        T: ::ab_contracts_macros::__private::IoType,
                    {}
                    assert_impl_io_type::<#type_name>();
                };

                <#type_name as ::ab_contracts_macros::__private::IoType>::from_ptr(
                    &args.#ptr_field,
                    args.#size_field.as_ref(),
                    // Size matches capacity for immutable inputs
                    args.#size_field.read(),
                )
            }});
        }

        // Outputs with a pointer, size and capacity.
        // Also asserting that type is safe for memory copying.
        for output in &self.outputs {
            let type_name = &output.type_name;
            let arg_name = &output.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{arg_name}_capacity");
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: *mut ::core::primitive::u32,
                #[doc = #capacity_doc]
                pub #capacity_field: ::core::ptr::NonNull<::core::primitive::u32>,
            });

            original_fn_args.push(quote! {&mut *{
                // Ensure output type implements `IoTypeOptional`, which is required for handling of
                // the initially uninitialized type and implies implementation of `IoType`, which is
                // required for crossing host/guest boundary
                const _: () = {
                    const fn assert_impl_io_type_optional<T>()
                    where
                        T: ::ab_contracts_macros::__private::IoTypeOptional,
                    {}
                    assert_impl_io_type_optional::<#type_name>();
                };

                <#type_name as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                    &mut args.#ptr_field,
                    &mut args.#size_field,
                    args.#capacity_field.read(),
                )
            }});
        }

        let original_method_name = &fn_sig.ident;
        let ffi_fn_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;
        let return_type = self.return_type.return_type();

        let internal_args_struct = {
            // Result can be used through return type or argument, for argument no special handling
            // of the return type is needed. Similarly, it is skipped for a unit return type.
            if !self.return_type.unit_return_type() {
                internal_args_pointers.push(quote! {
                    pub ok_result_ptr: ::core::ptr::NonNull<#return_type>,
                    /// The size of the contents `ok_result_ptr` points to
                    pub ok_result_size: *mut ::core::primitive::u32,
                    /// Capacity of the allocated memory `ok_result_ptr` points to
                    pub ok_result_capacity: ::core::ptr::NonNull<::core::primitive::u32>,
                });

                // Ensure return type implements not only `IoType`, which is required for crossing
                // host/guest boundary, but also `TrivialType` and result handling is trivial.
                // `#[output]` must be used for a variable size result.
                preparation.push(quote! {
                    debug_assert!(
                        args.ok_result_ptr.is_aligned(),
                        "`ok_result_ptr` pointer is misaligned"
                    );
                    if !args.ok_result_size.is_null() {
                        debug_assert_eq!(
                            args.ok_result_size.read(),
                            0,
                            "`ok_result_size` must be zero initially",
                        );
                    }
                    debug_assert!(
                        args.ok_result_capacity.read() >=
                            <#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                        "`ok_result_capacity` specified is invalid",
                    );
                });
            }
            let args_struct_doc = format!(
                "Data structure containing expected input to [`{ffi_fn_name}()`], it is used \
                internally by the contract, there should be no need to construct it explicitly \
                except maybe in contract's own tests"
            );
            quote_spanned! {fn_sig.span() =>
                #[doc = #args_struct_doc]
                #[repr(C)]
                pub struct InternalArgs<'internal_args>
                {
                    #( #internal_args_pointers )*
                    _phantom: ::core::marker::PhantomData<&'internal_args ()>,
                }
            }
        };

        let result_var_name = format_ident!("result");
        let guest_fn = {
            // Depending on whether `T` or `Result<T, ContractError>` is used as return type,
            // generate different code for result handling
            let result_handling = match &self.return_type {
                MethodReturnType::Unit(_) => {
                    quote! {
                        // Return exit code
                        ::ab_contracts_macros::__private::ExitCode::ok()
                    }
                }
                MethodReturnType::Regular(_) => {
                    quote! {
                        // Size ight be a null pointer for trivial types
                        if !args.ok_result_size.is_null() {
                            args.ok_result_size.write(
                                <#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                            );
                        }
                        args.ok_result_ptr.write(#result_var_name);
                        // Return exit code
                        ::ab_contracts_macros::__private::ExitCode::ok()
                    }
                }
                MethodReturnType::ResultUnit(_) => {
                    quote! {
                        // Return exit code
                        match #result_var_name {
                            Ok(()) => ::ab_contracts_macros::__private::ExitCode::ok(),
                            Err(error) => error.exit_code(),
                        }
                    }
                }
                MethodReturnType::Result(_) => {
                    quote! {
                        // Write a result into `InternalArgs` if there is any, return exit code
                        match #result_var_name {
                            Ok(result) => {
                                // Size ight be a null pointer for trivial types
                                if !args.ok_result_size.is_null() {
                                    args.ok_result_size.write(
                                        <#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                                    );
                                }
                                args.ok_result_ptr.write(result);
                                // Return exit code
                                ::ab_contracts_macros::__private::ExitCode::ok()
                            }
                            Err(error) => error.exit_code(),
                        }
                    }
                }
            };

            let full_struct_name = if let Some(trait_name) = trait_name {
                quote! { <#self_type as #trait_name> }
            } else {
                quote! { #self_type }
            };

            // Generate FFI function with original name (hiding original implementation), but
            // exported as shortcut name
            quote_spanned! {fn_sig.span() =>
                /// FFI interface into a method, called by the host.
                ///
                /// NOTE: Calling this function directly shouldn't be necessary except maybe in
                /// contract's own tests.
                ///
                /// # Safety
                ///
                /// Caller must ensure the provided pointer corresponds to expected ABI.
                #[cfg_attr(feature = "guest", unsafe(no_mangle))]
                #[allow(clippy::new_ret_no_self, reason = "Method was re-written for FFI purposes without `Self`")]
                #[allow(clippy::absurd_extreme_comparisons, reason = "Macro-generated code doesn't know the size upfront")]
                pub unsafe extern "C" fn #ffi_fn_name(
                    mut args: ::core::ptr::NonNull<InternalArgs<'_>>,
                ) -> ::ab_contracts_macros::__private::ExitCode {
                    // SAFETY: Must be upheld by the caller (executor)
                    unsafe {
                        debug_assert!(args.is_aligned(), "`args` pointer is misaligned");
                        let args = args.as_mut();

                        #( #preparation )*

                        // Call inner function via normal Rust API
                        #[allow(
                            unused_variables,
                            reason = "Sometimes result is `()`"
                        )]
                        #[allow(
                            clippy::let_unit_value,
                            reason = "Sometimes result is `()`"
                        )]
                        let #result_var_name = #full_struct_name::#original_method_name(
                            #( #original_fn_args, )*
                        );

                        #result_handling
                    }
                }
            }
        };

        let fn_pointer_static = {
            let adapter_ffi_fn_name = format_ident!("{ffi_fn_name}_adapter");
            let args_struct_name =
                derive_external_args_struct_name(self_type, trait_name, original_method_name)?;

            quote! {
                #[doc(hidden)]
                pub mod fn_pointer {
                    use super::*;

                    unsafe extern "C" fn #adapter_ffi_fn_name(
                        ptr: ::core::ptr::NonNull<::core::ptr::NonNull<::core::ffi::c_void>>,
                    ) -> ::ab_contracts_macros::__private::ExitCode {
                        // SAFETY: Caller must ensure correct ABI of the void pointer, little can be
                        // done here
                        unsafe { #ffi_fn_name(ptr.cast::<InternalArgs>()) }
                    }

                    pub const METHOD_FN_POINTER: ::ab_contracts_macros::__private::NativeExecutorContactMethod =
                        ::ab_contracts_macros::__private::NativeExecutorContactMethod {
                            method_fingerprint: &<#args_struct_name as ::ab_contracts_macros::__private::ExternalArgs>::FINGERPRINT,
                            method_metadata: METADATA,
                            ffi_fn: #adapter_ffi_fn_name,
                        };
                }
            }
        };

        Ok(quote! {
            #internal_args_struct
            #guest_fn
            #fn_pointer_static
        })
    }

    fn generate_external_args_struct(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        let self_type = &self.self_type;
        let original_method_name = &fn_sig.ident;

        let args_struct_name =
            derive_external_args_struct_name(self_type, trait_name, original_method_name)?;
        // `external_args_pointers` will generate pointers in `ExternalArgs` fields
        let mut external_args_fields = Vec::new();
        // Arguments of `::new()` method
        let mut method_args = Vec::new();
        // Fields set on `Self` in `::new()` method
        let mut method_args_fields = Vec::new();

        // For slots in external args only address is needed
        for slot in &self.slots {
            let arg_name = &slot.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");

            external_args_fields.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<::ab_contracts_macros::__private::Address>,
            });

            method_args.push(quote! {
                #arg_name: &::ab_contracts_macros::__private::Address,
            });
            method_args_fields.push(quote! {
                // TODO: Use `NonNull::from_ref()` once stable
                #ptr_field: ::core::ptr::NonNull::from(#arg_name),
            });
        }

        // Inputs with a pointer and size
        for input in &self.inputs {
            let type_name = &input.type_name;
            let arg_name = &input.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");

            external_args_fields.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: ::core::ptr::NonNull<::core::primitive::u32>,
            });

            method_args.push(quote! {
                #arg_name: &#type_name,
            });
            method_args_fields.push(quote! {
                // SAFETY: This pointer is used as input to FFI call, and underlying data
                // will not be modified, also the pointer will not outlive the reference
                // from which it was created despite copying
                #ptr_field: unsafe {
                    *::ab_contracts_macros::__private::IoType::as_ptr(#arg_name)
                },
                // SAFETY: This pointer is used as input to FFI call, and underlying data
                // will not be modified, also the pointer will not outlive the reference
                // from which it was created despite copying
                #size_field: unsafe {
                    *::ab_contracts_macros::__private::IoType::size_ptr(#arg_name)
                },
            });
        }

        // Outputs with a pointer, size and capacity
        let mut outputs_iter = self.outputs.iter().peekable();
        while let Some(output) = outputs_iter.next() {
            let type_name = &output.type_name;
            let arg_name = &output.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{arg_name}_capacity");
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            // Initializer's return type will be `()` for caller of `#[init]`, state is stored by
            // the host and not returned to the caller, hence no explicit argument is needed
            if outputs_iter.is_empty()
                && self.return_type.unit_return_type()
                && matches!(self.method_type, MethodType::Init)
            {
                continue;
            }

            external_args_fields.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: *mut ::core::primitive::u32,
                #[doc = #capacity_doc]
                pub #capacity_field: ::core::ptr::NonNull<::core::primitive::u32>,
            });

            method_args.push(quote! {
                #arg_name: &mut #type_name,
            });
            method_args_fields.push(quote! {
                // SAFETY: This pointer is used as input to FFI call, and underlying data will only
                // be modified there, also the pointer will not outlive the reference from which it
                // was created despite copying
                #ptr_field: unsafe {
                    *::ab_contracts_macros::__private::IoType::as_mut_ptr(#arg_name)
                },
                // SAFETY: This pointer is used as input to FFI call, and underlying data will only
                // be modified there, also the pointer will not outlive the reference from which it
                // was created despite copying
                #size_field: unsafe {
                    *::ab_contracts_macros::__private::IoType::size_mut_ptr(#arg_name)
                },
                // SAFETY: This pointer is used as input to FFI call, and underlying data will not
                // be modified, also the pointer will not outlive the reference from which it was
                // created despite copying
                #capacity_field: unsafe {
                    *::ab_contracts_macros::__private::IoType::capacity_ptr(#arg_name)
                },
            });
        }

        let ffi_fn_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;

        // Initializer's return type will be `()` for caller of `#[init]` since the state is stored
        // by the host and not returned to the caller and explicit argument is not needed in
        // `ExternalArgs` struct. Similarly, it is skipped for a unit return type.
        if !(matches!(self.method_type, MethodType::Init) || self.return_type.unit_return_type()) {
            let return_type = &self.return_type.return_type();

            external_args_fields.push(quote! {
                pub ok_result_ptr: ::core::ptr::NonNull<#return_type>,
                /// Size of the contents `ok_result_ptr` points to
                pub ok_result_size: *mut ::core::primitive::u32,
                /// Capacity of the allocated memory `ok_result_ptr` points to
                pub ok_result_capacity: ::core::ptr::NonNull<::core::primitive::u32>,
            });

            method_args.push(quote! {
                ok_result: &mut ::core::mem::MaybeUninit<#return_type>,
                ok_result_size: &mut ::core::primitive::u32,
            });
            method_args_fields.push(quote! {
                // SAFETY: Pointer created from an allocated struct
                ok_result_ptr: unsafe {
                    ::core::ptr::NonNull::new_unchecked(ok_result.as_mut_ptr())
                },
                ok_result_size: ::core::ptr::from_mut(ok_result_size),
                // This is for `TrivialType` and will never be modified
                // TODO: Use `NonNull::from_ref()` once stable
                ok_result_capacity: ::core::ptr::NonNull::from(
                    &<#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                ),
            });
        }
        let args_struct_doc = format!(
            "Data structure containing expected input for external method invocation, eventually \
            calling `{ffi_fn_name}()` on the other side by the host.\n\n\
            This can be used with [`Env`](::ab_contracts_macros::__private::Env), though there are \
            helper methods on this provided by extension trait that allow not dealing with this \
            struct directly in simpler cases."
        );

        Ok(quote_spanned! {fn_sig.span() =>
            #[doc = #args_struct_doc]
            #[repr(C)]
            pub struct #args_struct_name {
                #( #external_args_fields )*
            }

            #[automatically_derived]
            unsafe impl ::ab_contracts_macros::__private::ExternalArgs for #args_struct_name {
                const FINGERPRINT: ::ab_contracts_macros::__private::MethodFingerprint =
                    ::ab_contracts_macros::__private::MethodFingerprint::new(METADATA)
                        .expect("Metadata is statically correct; qed");
                const METADATA: &[::core::primitive::u8] = METADATA;
            }

            impl #args_struct_name {
                /// Create a new instance
                #[allow(
                    clippy::new_without_default,
                    reason = "Do not want `Default` in auto-generated code"
                )]
                pub fn new(
                    #( #method_args )*
                ) -> Self {
                    Self {
                        #( #method_args_fields )*
                    }
                }
            }
        })
    }

    fn generate_metadata(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        let self_type = &self.self_type;
        // `method_metadata` will generate metadata about method arguments, each element in this
        // vector corresponds to one argument
        let mut method_metadata = Vec::new();

        if let Some(env) = &self.env {
            let env_metadata_type = if env.mutability.is_some() {
                "EnvRw"
            } else {
                "EnvRo"
            };

            let env_metadata_type = format_ident!("{env_metadata_type}");
            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#env_metadata_type as ::core::primitive::u8],
            });
        }

        if let Some(tmp) = &self.tmp {
            let tmp_metadata_type = if tmp.mutability.is_some() {
                "TmpRw"
            } else {
                "TmpRo"
            };

            let tmp_metadata_type = format_ident!("{tmp_metadata_type}");
            let arg_name_metadata = derive_ident_metadata(&tmp.arg_name)?;
            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#tmp_metadata_type as ::core::primitive::u8],
                #arg_name_metadata,
            });
        }

        for slot in &self.slots {
            let slot_metadata_type = if slot.mutability.is_some() {
                "SlotRw"
            } else {
                "SlotRo"
            };

            let slot_metadata_type = format_ident!("{slot_metadata_type}");
            let arg_name_metadata = derive_ident_metadata(&slot.arg_name)?;
            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#slot_metadata_type as ::core::primitive::u8],
                #arg_name_metadata,
            });
        }

        for input in &self.inputs {
            let io_metadata_type = format_ident!("Input");
            let arg_name_metadata = derive_ident_metadata(&input.arg_name)?;
            let type_name = &input.type_name;

            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#io_metadata_type as ::core::primitive::u8],
                #arg_name_metadata,
                <#type_name as ::ab_contracts_macros::__private::IoType>::METADATA,
            });
        }

        let mut outputs_iter = self.outputs.iter().peekable();
        while let Some(output) = outputs_iter.next() {
            let io_metadata_type = "Output";

            let io_metadata_type = format_ident!("{io_metadata_type}");
            let arg_name_metadata = derive_ident_metadata(&output.arg_name)?;
            // Skip type metadata for `#[init]`'s last output since it is known statically
            let with_type_metadata = if outputs_iter.is_empty()
                && self.return_type.unit_return_type()
                && matches!(self.method_type, MethodType::Init)
            {
                None
            } else {
                let type_name = &output.type_name;
                Some(quote! {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::METADATA,
                })
            };
            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#io_metadata_type as ::core::primitive::u8],
                #arg_name_metadata,
                #with_type_metadata
            });
        }

        // Skipped if return type is unit
        if !self.return_type.unit_return_type() {
            // There isn't an explicit name in case of the return type
            let arg_name_metadata = Literal::u8_unsuffixed(0);
            // Skip type metadata for `#[init]`'s result since it is known statically
            let with_type_metadata = if matches!(self.method_type, MethodType::Init) {
                None
            } else {
                let return_type = self.return_type.return_type();
                Some(quote! {
                    <#return_type as ::ab_contracts_macros::__private::IoType>::METADATA,
                })
            };
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_macros::__private::ContractMetadataKind::Output as ::core::primitive::u8,
                    #arg_name_metadata,
                ],
                #with_type_metadata
            });
        }

        let method_type = match self.method_type {
            MethodType::Init => "Init",
            MethodType::Update => {
                if let Some(mutable) = &self.state {
                    if mutable.is_some() {
                        "UpdateStatefulRw"
                    } else {
                        "UpdateStatefulRo"
                    }
                } else {
                    "UpdateStateless"
                }
            }
            MethodType::View => {
                if let Some(mutable) = &self.state {
                    if mutable.is_some() {
                        return Err(Error::new(
                            fn_sig.span(),
                            "Stateful view methods are not supported",
                        ));
                    }

                    "ViewStateful"
                } else {
                    "ViewStateless"
                }
            }
        };

        let method_type = format_ident!("{method_type}");
        let number_of_arguments = u8::try_from(method_metadata.len()).map_err(|_error| {
            Error::new(
                fn_sig.span(),
                format!("Number of arguments must not be more than {}", u8::MAX),
            )
        })?;
        let number_of_arguments = Literal::u8_unsuffixed(number_of_arguments);

        let original_method_name = &fn_sig.ident;
        let ffi_fn_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;
        let method_name_metadata = derive_ident_metadata(&ffi_fn_name)?;
        Ok(quote_spanned! {fn_sig.span() =>
            const fn metadata()
                -> ([::core::primitive::u8; ::ab_contracts_macros::__private::MAX_METADATA_CAPACITY], usize)
            {
                ::ab_contracts_macros::__private::concat_metadata_sources(&[
                    &[::ab_contracts_macros::__private::ContractMetadataKind::#method_type as ::core::primitive::u8],
                    #method_name_metadata,
                    &[#number_of_arguments],
                    #( #method_metadata )*
                ])
            }

            /// Method metadata, see [`ContractMetadataKind`] for encoding details
            ///
            /// [`ContractMetadataKind`]: ::ab_contracts_macros::__private::ContractMetadataKind
            // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically
            pub const METADATA: &[::core::primitive::u8] =
                metadata()
                    .0
                    .split_at(metadata().1)
                    .0;
        })
    }

    pub(super) fn generate_trait_ext_components(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<ExtTraitComponents, Error> {
        let self_type = &self.self_type;

        let mut preparation = Vec::new();
        let mut method_args = Vec::new();
        let mut external_args_args = Vec::new();
        let mut result_processing = Vec::new();

        // Address of the contract
        method_args.push(quote! {
            contract: ::ab_contracts_macros::__private::Address,
        });

        // For each slot argument generate an address argument
        for slot in &self.slots {
            let arg_name = &slot.arg_name;

            method_args.push(quote! {
                #arg_name: &::ab_contracts_macros::__private::Address,
            });
            external_args_args.push(quote! { #arg_name });
        }

        // For each input argument, generate a corresponding read-only argument
        for input in &self.inputs {
            let type_name = &input.type_name;
            let arg_name = &input.arg_name;

            method_args.push(quote! {
                #arg_name: &#type_name,
            });
            external_args_args.push(quote! { #arg_name });
        }

        // For each output argument, generate a corresponding write-only argument
        let mut outputs_iter = self.outputs.iter().peekable();
        while let Some(output) = outputs_iter.next() {
            let type_name = &output.type_name;
            let arg_name = &output.arg_name;

            // Initializer's return type will be `()` for caller of `#[init]`, state is stored by
            // the host and not returned to the caller
            if outputs_iter.is_empty()
                && self.return_type.unit_return_type()
                && matches!(self.method_type, MethodType::Init)
            {
                continue;
            }

            method_args.push(quote! {
                #arg_name: &mut #type_name,
            });
            external_args_args.push(quote! { #arg_name });
        }

        let original_method_name = &fn_sig.ident;
        let ext_method_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;
        // Non-`#[view]` methods can only be called on `&mut Env`
        let env_self = if matches!(self.method_type, MethodType::View) {
            quote! { &self }
        } else {
            quote! { &mut self }
        };
        // `#[view]` methods do not require explicit method context
        let method_context_arg = (!matches!(self.method_type, MethodType::View)).then(|| {
            quote! {
                method_context: ::ab_contracts_macros::__private::MethodContext,
            }
        });
        // Initializer's return type will be `()` for caller of `#[init]` since the state is stored
        // by the host and not returned to the caller. Similarly, it is skipped for a unit return
        // type.
        let method_signature = if matches!(self.method_type, MethodType::Init)
            || self.return_type.unit_return_type()
        {
            quote! {
                fn #ext_method_name(
                    #env_self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<(), ::ab_contracts_macros::__private::ContractError>
            }
        } else {
            let return_type = self.return_type.return_type();

            preparation.push(quote! {
                let mut ok_result = ::core::mem::MaybeUninit::uninit();
                // While this will not change for `TrivialType`, the pointer will be written to and
                // as such, the value needs to be given
                let mut ok_result_size =
                    <#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE;
            });
            external_args_args.push(quote! {
                &mut ok_result,
                &mut ok_result_size
            });
            result_processing.push(quote! {
                // This is fine for `TrivialType` types
                ok_result.assume_init()
            });

            quote! {
                fn #ext_method_name(
                    #env_self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<
                    #return_type,
                    ::ab_contracts_macros::__private::ContractError,
                >
            }
        };

        let definitions = quote! {
            #method_signature;
        };

        let args_struct_name =
            derive_external_args_struct_name(self_type, trait_name, original_method_name)?;
        // `#[view]` methods do not require explicit method context
        let method_context_value = if matches!(self.method_type, MethodType::View) {
            quote! { ::ab_contracts_macros::__private::MethodContext::Reset }
        } else {
            quote! { method_context }
        };
        let impls = quote! {
            #[inline]
            #method_signature {
                #( #preparation )*

                let mut args = #original_method_name::#args_struct_name::new(
                    #( #external_args_args, )*
                );

                self.call(contract, &mut args, #method_context_value)?;

                // SAFETY: The non-error result above indicates successful storing of the result
                #[allow(
                    unused_unsafe,
                    reason = "Sometimes there is no result to process and block is empty"
                )]
                #[allow(
                    clippy::let_unit_value,
                    reason = "Sometimes there is no result to process and block is empty"
                )]
                let result = unsafe {
                    #( #result_processing )*
                };

                Ok(result)
            }
        };

        Ok(ExtTraitComponents { definitions, impls })
    }
}

fn extract_arg_name(mut pat: &Pat) -> Option<Ident> {
    loop {
        match pat {
            Pat::Ident(pat_ident) => {
                return Some(pat_ident.ident.clone());
            }
            Pat::Reference(pat_reference) => {
                pat = &pat_reference.pat;
            }
            _ => {
                return None;
            }
        }
    }
}

fn derive_ffi_fn_name(
    type_name: &Type,
    trait_name: Option<&Ident>,
    method_name: &Ident,
) -> Result<Ident, Error> {
    let type_name = extract_ident_from_type(type_name).ok_or_else(|| {
        Error::new(
            type_name.span(),
            "`#[contract]` must be applied to a simple struct without generics",
        )
    })?;
    let ffi_fn_prefix =
        RenameRule::SnakeCase.apply_to_variant(trait_name.unwrap_or(type_name).to_string());

    Ok(format_ident!("{ffi_fn_prefix}_{method_name}"))
}

fn derive_external_args_struct_name(
    type_name: &Type,
    trait_name: Option<&Ident>,
    method_name: &Ident,
) -> Result<Ident, Error> {
    let type_name = extract_ident_from_type(type_name).ok_or_else(|| {
        Error::new(
            type_name.span(),
            "`#[contract]` must be applied to a simple struct without generics",
        )
    })?;
    Ok(format_ident!(
        "{}{}Args",
        trait_name.unwrap_or(type_name),
        RenameRule::PascalCase.apply_to_field(method_name.to_string())
    ))
}
