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
    fn attr_str(&self) -> &'static str {
        match self {
            MethodType::Init => "init",
            MethodType::Update => "update",
            MethodType::View => "view",
        }
    }
}

#[derive(Clone)]
struct Tmp {
    type_name: Type,
    arg_name: Ident,
    mutability: Option<Token![mut]>,
}

#[derive(Clone)]
struct Slot {
    with_address_arg: Option<Ident>,
    type_name: Type,
    arg_name: Ident,
    mutability: Option<Token![mut]>,
}

#[derive(Clone)]
enum IoArg {
    Input { type_name: Type, arg_name: Ident },
    Output { type_name: Type, arg_name: Ident },
    Result { type_name: Type, arg_name: Ident },
}

impl IoArg {
    fn type_name(&self) -> &Type {
        let (Self::Input { type_name, .. }
        | Self::Output { type_name, .. }
        | Self::Result { type_name, .. }) = self;
        type_name
    }

    fn arg_name(&self) -> &Ident {
        let (Self::Input { arg_name, .. }
        | Self::Output { arg_name, .. }
        | Self::Result { arg_name, .. }) = self;
        arg_name
    }
}

enum MethodResultType {
    /// Function doesn't have any return type defined
    Unit(Type),
    /// Returns a type without [`Result`]
    Regular(Type),
    /// Returns [`Result`], but [`Ok`] variant is `()`
    ResultUnit(Type),
    /// Returns [`Result`], but [`Ok`] variant is not `()`
    Result(Type),
}

impl MethodResultType {
    fn unit() -> Self {
        Self::Unit(Self::unit_type())
    }

    fn unit_type() -> Type {
        Type::Tuple(TypeTuple {
            paren_token: Default::default(),
            elems: Default::default(),
        })
    }

    fn unit_result_type(&self) -> bool {
        match self {
            Self::Unit(_) | Self::ResultUnit(_) => true,
            Self::Regular(_) | Self::Result(_) => false,
        }
    }

    fn result_type(&self) -> &Type {
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
    env: Option<Option<Token![mut]>>,
    tmp: Option<Tmp>,
    slots: Vec<Slot>,
    io: Vec<IoArg>,
    result_type: MethodResultType,
}

impl MethodDetails {
    pub(super) fn new(method_type: MethodType, self_type: Type) -> Self {
        Self {
            method_type,
            self_type,
            state: None,
            env: None,
            tmp: None,
            slots: vec![],
            io: vec![],
            result_type: MethodResultType::Unit(MethodResultType::unit_type()),
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

        Some(tmp_type.unwrap_or_else(MethodResultType::unit_type))
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

        Some(slot_type.unwrap_or_else(MethodResultType::unit_type))
    }

    pub(super) fn process_output(&mut self, output: &ReturnType) -> Result<(), Error> {
        // Check if return type is `T` or `Result<T, ContractError>`
        let error_message = format!(
            "`#[{}]` must return `()` or `T` or `Result<T, ContractError>",
            self.method_type.attr_str()
        );
        match output {
            ReturnType::Default => {
                self.set_result_type(MethodResultType::unit());
            }
            ReturnType::Type(_r_arrow, return_type) => match return_type.as_ref() {
                Type::Array(_type_array) => {
                    self.set_result_type(MethodResultType::Regular(return_type.as_ref().clone()));
                }
                Type::Path(type_path) => {
                    // Check for `-> Result<T, ContractError>`
                    if let Some(last_path_segment) = type_path.path.segments.last() {
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
                                    .map(|s| s.ident == "ContractError")
                                    .unwrap_or_default()
                            {
                                if let Type::Path(ok_path) = ok_type
                                    && ok_path
                                        .path
                                        .segments
                                        .first()
                                        .map(|s| s.ident == "Self")
                                        .unwrap_or_default()
                                {
                                    // Swap `Self` for an actual struct name
                                    self.set_result_type(MethodResultType::Regular(
                                        self.self_type.clone(),
                                    ));
                                } else {
                                    self.set_result_type(MethodResultType::Result(ok_type.clone()));
                                }
                            } else {
                                return Err(Error::new(return_type.span(), error_message));
                            }
                        } else if last_path_segment.ident == "Self" {
                            // Swap `Self` for an actual struct name
                            self.set_result_type(MethodResultType::Regular(self.self_type.clone()));
                        } else {
                            self.set_result_type(MethodResultType::Regular(
                                return_type.as_ref().clone(),
                            ));
                        }
                    } else {
                        self.set_result_type(MethodResultType::Regular(
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

    fn set_result_type(&mut self, result_type: MethodResultType) {
        let unit_type = MethodResultType::unit_type();
        self.result_type = match result_type {
            MethodResultType::Unit(ty) => MethodResultType::Unit(ty),
            MethodResultType::Regular(ty) => {
                if ty == unit_type {
                    MethodResultType::Unit(ty)
                } else {
                    MethodResultType::Regular(ty)
                }
            }
            MethodResultType::ResultUnit(ty) => MethodResultType::ResultUnit(ty),
            MethodResultType::Result(ty) => {
                if ty == unit_type {
                    MethodResultType::ResultUnit(ty)
                } else {
                    MethodResultType::Result(ty)
                }
            }
        };
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
        if self.env.is_some() || self.tmp.is_some() || !self.io.is_empty() {
            return Err(Error::new(
                input_span,
                "`#[env]` must be the first non-Self argument and only appear once",
            ));
        }

        if let Type::Reference(type_reference) = &*pat_type.ty
            && let Type::Path(_type_path) = &*type_reference.elem
        {
            if type_reference.mutability.is_some() && !allow_mut {
                return Err(Error::new(
                    input_span,
                    "`#[env]` is not allowed to mutate data here",
                ));
            }

            self.env.replace(type_reference.mutability);
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
        if self.tmp.is_some() || !self.io.is_empty() {
            return Err(Error::new(
                input_span,
                "`#[tmp]` must appear only once before any inputs or outputs",
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
        if !self.io.is_empty() {
            return Err(Error::new(
                input_span,
                "`#[slot]` must appear before any inputs or outputs",
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
                    with_address_arg: None,
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
                {
                    if outer_slot_type.mutability.is_some() && !allow_mut {
                        return Err(Error::new(
                            input_span,
                            "`#[slot]` is not allowed to mutate data here",
                        ));
                    }

                    let (address_arg, arg_name) = if let Pat::Tuple(pat_tuple) = &*pat_type.pat
                        && pat_tuple.elems.len() == 2
                        && let Some(address_arg) = extract_arg_name(&pat_tuple.elems[0])
                        && let Some(slot_arg) = extract_arg_name(&pat_tuple.elems[1])
                    {
                        (address_arg, slot_arg)
                    } else {
                        return Err(Error::new(
                            pat_type.span(),
                            "`#[slot]` with address must be a tuple of arguments, each of \
                            which is either a simple variable or a reference",
                        ));
                    };

                    self.slots.push(Slot {
                        with_address_arg: Some(address_arg),
                        type_name: outer_slot_type.elem.as_ref().clone(),
                        arg_name,
                        mutability: outer_slot_type.mutability,
                    });
                    return Ok(());
                }
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
        if self
            .io
            .iter()
            .any(|io_arg| !matches!(io_arg, IoArg::Input { .. }))
        {
            return Err(Error::new(
                input_span,
                "`#[input]` must appear before any outputs or result",
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

            self.io.push(IoArg::Input {
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
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        if self
            .io
            .iter()
            .any(|io_arg| matches!(io_arg, IoArg::Result { .. }))
        {
            return Err(Error::new(
                input_span,
                "`#[output]` must appear before result",
            ));
        }

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

            self.io.push(IoArg::Output {
                type_name: type_reference.elem.as_ref().clone(),
                arg_name: pat_ident.ident.clone(),
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

    pub(super) fn process_result_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        if !self.result_type.unit_result_type() {
            return Err(Error::new(
                input_span,
                "`#[result]` must only be used with methods that either return `()` or \
                `Result<(), ContractError>`",
            ));
        }

        // Ensure input looks like `&mut Type`
        if let Type::Reference(type_reference) = &*pat_type.ty
            && type_reference.mutability.is_some()
        {
            let Pat::Ident(pat_ident) = &*pat_type.pat else {
                return Err(Error::new(
                    pat_type.span(),
                    "`#[result]` argument name must be an exclusive reference",
                ));
            };

            let mut type_name = type_reference.elem.as_ref().clone();

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
            }

            self.io.push(IoArg::Result {
                type_name,
                arg_name: pat_ident.ident.clone(),
            });
            Ok(())
        } else {
            Err(Error::new(
                pat_type.span(),
                "`#[result]` must be an exclusive reference to a type implementing \
                `IoTypeOptional`, likely `MaybeData` container",
            ))
        }
    }

    pub(super) fn generate_guest_ffi(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        let self_type = &self.self_type;
        if matches!(self.method_type, MethodType::Init) {
            let self_return_type = self.result_type.result_type() == self_type;
            let self_result_type = self
                .io
                .last()
                .map(|io_arg| {
                    // Match things like `MaybeData<#self_type>`
                    if let IoArg::Result { type_name, .. } = io_arg
                        && let Type::Path(type_path) = type_name
                        && let Some(path_segment) = type_path.path.segments.last()
                        && let PathArguments::AngleBracketed(generic_arguments) =
                            &path_segment.arguments
                        && let Some(GenericArgument::Type(first_generic_argument)) =
                            generic_arguments.args.first()
                    {
                        first_generic_argument == self_type
                    } else {
                        false
                    }
                })
                .unwrap_or_default();

            if !(self_return_type || self_result_type) || (self_return_type && self_result_type) {
                return Err(Error::new(
                    fn_sig.span(),
                    "`#[init]` must have result type of `Self` as either return type or explicit \
                    `#[result]` argument, but not both",
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
        // `internal_args_sizes` will generate sizes in `InternalArgs` fields
        let mut internal_args_sizes = Vec::new();
        // `internal_args_capacities` will generate capacities in `InternalArgs` fields
        let mut internal_args_capacities = Vec::new();
        // `preparation` will generate code that is used before calling original function
        let mut preparation = Vec::new();
        // `original_fn_args` will generate arguments for calling original method implementation
        let mut original_fn_args = Vec::new();

        // Optional state argument with pointer and size (+ capacity if mutable)
        if let Some(mutability) = self.state {
            internal_args_pointers.push(quote! {
                pub state_ptr: ::core::ptr::NonNull<
                    <#self_type as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
            });

            internal_args_sizes.push(quote! {
                /// Size of the contents following `state_ptr` points to
                pub state_size: u32,
            });

            if mutability.is_some() {
                internal_args_capacities.push(quote! {
                    /// Capacity of the allocated memory following `state_ptr` points to
                    pub state_capacity: u32,
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
                        &mut args.state_ptr,
                        &mut args.state_size,
                        args.state_capacity,
                    )
                }});
            } else {
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
                        &args.state_ptr,
                        &args.state_size,
                        // Size matches capacity for immutable inputs
                        args.state_size,
                    )
                }});
            }
        }

        // Optional environment argument with just a pointer
        if let Some(mutability) = self.env {
            internal_args_pointers.push(quote! {
                // Use `Env` to check if method argument had correct type at compile time
                pub env_ptr: ::core::ptr::NonNull<::ab_contracts_macros::__private::Env>,
            });

            if mutability.is_some() {
                original_fn_args.push(quote! {{
                    debug_assert!(args.env_ptr.is_aligned(), "`env_ptr` pointer is misaligned");
                    args.env_ptr.as_mut()
                }});
            } else {
                original_fn_args.push(quote! {{
                    debug_assert!(args.env_ptr.is_aligned(), "`env_ptr` pointer is misaligned");
                    args.env_ptr.as_ref()
                }});
            }
        }

        // Optional tmp argument with pointer to slot and size (+ capacity if mutable)
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
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
            });
            internal_args_sizes.push(quote! {
                #[doc = #size_doc]
                pub #size_field: u32,
            });

            if mutability.is_some() {
                internal_args_capacities.push(quote! {
                    #[doc = #capacity_doc]
                    pub #capacity_field: u32,
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
                        args.#capacity_field,
                    )
                }});
            } else {
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
                        &args.#size_field,
                        // Size matches capacity for immutable inputs
                        args.#size_field,
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
            if let Some(address_arg) = &slot.with_address_arg {
                let address_ptr = format_ident!("{address_arg}_ptr");
                internal_args_pointers.push(quote! {
                    // Use `Address` to check if method argument had correct type at compile time
                    pub #address_ptr: ::core::ptr::NonNull<::ab_contracts_macros::__private::Address>,
                });
            }

            let type_name = &slot.type_name;
            let mutability = slot.mutability;
            let ptr_field = format_ident!("{}_ptr", slot.arg_name);
            let size_field = format_ident!("{}_size", slot.arg_name);
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", slot.arg_name);
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
            });
            internal_args_sizes.push(quote! {
                #[doc = #size_doc]
                pub #size_field: u32,
            });

            let arg_extraction = if mutability.is_some() {
                internal_args_capacities.push(quote! {
                    #[doc = #capacity_doc]
                    pub #capacity_field: u32,
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
                        args.#capacity_field,
                    )
                }}
            } else {
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
                        &args.#size_field,
                        // Size matches capacity for immutable inputs
                        args.#size_field,
                    )
                }}
            };

            if let Some(address_arg) = &slot.with_address_arg {
                let address_ptr = format_ident!("{address_arg}_ptr");
                original_fn_args.push(quote! {
                    (
                        &<::ab_contracts_macros::__private::Address as ::ab_contracts_macros::__private::IoType>::from_ptr(
                            &args.#address_ptr,
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

        // Inputs and outputs with pointer and size (+ capacity if mutable).
        // Also asserting that type is safe for memory copying.
        for io_arg in &self.io {
            let type_name = io_arg.type_name();
            let ptr_field = format_ident!("{}_ptr", io_arg.arg_name());
            let size_field = format_ident!("{}_size", io_arg.arg_name());
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", io_arg.arg_name());
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            match io_arg {
                IoArg::Input { .. } => {
                    internal_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                        >,
                    });
                    internal_args_sizes.push(quote! {
                        #[doc = #size_doc]
                        pub #size_field: u32,
                    });

                    original_fn_args.push(quote! {&*{
                        // Ensure input type implements `IoType`, which is required for crossing
                        // host/guest boundary
                        const _: () = {
                            const fn assert_impl_io_type<T>()
                            where
                                T: ::ab_contracts_macros::__private::IoType,
                            {}
                            assert_impl_io_type::<#type_name>();
                        };

                        <#type_name as ::ab_contracts_macros::__private::IoType>::from_ptr(
                            &args.#ptr_field,
                            &args.#size_field,
                            // Size matches capacity for immutable inputs
                            args.#size_field,
                        )
                    }});
                }
                IoArg::Output { .. } | IoArg::Result { .. } => {
                    internal_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                        >,
                    });
                    internal_args_sizes.push(quote! {
                        #[doc = #size_doc]
                        pub #size_field: u32,
                    });
                    internal_args_capacities.push(quote! {
                        #[doc = #capacity_doc]
                        pub #capacity_field: u32,
                    });

                    original_fn_args.push(quote! {&mut *{
                        // Ensure output type implements `IoTypeOptional`, which is required for
                        // handling of initially uninitialized type and implies implementation of
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
                            args.#capacity_field,
                        )
                    }});
                }
            }
        }

        let original_method_name = &fn_sig.ident;
        let ffi_fn_name = derive_ffi_fn_name(original_method_name, trait_name);
        let result_type = self.result_type.result_type();

        let result_var_name = format_ident!("result");
        let internal_args_struct = {
            // Result can be used through return type or argument, for argument no special handling
            // of the return type is needed
            if !matches!(self.io.last(), Some(IoArg::Result { .. })) {
                internal_args_pointers.push(quote! {
                    pub ok_result_ptr: ::core::ptr::NonNull<#result_type>,
                });
                internal_args_sizes.push(quote! {
                    /// Size of the contents `ok_result_ptr` points to
                    pub ok_result_size: u32,
                });
                internal_args_capacities.push(quote! {
                    /// Capacity of the allocated memory `ok_result_ptr` points to
                    pub ok_result_capacity: u32,
                });

                // Ensure return type implements not only `IoType`, which is required for crossing
                // host/guest boundary, but also `TrivialType` that ensures size matches capacity
                // and result handling is trivial, for variable size result `#[result]` must be used
                preparation.push(quote! {
                    debug_assert!(
                        args.ok_result_ptr.is_aligned(),
                        "`ok_result_ptr` pointer is misaligned"
                    );
                    debug_assert_eq!(
                        args.ok_result_size,
                        <#result_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                        "`ok_result_size` specified is invalid",
                    );
                    debug_assert_eq!(
                        args.ok_result_capacity,
                        <#result_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
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
                pub struct InternalArgs
                {
                    #( #internal_args_pointers )*
                    #( #internal_args_sizes )*
                    #( #internal_args_capacities )*
                }
            }
        };

        let guest_fn = {
            // Depending on whether `T` or `Result<T, ContractError>` is used as return type,
            // generate different code for result handling
            let result_handling = match &self.result_type {
                MethodResultType::Unit(_) => {
                    quote! {
                        // Return exit code
                        ::ab_contracts_macros::__private::ExitCode::Ok
                    }
                }
                MethodResultType::Regular(_) => {
                    quote! {
                        // Write result into `InternalArgs`, return exit code.
                        // It is okay to not write the size because return type is for `TrivialType`
                        // only, whose size is always fixed.
                        args.ok_result_ptr.write(#result_var_name);
                        ::ab_contracts_macros::__private::ExitCode::Ok
                    }
                }
                MethodResultType::ResultUnit(_) => {
                    quote! {
                        // Return exit code
                        match #result_var_name {
                            Ok(()) => ::ab_contracts_macros::__private::ExitCode::Ok,
                            Err(error) => error.exit_code(),
                        }
                    }
                }
                MethodResultType::Result(_) => {
                    quote! {
                        // Write result into `InternalArgs` if there is any, return exit code
                        match #result_var_name {
                            Ok(result) => {
                                // It is okay to not write the size because return type is for
                                // `TrivialType` only, whose size is always fixed
                                args.ok_result_ptr.write(result);
                                ::ab_contracts_macros::__private::ExitCode::Ok
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
                /// Caller must ensure provided pointer corresponds to expected ABI.
                #[cfg_attr(feature = "guest", unsafe(no_mangle))]
                #[allow(clippy::new_ret_no_self, reason = "Method was re-written for FFI purposes")]
                pub unsafe extern "C" fn #ffi_fn_name(
                    mut args: ::core::ptr::NonNull<InternalArgs>,
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
            let struct_name = extract_ident_from_type(self_type).ok_or_else(|| {
                Error::new(
                    self_type.span(),
                    "`#[contract]` must be applied to a simple struct without generics",
                )
            })?;
            let adapter_ffi_fn_name = format_ident!("{ffi_fn_name}_adapter");
            let args_struct_name = derive_external_args_struct_name(
                &self.self_type,
                trait_name,
                original_method_name,
            )?;

            quote! {
                #[cfg(any(unix, windows))]
                mod fn_pointer {
                    use super::*;

                    unsafe extern "C" fn #adapter_ffi_fn_name(
                        ptr: ::core::ptr::NonNull<::core::ffi::c_void>,
                    ) -> ::ab_contracts_macros::__private::ExitCode {
                        // SAFETY: Caller must ensure correct ABI of the void pointer, not much can
                        // be done here
                        unsafe { #ffi_fn_name(ptr.cast::<InternalArgs>()) }
                    }

                    #[::ab_contracts_macros::__private::linkme::distributed_slice(
                        ::ab_contracts_macros::__private::CONTRACTS_METHODS_FN_POINTERS
                    )]
                    static FN_POINTER: (
                        &str,
                        &::ab_contracts_macros::__private::MethodFingerprint,
                        &[u8],
                        unsafe extern "C" fn(::core::ptr::NonNull<::core::ffi::c_void>) -> ::ab_contracts_macros::__private::ExitCode,
                    ) = (
                        <#struct_name as ::ab_contracts_macros::__private::Contract>::CRATE_NAME,
                        &<#args_struct_name as ::ab_contracts_macros::__private::ExternalArgs>::FINGERPRINT,
                        &METADATA,
                        #adapter_ffi_fn_name,
                    );
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
        let original_method_name = &fn_sig.ident;

        let args_struct_name =
            derive_external_args_struct_name(&self.self_type, trait_name, original_method_name)?;
        // `external_args_pointers` will generate pointers in `*Args` fields
        let mut external_args_pointers = Vec::new();
        // `external_args_sizes` will generate sizes in `*Args` fields
        let mut external_args_sizes = Vec::new();
        // `external_args_capacities` will generate capacities in `*Args` fields
        let mut external_args_capacities = Vec::new();

        // For slots in external args only address is needed
        for slot in &self.slots {
            let ptr_field = format_ident!("{}_ptr", slot.arg_name);

            external_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<::ab_contracts_macros::__private::Address>,
            });
        }

        // Inputs and outputs with pointer and size (+ capacity if mutable)
        for io_arg in &self.io {
            let type_name = io_arg.type_name();
            let ptr_field = format_ident!("{}_ptr", io_arg.arg_name());
            let size_field = format_ident!("{}_size", io_arg.arg_name());
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", io_arg.arg_name());
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");

            match io_arg {
                IoArg::Input { .. } => {
                    external_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                        >,
                    });
                    external_args_sizes.push(quote! {
                        #[doc = #size_doc]
                        pub #size_field: u32,
                    });
                }
                IoArg::Output { .. } => {
                    external_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                        >,
                    });
                    external_args_sizes.push(quote! {
                        #[doc = #size_doc]
                        pub #size_field: u32,
                    });
                    external_args_capacities.push(quote! {
                        #[doc = #capacity_doc]
                        pub #capacity_field: u32,
                    });
                }
                IoArg::Result { .. } => {
                    // Initializer's return type will be `()` for caller, state is stored by the
                    // host and not returned to the caller, hence no explicit argument is needed
                    if !matches!(self.method_type, MethodType::Init) {
                        external_args_pointers.push(quote! {
                            pub #ptr_field: ::core::ptr::NonNull<
                                <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                            >,
                        });
                        external_args_sizes.push(quote! {
                            #[doc = #size_doc]
                            pub #size_field: u32,
                        });
                        external_args_capacities.push(quote! {
                            #[doc = #capacity_doc]
                            pub #capacity_field: u32,
                        });
                    }
                }
            }
        }

        let ffi_fn_name = derive_ffi_fn_name(original_method_name, trait_name);

        // Initializer's return type will be `()` for caller, state is stored by the host and not
        // returned to the caller, also if explicit `#[result]` argument is used return type is
        // also `()`. In both cases explicit arguments are not needed in `*Args` struct.
        if !(matches!(self.io.last(), Some(IoArg::Result { .. }))
            || matches!(self.method_type, MethodType::Init))
        {
            let result_type = &self.result_type.result_type();

            external_args_pointers.push(quote! {
                pub ok_result_ptr: ::core::ptr::NonNull<#result_type>,
            });
            external_args_sizes.push(quote! {
                /// Size of the contents `ok_result_ptr` points to
                pub ok_result_size: u32,
            });
            external_args_capacities.push(quote! {
                /// Capacity of the allocated memory `ok_result_ptr` points to
                pub ok_result_capacity: u32,
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
            pub struct #args_struct_name
            {
                #( #external_args_pointers )*
                #( #external_args_sizes )*
                #( #external_args_capacities )*
            }

            #[automatically_derived]
            unsafe impl ::ab_contracts_macros::__private::ExternalArgs for #args_struct_name {
                const FINGERPRINT: ::ab_contracts_macros::__private::MethodFingerprint =
                    ::ab_contracts_macros::__private::MethodFingerprint::new(METADATA)
                        .expect("Metadata is statically correct; qed");
            }

            // TODO: `*Args` constructor for easier usage (that fills in default
            //  capacities and sized), use it in extension trait implementation to reduce code
            //  duplication
        })
    }

    fn generate_metadata(
        &self,
        fn_sig: &Signature,
        trait_name: Option<&Ident>,
    ) -> Result<TokenStream, Error> {
        // `method_metadata` will generate metadata about method arguments, each element in this
        // vector corresponds to one argument
        let mut method_metadata = Vec::new();

        if let Some(mutability) = self.env {
            let env_metadata_type = if mutability.is_some() {
                "EnvRw"
            } else {
                "EnvRo"
            };

            let env_metadata_type = format_ident!("{env_metadata_type}");
            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#env_metadata_type as u8],
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
                &[
                    ::ab_contracts_macros::__private::ContractMetadataKind::#tmp_metadata_type as u8,
                    #( #arg_name_metadata, )*
                ],
            });
        }

        for slot in &self.slots {
            let with_address = slot.with_address_arg.is_some();
            let mutable = slot.mutability.is_some();
            let slot_metadata_type = match (with_address, mutable) {
                (true, true) => "SlotWithAddressRw",
                (true, false) => "SlotWithAddressRo",
                (false, true) => "SlotWithoutAddressRw",
                (false, false) => "SlotWithoutAddressRo",
            };

            let slot_metadata_type = format_ident!("{slot_metadata_type}");
            let arg_name_metadata = derive_ident_metadata(&slot.arg_name)?;
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_macros::__private::ContractMetadataKind::#slot_metadata_type as u8,
                    #( #arg_name_metadata, )*
                ],
            });
        }

        for io_arg in &self.io {
            let io_metadata_type = match io_arg {
                IoArg::Input { .. } => "Input",
                IoArg::Output { .. } => "Output",
                IoArg::Result { .. } => "Result",
            };

            let io_metadata_type = format_ident!("{io_metadata_type}");
            let arg_name_metadata = derive_ident_metadata(io_arg.arg_name())?;
            // Skip type metadata for `#[init]`'s result since it is known statically
            let with_type_metadata = if matches!(
                (self.method_type, io_arg),
                (MethodType::Init, IoArg::Result { .. })
            ) {
                None
            } else {
                let type_name = io_arg.type_name();
                Some(quote! {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::METADATA,
                })
            };
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_macros::__private::ContractMetadataKind::#io_metadata_type as u8,
                    #( #arg_name_metadata, )*
                ],
                #with_type_metadata
            });
        }

        if !matches!(self.io.last(), Some(IoArg::Result { .. })) {
            // There isn't an explicit name in case of the return type
            let arg_name_metadata = Literal::u8_unsuffixed(0);
            // Skip type metadata for `#[init]`'s result since it is known statically
            let with_type_metadata = if matches!(self.method_type, MethodType::Init) {
                None
            } else {
                let result_type = self.result_type.result_type();
                Some(quote! {
                    <#result_type as ::ab_contracts_macros::__private::IoType>::METADATA,
                })
            };
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_macros::__private::ContractMetadataKind::Result as u8,
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
                    } else {
                        "ViewStateful"
                    }
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
        let ffi_fn_name = derive_ffi_fn_name(original_method_name, trait_name);
        let method_name_metadata = derive_ident_metadata(&ffi_fn_name)?;
        Ok(quote_spanned! {fn_sig.span() =>
            const fn metadata()
                -> ([u8; ::ab_contracts_macros::__private::MAX_METADATA_CAPACITY], usize)
            {
                ::ab_contracts_macros::__private::concat_metadata_sources(&[
                    &[
                        ::ab_contracts_macros::__private::ContractMetadataKind::#method_type as u8,
                        #( #method_name_metadata, )*
                        #number_of_arguments,
                    ],
                    #( #method_metadata )*
                ])
            }

            /// Method metadata, see [`ContractMetadataKind`] for encoding details
            ///
            /// [`ContractMetadataKind`]: ::ab_contracts_macros::__private::ContractMetadataKind
            // Strange syntax to allow Rust to extend the lifetime of metadata scratch automatically
            pub const METADATA: &[u8] =
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
        let mut preparation = Vec::new();
        let mut method_args = Vec::new();
        let mut args_pointers = Vec::new();
        let mut args_sizes = Vec::new();
        let mut args_capacities = Vec::new();
        let mut result_processing = Vec::new();

        // Address of the contract
        method_args.push(quote! {
            contract: &::ab_contracts_macros::__private::Address,
        });

        // For each slot argument generate an address argument
        for slot in &self.slots {
            let arg_name = &slot.arg_name;
            let struct_field_ptr = format_ident!("{arg_name}_ptr");

            method_args.push(quote! {
                #arg_name: &::ab_contracts_macros::__private::Address,
            });
            args_pointers.push(quote! {
                // TODO: Use `NonNull::from_ref()` once stable
                #struct_field_ptr: ::core::ptr::NonNull::from(#arg_name),
            });
        }

        // For each I/O argument generate corresponding read-only or write-only argument
        for io_arg in &self.io {
            let arg_name = io_arg.arg_name();
            let struct_field_ptr = format_ident!("{arg_name}_ptr");
            let struct_field_size = format_ident!("{arg_name}_size");
            let struct_field_capacity = format_ident!("{arg_name}_capacity");

            match io_arg {
                IoArg::Input {
                    type_name,
                    arg_name,
                } => {
                    method_args.push(quote! {
                        #arg_name: &#type_name,
                    });
                    args_pointers.push(quote! {
                        // SAFETY: This pointer is used as input to FFI call and underlying data
                        // will not be modified, also pointer will not outlive the reference from
                        // which it was created despite copying
                        #struct_field_ptr: unsafe {
                            *::ab_contracts_macros::__private::IoType::as_ptr(#arg_name)
                        },
                    });
                    args_sizes.push(quote! {
                        #struct_field_size: ::ab_contracts_macros::__private::IoType::size(#arg_name),
                    });
                }
                IoArg::Output {
                    type_name,
                    arg_name,
                } => {
                    method_args.push(quote! {
                        #arg_name: &mut #type_name,
                    });
                    args_pointers.push(quote! {
                        // SAFETY: This pointer is used as input to FFI call and underlying data
                        // will only be modified there, also pointer will not outlive the reference
                        // from which it was created despite copying
                        #struct_field_ptr: unsafe {
                            let ptr =
                                *::ab_contracts_macros::__private::IoType::as_mut_ptr(#arg_name);
                            ptr
                        },
                    });
                    args_sizes.push(quote! {
                        #struct_field_size:
                            ::ab_contracts_macros::__private::IoType::size(#arg_name),
                    });
                    args_capacities.push(quote! {
                        #struct_field_capacity:
                            ::ab_contracts_macros::__private::IoType::capacity(#arg_name),
                    });
                    result_processing.push(quote! {
                        ::ab_contracts_macros::__private::IoType::set_size(
                            #arg_name,
                            args.#struct_field_size,
                        );
                    });
                }
                IoArg::Result {
                    type_name,
                    arg_name,
                } => {
                    // Initializer's return type will be `()` for caller, state is stored by the
                    // host and not returned to the caller
                    if !matches!(self.method_type, MethodType::Init) {
                        method_args.push(quote! {
                            #arg_name: &mut #type_name,
                        });
                        args_pointers.push(quote! {
                            // SAFETY: This pointer is used as input to FFI call and underlying data
                            // will only be modified there, also pointer will not outlive the
                            // reference from which it was created despite copying
                            #struct_field_ptr: unsafe {
                                let ptr = *::ab_contracts_macros::__private::IoType::as_mut_ptr(
                                    #arg_name,
                                );
                                ptr
                            },
                        });
                        args_sizes.push(quote! {
                            #struct_field_size:
                                ::ab_contracts_macros::__private::IoType::size(#arg_name),
                        });
                        args_capacities.push(quote! {
                            #struct_field_capacity:
                                ::ab_contracts_macros::__private::IoType::capacity(#arg_name),
                        });
                        result_processing.push(quote! {
                            ::ab_contracts_macros::__private::IoType::set_size(
                                #arg_name,
                                args.#struct_field_size,
                            );
                        });
                    }
                }
            }
        }

        let self_type = &self.self_type;
        let original_method_name = &fn_sig.ident;
        let ext_method_prefix = if let Some(trait_name) = trait_name {
            Some(trait_name)
        } else if let Type::Path(type_path) = self_type
            && let Some(path_segment) = type_path.path.segments.last()
        {
            Some(&path_segment.ident)
        } else {
            None
        };
        let ext_method_name = if let Some(ext_method_prefix) = ext_method_prefix {
            let ext_method_prefix =
                RenameRule::SnakeCase.apply_to_variant(ext_method_prefix.to_string());
            format_ident!("{ext_method_prefix}_{original_method_name}")
        } else {
            original_method_name.clone()
        };
        // Non-`#[view]` methods can only be called on `&mut Env`
        let env_self = if matches!(self.method_type, MethodType::View) {
            quote! { &self }
        } else {
            quote! { self: &&mut Self }
        };
        // `#[view]` methods do not require explicit method context
        let method_context_arg = (!matches!(self.method_type, MethodType::View)).then(|| {
            quote! {
                method_context: &::ab_contracts_macros::__private::MethodContext,
            }
        });
        // Initializer's return type will be `()` for caller, state is stored by the host and not
        // returned to the caller, also if explicit `#[result]` argument is used return type is
        // also `()`.
        let method_signature = if matches!(self.io.last(), Some(IoArg::Result { .. }))
            || matches!(self.method_type, MethodType::Init)
        {
            quote! {
                fn #ext_method_name(
                    #env_self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<(), ::ab_contracts_macros::__private::ContractError>
            }
        } else {
            let result_type = self.result_type.result_type();

            preparation.push(quote! {
                let mut ok_result = ::core::mem::MaybeUninit::uninit();
            });
            args_pointers.push(quote! {
                // SAFETY: Pointer created from an allocated struct
                ok_result_ptr: unsafe {
                    ::core::ptr::NonNull::new_unchecked(ok_result.as_mut_ptr())
                },
            });
            args_sizes.push(quote! {
                ok_result_size:
                    <#result_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
            });
            args_capacities.push(quote! {
                ok_result_capacity:
                    <#result_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
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
                    #result_type,
                    ::ab_contracts_macros::__private::ContractError,
                >
            }
        };

        let definitions = quote! {
            #method_signature;
        };

        let args_struct_name =
            derive_external_args_struct_name(&self.self_type, trait_name, original_method_name)?;
        // `#[view]` methods do not require explicit method context
        let method_context_value = if matches!(self.method_type, MethodType::View) {
            quote! { &::ab_contracts_macros::__private::MethodContext::Reset }
        } else {
            quote! { method_context }
        };
        let impls = quote! {
            #[inline]
            #method_signature {
                #( #preparation )*

                let mut args = #original_method_name::#args_struct_name {
                    #( #args_pointers )*
                    #( #args_sizes )*
                    #( #args_capacities )*
                };

                self.call(contract, &mut args, #method_context_value)?;

                // SAFETY: Non-error result above indicates successful storing of the result
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

fn derive_ffi_fn_name(original_method_name: &Ident, trait_name: Option<&Ident>) -> Ident {
    if let Some(trait_name) = trait_name {
        let ffi_fn_prefix = RenameRule::SnakeCase.apply_to_variant(trait_name.to_string());
        format_ident!("{ffi_fn_prefix}_{original_method_name}")
    } else {
        format_ident!("{original_method_name}")
    }
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
