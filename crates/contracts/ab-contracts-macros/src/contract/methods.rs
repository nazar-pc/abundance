use proc_macro2::{Ident, Literal, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    Error, GenericArgument, ImplItemFn, Pat, PatType, PathArguments, ReturnType, Token, Type,
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
        let (IoArg::Input { type_name, .. }
        | IoArg::Output { type_name, .. }
        | IoArg::Result { type_name, .. }) = self;
        type_name
    }

    fn arg_name(&self) -> &Ident {
        let (IoArg::Input { arg_name, .. }
        | IoArg::Output { arg_name, .. }
        | IoArg::Result { arg_name, .. }) = self;
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
            MethodResultType::Unit(ty)
            | MethodResultType::Regular(ty)
            | MethodResultType::ResultUnit(ty)
            | MethodResultType::Result(ty) => ty,
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
    struct_name: Type,
    state: Option<Option<Token![mut]>>,
    env: Option<Option<Token![mut]>>,
    slots: Vec<Slot>,
    io: Vec<IoArg>,
    result_type: MethodResultType,
}

impl MethodDetails {
    pub(super) fn new(method_type: MethodType, struct_name: Type) -> Self {
        Self {
            method_type,
            struct_name,
            state: None,
            env: None,
            slots: vec![],
            io: vec![],
            result_type: MethodResultType::Unit(MethodResultType::unit_type()),
        }
    }

    pub(super) fn same_slot_types<'a, I>(iter: I) -> bool
    where
        I: Iterator<Item = &'a Self> + 'a,
    {
        iter.flat_map(|method_metadata| method_metadata.slots.iter())
            .map_windows(|[a, b]| a.type_name == b.type_name)
            .all(|same| same)
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
                                        self.struct_name.clone(),
                                    ));
                                } else {
                                    self.set_result_type(MethodResultType::Result(ok_type.clone()));
                                }
                            } else {
                                return Err(Error::new(return_type.span(), error_message));
                            }
                        } else if last_path_segment.ident == "Self" {
                            // Swap `Self` for an actual struct name
                            self.set_result_type(MethodResultType::Regular(
                                self.struct_name.clone(),
                            ));
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
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        Self::process_env_arg(input_span, pat_type, metadata, false)
    }

    pub(super) fn process_env_arg_rw(
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        Self::process_env_arg(input_span, pat_type, metadata, true)
    }

    fn process_env_arg(
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
        allow_mut: bool,
    ) -> Result<(), Error> {
        if metadata.env.is_some() || !metadata.io.is_empty() {
            return Err(Error::new(
                input_span,
                "`#[env]` must be the first non-Self argument",
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

            metadata.env.replace(type_reference.mutability);
            Ok(())
        } else {
            Err(Error::new(
                pat_type.span(),
                "`#[env]` must be a reference to `Env` type (can be shared or exclusive)",
            ))
        }
    }

    pub(super) fn process_state_arg_ro(
        input_span: Span,
        ty: &Type,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        Self::process_state_arg(input_span, ty, metadata, false)
    }

    pub(super) fn process_state_arg_rw(
        input_span: Span,
        ty: &Type,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        Self::process_state_arg(input_span, ty, metadata, true)
    }

    fn process_state_arg(
        input_span: Span,
        ty: &Type,
        metadata: &mut Self,
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

            metadata.state.replace(type_reference.mutability);
            Ok(())
        } else {
            Err(Error::new(
                ty.span(),
                "Can't consume `Self`, use `&self` or `&mut self` instead",
            ))
        }
    }

    pub(super) fn process_slot_arg_ro(
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        Self::process_slot_arg(input_span, pat_type, metadata, false)
    }

    pub(super) fn process_slot_arg_rw(
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        Self::process_slot_arg(input_span, pat_type, metadata, true)
    }

    fn process_slot_arg(
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
        allow_mut: bool,
    ) -> Result<(), Error> {
        if !metadata.io.is_empty() {
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

                metadata.slots.push(Slot {
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

                    metadata.slots.push(Slot {
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
            to slot type like `(&Address, &mut VariableBytes<1024>)",
        ))
    }

    pub(super) fn process_input_arg(
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        if metadata
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

            metadata.io.push(IoArg::Input {
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
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        if metadata
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

            metadata.io.push(IoArg::Output {
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
        input_span: Span,
        pat_type: &PatType,
        metadata: &mut Self,
    ) -> Result<(), Error> {
        if !metadata.result_type.unit_result_type() {
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

            // Replace things like `MaybeData<Self>` with `MaybeData<#struct_name>`
            if let Type::Path(type_path) = &mut type_name
                && let Some(path_segment) = type_path.path.segments.first_mut()
                && let PathArguments::AngleBracketed(generic_arguments) =
                    &mut path_segment.arguments
                && let Some(GenericArgument::Type(first_generic_argument)) =
                    generic_arguments.args.first_mut()
                && let Type::Path(type_path) = &first_generic_argument
                && type_path.path.is_ident("Self")
            {
                *first_generic_argument = metadata.struct_name.clone();
            }

            metadata.io.push(IoArg::Result {
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
        impl_item_fn: &ImplItemFn,
    ) -> Result<TokenStream, Error> {
        let struct_name = &self.struct_name;
        if matches!(self.method_type, MethodType::Init) {
            let self_return_type = self.result_type.result_type() == struct_name;
            let self_result_type = self
                .io
                .last()
                .map(|io_arg| {
                    // Match things like `MaybeData<#struct_name>`
                    if let IoArg::Result { type_name, .. } = io_arg
                        && let Type::Path(type_path) = type_name
                        && let Some(path_segment) = type_path.path.segments.last()
                        && let PathArguments::AngleBracketed(generic_arguments) =
                            &path_segment.arguments
                        && let Some(GenericArgument::Type(first_generic_argument)) =
                            generic_arguments.args.first()
                    {
                        first_generic_argument == struct_name
                    } else {
                        false
                    }
                })
                .unwrap_or_default();

            if !(self_return_type || self_result_type) || (self_return_type && self_result_type) {
                return Err(Error::new(
                    impl_item_fn.sig.span(),
                    "`#[init]` must have result type of `Self` as either return type or explicit \
                    `#[result]` argument, but not both",
                ));
            }
        }

        // `internal_args_pointers` will generate pointers in `InternalArgs` fields
        let mut internal_args_pointers = Vec::new();
        // `internal_args_sizes` will generate sizes in `InternalArgs` fields
        let mut internal_args_sizes = Vec::new();
        // `internal_args_capacities` will generate capacities in `InternalArgs` fields
        let mut internal_args_capacities = Vec::new();
        // `preparation` will generate code that is used before calling original function
        let mut preparation = Vec::new();
        // `external_args_pointers` will generate pointers in `ExternalArgs` fields
        let mut external_args_pointers = Vec::new();
        // `external_args_sizes` will generate sizes in `ExternalArgs` fields
        let mut external_args_sizes = Vec::new();
        // `external_args_capacities` will generate capacities in `ExternalArgs` fields
        let mut external_args_capacities = Vec::new();
        // `original_fn_args` will generate arguments for calling original method implementation
        let mut original_fn_args = Vec::new();
        // `method_metadata` will generate metadata about method arguments, each element in this
        // vector corresponds to one argument
        let mut method_metadata = Vec::new();

        // Optional state argument with pointer and size (+ capacity if mutable)
        if let Some(mutability) = self.state {
            internal_args_pointers.push(quote! {
                pub state_ptr: ::core::ptr::NonNull<
                    <#struct_name as ::ab_contracts_io_type::IoType>::PointerType,
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

                original_fn_args.push(quote! {{
                    // Ensure state type implements `IoType`, which is required for crossing
                    // host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type<T: ::ab_contracts_io_type::IoType>() {}
                        assert_impl_io_type::<#struct_name>();
                    };

                    &mut <#struct_name as ::ab_contracts_io_type::IoType>::from_ptr_mut(
                        &mut args.state_ptr,
                        &mut args.state_size,
                        args.state_capacity,
                    )
                }});
            } else {
                original_fn_args.push(quote! {{
                    // Ensure state type implements `IoType`, which is required for crossing
                    // host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type<T: ::ab_contracts_io_type::IoType>() {}
                        assert_impl_io_type::<#struct_name>();
                    };

                    &<#struct_name as ::ab_contracts_io_type::IoType>::from_ptr(
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
                pub env_ptr: ::core::ptr::NonNull<::ab_contracts_common::env::Env>,
            });

            if mutability.is_some() {
                original_fn_args.push(quote! {
                    debug_assert!(args.env_ptr.is_aligned(), "`env_ptr` pointer is misaligned");
                    args.env_ptr.as_mut()
                });

                method_metadata.push(quote! {
                    &[::ab_contracts_common::ContractMethodMetadata::EnvRw as u8],
                });
            } else {
                original_fn_args.push(quote! {
                    debug_assert!(args.env_ptr.is_aligned(), "`env_ptr` pointer is misaligned");
                    args.env_ptr.as_ref()
                });

                method_metadata.push(quote! {
                    &[::ab_contracts_common::ContractMethodMetadata::EnvRo as u8],
                });
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
                    pub #address_ptr: ::core::ptr::NonNull<::ab_contracts_common::Address>,
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
                    <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
                >,
            });
            internal_args_sizes.push(quote! {
                #[doc = #size_doc]
                pub #size_field: u32,
            });
            external_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<::ab_contracts_common::Address>,
            });

            let arg_extraction = if mutability.is_some() {
                internal_args_capacities.push(quote! {
                    #[doc = #capacity_doc]
                    pub #capacity_field: u32,
                });

                quote! {{
                    // Ensure slot type implements `IoTypeOptional`, which is required for handling
                    // of slot that might be removed or not present and implies implementation of
                    // `IoType`, which is required for crossing host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type_optional<T: ::ab_contracts_io_type::IoTypeOptional>() {}
                        assert_impl_io_type_optional::<#type_name>();
                    };

                    &mut <#type_name as ::ab_contracts_io_type::IoType>::from_ptr_mut(
                        &mut args.#ptr_field,
                        &mut args.#size_field,
                        args.#capacity_field,
                    )
                }}
            } else {
                quote! {{
                    // Ensure slot type implements `IoTypeOptional`, which is required for handling
                    // of slot that might be removed or not present and implies implementation of
                    // `IoType`, which is required for crossing host/guest boundary
                    const _: () = {
                        const fn assert_impl_io_type_optional<T: ::ab_contracts_io_type::IoTypeOptional>() {}
                        assert_impl_io_type_optional::<#type_name>();
                    };

                    &<#type_name as ::ab_contracts_io_type::IoType>::from_ptr(
                        &args.#ptr_field,
                        &args.#size_field,
                        // Size matches capacity for immutable inputs
                        args.#size_field,
                    )
                }}
            };

            let slot_metadata_type;
            if let Some(address_arg) = &slot.with_address_arg {
                let address_ptr = format_ident!("{address_arg}_ptr");
                original_fn_args.push(quote! {{
                    (
                        &<::ab_contracts_common::Address as ::ab_contracts_io_type::IoType>::from_ptr(
                            &args.#address_ptr,
                            &<::ab_contracts_common::Address as ::ab_contracts_io_type::trivial_type::TrivialType>::SIZE,
                            <::ab_contracts_common::Address as ::ab_contracts_io_type::trivial_type::TrivialType>::SIZE,
                        ),
                        #arg_extraction,
                    )
                }});

                slot_metadata_type = if mutability.is_some() {
                    format_ident!("SlotWithAddressRw")
                } else {
                    format_ident!("SlotWithAddressRo")
                };
            } else {
                original_fn_args.push(arg_extraction);

                slot_metadata_type = if mutability.is_some() {
                    format_ident!("SlotWithoutAddressRw")
                } else {
                    format_ident!("SlotWithoutAddressRo")
                };
            }

            let arg_name_metadata = derive_ident_metadata(&slot.arg_name)?;
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_common::ContractMethodMetadata::#slot_metadata_type as u8,
                    #( #arg_name_metadata, )*
                ],
                <#type_name as ::ab_contracts_io_type::IoType>::METADATA,
            });
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

            let io_metadata_type = match io_arg {
                IoArg::Input { .. } => {
                    internal_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
                        >,
                    });
                    internal_args_sizes.push(quote! {
                        #[doc = #size_doc]
                        pub #size_field: u32,
                    });

                    external_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
                        >,
                    });
                    external_args_sizes.push(quote! {
                        #[doc = #size_doc]
                        pub #size_field: u32,
                    });

                    original_fn_args.push(quote! {
                        // Ensure input type implements `IoType`, which is required for crossing
                        // host/guest boundary
                        const _: () = {
                            const fn assert_impl_io_type<T: ::ab_contracts_io_type::IoType>() {}
                            assert_impl_io_type::<#type_name>();
                        };

                        &<#type_name as ::ab_contracts_io_type::IoType>::from_ptr(
                            &args.#ptr_field,
                            &args.#size_field,
                            // Size matches capacity for immutable inputs
                            args.#size_field,
                        )
                    });

                    format_ident!("Input")
                }
                IoArg::Output { .. } => {
                    internal_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
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

                    external_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
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

                    original_fn_args.push(quote! {
                        // Ensure output type implements `IoTypeOptional`, which is required for
                        // handling of initially uninitialized type and implies implementation of
                        // `IoType`, which is required for crossing host/guest boundary
                        const _: () = {
                            const fn assert_impl_io_type_optional<T: ::ab_contracts_io_type::IoTypeOptional>() {}
                            assert_impl_io_type_optional::<#type_name>();
                        };

                        &mut <#type_name as ::ab_contracts_io_type::IoType>::from_ptr_mut(
                            &mut args.#ptr_field,
                            &mut args.#size_field,
                            args.#capacity_field,
                        )
                    });

                    format_ident!("Output")
                }
                IoArg::Result { .. } => {
                    internal_args_pointers.push(quote! {
                        pub #ptr_field: ::core::ptr::NonNull<
                            <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
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

                    // Initializer's return type will be `()` for caller, state is stored by the
                    // host and not returned to the caller
                    if matches!(self.method_type, MethodType::Init) {
                        external_args_pointers.push(quote! {
                            pub #ptr_field: ::core::ptr::NonNull<()>,
                        });
                        external_args_sizes.push(quote! {
                            #[doc = #size_doc]
                            pub #size_field: u32,
                        });
                        external_args_capacities.push(quote! {
                            #[doc = #capacity_doc]
                            pub #capacity_field: u32,
                        });
                    } else {
                        external_args_pointers.push(quote! {
                            pub #ptr_field: ::core::ptr::NonNull<
                                <#type_name as ::ab_contracts_io_type::IoType>::PointerType,
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

                    original_fn_args.push(quote! {
                        // Ensure result type implements `IoTypeOptional`, which is required for
                        // handling of initially uninitialized type and implies implementation of
                        // `IoType`, which is required for crossing host/guest boundary
                        const _: () = {
                            const fn assert_impl_io_type_optional<T: ::ab_contracts_io_type::IoTypeOptional>() {}
                            assert_impl_io_type_optional::<#type_name>();
                        };

                        &mut <#type_name as ::ab_contracts_io_type::IoType>::from_ptr_mut(
                            &mut args.#ptr_field,
                            &mut args.#size_field,
                            args.#capacity_field,
                        )
                    });

                    format_ident!("Result")
                }
            };

            let arg_name_metadata = derive_ident_metadata(io_arg.arg_name())?;
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_common::ContractMethodMetadata::#io_metadata_type as u8,
                    #( #arg_name_metadata, )*
                ],
                <#type_name as ::ab_contracts_io_type::IoType>::METADATA,
            });
        }

        let original_method_name = &impl_item_fn.sig.ident;
        let result_type = self.result_type.result_type();

        let result_var_name = format_ident!("result");
        let internal_args_struct = {
            // Result can be used through return type or argument, for argument no special handling
            // of return type is needed
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
                        <#result_type as ::ab_contracts_io_type::trivial_type::TrivialType>::SIZE,
                        "`ok_result_size` specified is invalid",
                    );
                    debug_assert_eq!(
                        args.ok_result_capacity,
                        <#result_type as ::ab_contracts_io_type::trivial_type::TrivialType>::SIZE,
                        "`ok_result_capacity` specified is invalid",
                    );
                });

                // Placeholder argument name to keep metadata consistent
                let arg_name_metadata = derive_ident_metadata(&result_var_name)?;
                method_metadata.push(quote! {
                    &[
                        ::ab_contracts_common::ContractMethodMetadata::Result as u8,
                        #( #arg_name_metadata, )*
                    ],
                    <#result_type as ::ab_contracts_io_type::IoType>::METADATA,
                });
            }
            let args_struct_doc = format!(
                "Data structure containing expected input to [`{original_method_name}()`], it is \
                used internally by the contract, there should be no need to construct it \
                explicitly except maybe in contract's own tests"
            );
            quote_spanned! {impl_item_fn.sig.span() =>
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
                        ::ab_contracts_common::ExitCode::Ok
                    }
                }
                MethodResultType::Regular(_) => {
                    quote! {
                        <#result_type as ::ab_contracts_io_type::IoType>::from_ptr(
                            &mut args.ok_result_ptr,
                            &mut args.ok_result_size,
                            args.ok_result_capacity,
                        );
                        // Write result into `InternalArgs`, return exit code
                        args.ok_result_ptr.write(#result_var_name);
                        ::ab_contracts_common::ExitCode::Ok
                    }
                }
                MethodResultType::ResultUnit(_) => {
                    quote! {
                        // Return exit code
                        match #result_var_name {
                            Ok(()) => ::ab_contracts_common::ExitCode::Ok,
                            Err(error) => error.exit_code(),
                        }
                    }
                }
                MethodResultType::Result(_) => {
                    quote! {
                        // Write result into `InternalArgs` if there is any, return exit code
                        match #result_var_name {
                            Ok(result) => {
                                args.ok_result_ptr.write(result);
                                ::ab_contracts_common::ExitCode::Ok
                            }
                            Err(error) => error.exit_code(),
                        }
                    }
                }
            };

            // Generate FFI function with original name (hiding original implementation), but
            // exported as shortcut name
            quote_spanned! {impl_item_fn.sig.span() =>
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
                pub unsafe extern "C" fn #original_method_name(
                    mut args: ::core::ptr::NonNull<InternalArgs>,
                ) -> ::ab_contracts_common::ExitCode {
                    debug_assert!(args.is_aligned(), "`args` pointer is misaligned");
                    let args = args.as_mut();

                    #( #preparation )*

                    // Call inner function via normal Rust API
                    #[allow(
                        unused_braces,
                        reason = "Boilerplate to suppress when not used, seems to be false-positive"
                    )]
                    let #result_var_name = #struct_name::#original_method_name(
                        #( { #original_fn_args }, )*
                    );

                    #result_handling
                }
            }
        };

        let external_args_struct = {
            // Result can be used through return type or argument, for argument no special handling
            // of return type is needed
            if !matches!(self.io.last(), Some(IoArg::Result { .. })) {
                // Initializer's return type will be `()` for caller, state is stored by the host \
                // and not returned to the caller
                let result_type = if matches!(self.method_type, MethodType::Init) {
                    quote! { () }
                } else {
                    let result_type = &self.result_type.result_type();
                    quote! { #result_type }
                };

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
                "Data structure containing expected input for external method invocation, \
                eventually calling [`{original_method_name}()`] on the other side by the host. \
                \n\nThis can be used with [`Env`](::ab_contracts_common::env::Env), though there \
                are helper methods on this provided by extension trait that allow not dealing with \
                this struct directly in simpler cases."
            );
            quote_spanned! {impl_item_fn.sig.span() =>
                #[doc = #args_struct_doc]
                #[repr(C)]
                pub struct ExternalArgs
                {
                    #( #external_args_pointers )*
                    #( #external_args_sizes )*
                    #( #external_args_capacities )*
                }

                #[automatically_derived]
                unsafe impl ::ab_contracts_common::method::ExternalArgs for ExternalArgs {
                    const FINGERPRINT: &::ab_contracts_common::method::MethodFingerprint =
                        &FINGERPRINT;
                }

                // TODO: `ExternalArgs` constructor for easier usage (that fills in default
                //  capacities and sized), use it in extension trait implementation to reduce code
                //  duplication
            }
        };

        let metadata = {
            let method_type = match self.method_type {
                MethodType::Init => "Init",
                MethodType::Update => {
                    if let Some(mutable) = &self.state {
                        if mutable.is_some() {
                            "CallStatefulRw"
                        } else {
                            "CallStatefulRo"
                        }
                    } else {
                        "CallStateless"
                    }
                }
                MethodType::View => {
                    if let Some(mutable) = &self.state {
                        if mutable.is_some() {
                            return Err(Error::new(
                                impl_item_fn.sig.span(),
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
            let method_type = format_ident!("{method_type}{}", method_metadata.len());

            let method_name_metadata = derive_ident_metadata(original_method_name)?;
            quote_spanned! {impl_item_fn.sig.span() =>
                const fn metadata() -> ([u8; 4096], usize) {
                    ::ab_contracts_io_type::utils::concat_metadata_sources(&[
                        &[
                            ::ab_contracts_common::ContractMethodMetadata::#method_type as u8,
                            #( #method_name_metadata, )*
                        ],
                        #( #method_metadata )*
                    ])
                }

                /// Method metadata, see
                /// [`ContractMethodMetadata`](::ab_contracts_common::ContractMethodMetadata) for
                /// encoding details
                // Strange syntax to allow Rust to extend lifetime of metadata scratch automatically
                pub const METADATA: &[u8] =
                    metadata()
                        .0
                        .split_at(metadata().1)
                        .0;

                /// Method fingerprint
                // TODO: Reduce metadata to essentials from above full metadata by collapsing tuple
                //  structs, removing field and struct names, leaving just function signatures and
                //  compact representation of data structures used for arguments
                pub const FINGERPRINT: ::ab_contracts_common::method::MethodFingerprint =
                    ::ab_contracts_common::method::MethodFingerprint::new(METADATA);
            }
        };

        Ok(quote! {
            pub mod #original_method_name {
                use super::*;

                #internal_args_struct
                #guest_fn
                #external_args_struct
                #metadata
            }
        })
    }

    pub(super) fn generate_trait_ext_components(
        &self,
        impl_item_fn: &ImplItemFn,
    ) -> ExtTraitComponents {
        let original_method_name = &impl_item_fn.sig.ident;

        let mut preparation = Vec::new();
        let mut method_args = Vec::new();
        let mut args_pointers = Vec::new();
        let mut args_sizes = Vec::new();
        let mut args_capacities = Vec::new();
        let mut result_processing = Vec::new();

        // Address of the contract
        method_args.push(quote! {
            contract: &::ab_contracts_common::Address,
        });

        // For each slot argument generate an address argument
        for slot in &self.slots {
            let arg_name = &slot.arg_name;
            let struct_field_ptr = format_ident!("{arg_name}_ptr");

            method_args.push(quote! {
                #arg_name: &::ab_contracts_common::Address,
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

            args_sizes.push(quote! {
                #struct_field_size: ::ab_contracts_io_type::IoType::size(#arg_name),
            });
            match io_arg {
                IoArg::Input {
                    type_name,
                    arg_name,
                } => {
                    method_args.push(quote! {
                        #arg_name: &#type_name,
                    });
                    args_pointers.push(quote! {
                        // TODO: Use `NonNull::from_ref()` once stable
                        #struct_field_ptr: ::core::ptr::NonNull::from(#arg_name),
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
                        #struct_field_ptr: *::ab_contracts_io_type::IoTypeOptional::as_mut_ptr(#arg_name),
                    });
                    args_capacities.push(quote! {
                        #struct_field_capacity: ::ab_contracts_io_type::IoType::capacity(#arg_name),
                    });
                    result_processing.push(quote! {
                        ::ab_contracts_io_type::IoType::set_size(
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
                    if matches!(self.method_type, MethodType::Init) {
                        method_args.push(quote! {
                            #arg_name: &mut ::ab_contracts_io_type::maybe_data::MaybeData<()>,
                        });
                    } else {
                        method_args.push(quote! {
                            #arg_name: &mut #type_name,
                        });
                    }
                    args_pointers.push(quote! {
                        #struct_field_ptr: *::ab_contracts_io_type::IoTypeOptional::as_mut_ptr(#arg_name),
                    });
                    args_capacities.push(quote! {
                        #struct_field_capacity: ::ab_contracts_io_type::IoType::capacity(#arg_name),
                    });
                    result_processing.push(quote! {
                        ::ab_contracts_io_type::IoType::set_size(
                            #arg_name,
                            args.#struct_field_size,
                        );
                    });
                }
            }
        }

        // Non-`#[view]` methods can only be called on `&mut Env`
        let env_mut = (!matches!(self.method_type, MethodType::View)).then(|| quote! { &mut });
        // `#[view]` methods do not require explicit method context
        let method_context_arg = (!matches!(self.method_type, MethodType::View)).then(|| {
            quote! {
                method_context: &::ab_contracts_common::env::MethodContext,
            }
        });
        let method_signature = if !matches!(self.io.last(), Some(IoArg::Result { .. })) {
            // Initializer's return type will be `()` for caller, state is stored by the host and
            // not returned to the caller
            let result_type = if matches!(self.method_type, MethodType::Init) {
                quote! { () }
            } else {
                let result_type = &self.result_type.result_type();
                quote! { #result_type }
            };

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
                ok_result_size: <#result_type as ::ab_contracts_io_type::trivial_type::TrivialType>::SIZE,
            });
            args_capacities.push(quote! {
                ok_result_capacity: <#result_type as ::ab_contracts_io_type::trivial_type::TrivialType>::SIZE,
            });
            result_processing.push(quote! {
                // This is fine for `TrivialType` types
                ok_result.assume_init()
            });

            quote! {
                fn #original_method_name(
                    self: &#env_mut Self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<#result_type, ::ab_contracts_common::ContractError>
            }
        } else {
            quote! {
                fn #original_method_name(
                    self: &#env_mut Self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<(), ::ab_contracts_common::ContractError>
            }
        };

        let definitions = quote! {
            #method_signature;
        };

        // `#[view]` methods do not require explicit method context
        let method_context_value = if matches!(self.method_type, MethodType::View) {
            quote! { &::ab_contracts_common::env::MethodContext::Reset }
        } else {
            quote! { method_context }
        };
        let impls = quote! {
            #[inline]
            #method_signature {
                #( #preparation )*

                let mut args = #original_method_name::ExternalArgs {
                    #( #args_pointers )*
                    #( #args_sizes )*
                    #( #args_capacities )*
                };

                self.call(&contract, &mut args, #method_context_value)?;

                // SAFETY: Non-error result above indicates successful storing of the result
                let result = unsafe {
                    #( #result_processing )*
                };

                Ok(result)
            }
        };

        ExtTraitComponents { definitions, impls }
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

fn derive_ident_metadata(ident: &Ident) -> Result<impl Iterator<Item = TokenTree>, Error> {
    let ident_string = ident.to_string();
    let ident_bytes = ident_string.as_bytes().to_vec();
    let ident_bytes_len = u8::try_from(ident_bytes.len()).map_err(|_error| {
        Error::new(
            ident.span(),
            format!(
                "Name of the field not be more than {} bytes in length",
                u8::MAX
            ),
        )
    })?;

    Ok(
        [TokenTree::Literal(Literal::u8_unsuffixed(ident_bytes_len))]
            .into_iter()
            .chain(
                ident_bytes
                    .into_iter()
                    .map(|char| TokenTree::Literal(Literal::byte_character(char))),
            ),
    )
}
