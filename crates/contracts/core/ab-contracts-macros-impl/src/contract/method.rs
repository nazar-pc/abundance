use crate::contract::common::{
    derive_ident_metadata, extract_ident_from_type, max_depth_assertion, replace_lifetimes,
    with_inferred_lifetimes, with_lifetime,
};
use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::visit::Visit;
use syn::{
    Attribute, Error, GenericArgument, Lifetime, Meta, Pat, PatType, PathArguments, Receiver,
    ReceiverKind, ReturnType, Signature, Token, Type, TypeTuple,
};

const SLOT_NAMED_LIFETIME_ERROR: &str = "`#[slot]` must not have named lifetimes, since slots \
    are trusted without being parsed and must not be lent to calls with `Env::lend_output()`, lend \
    a copy made with `Lent::new()` instead";

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

/// Kinds of method arguments, in the order they must appear in
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
enum ArgumentKind {
    Env,
    Tmp,
    Slot,
    Input,
    Output,
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
    /// The first named lifetime in the type (see `named_lifetime_in_type()`)
    named_lifetime: Option<Lifetime>,
}

impl Output {
    /// The type for `impl OutputArg<_>` in arguments of generated methods, and the lifetime that
    /// these methods need to declare for it, if any.
    ///
    /// `'_` is not allowed in `impl Trait` in arguments, so lifetimes in the type are replaced with
    /// one named after the output.
    fn output_arg_type(&self) -> (Type, Option<Lifetime>) {
        let mut type_name = self.type_name.clone();
        let lifetime = Lifetime {
            apostrophe: self.arg_name.span(),
            ident: self.arg_name.clone(),
        };
        let replaced = replace_lifetimes(&mut type_name, &lifetime);
        (type_name, replaced.then_some(lifetime))
    }
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
            attrs: Vec::new(),
            paren_token: Paren::default(),
            elems: Punctuated::default(),
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
    pub(super) definition: TokenStream,
    pub(super) r#impl: TokenStream,
}

pub(super) struct MethodDetails {
    method_type: MethodType,
    self_type: Type,
    #[expect(clippy::option_option, reason = "Intentional storing of syn's type")]
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

    /// Returns `#[tmp]` type (or `()` if it is not used) if all methods have the same slots type.
    ///
    /// Lifetimes in the type are replaced with `'static`, so that `Contract::Tmp` can name it.
    /// `'static` is only used at the type level, like for the pointer type in `InternalArgs`,
    /// metadata and type checks. The value a method gets is created by an `IoType` method like
    /// `IoType::from_mut_ptr()`. It gives the value the lifetime of the borrow of `args` of the FFI
    /// function instead, whatever lifetime the type has (see `IoType::WithLifetime`).
    pub(super) fn tmp_type<'a, I>(iter: I) -> Option<Type>
    where
        I: Iterator<Item = &'a Self> + 'a,
    {
        let mut tmp_type = None;
        for slot in iter.flat_map(|method_details| &method_details.tmp) {
            let type_name = with_lifetime(&slot.type_name, "'static");
            match &tmp_type {
                Some(tmp_type) => {
                    if tmp_type != &type_name {
                        return None;
                    }
                }
                None => {
                    tmp_type.replace(type_name);
                }
            }
        }

        Some(tmp_type.unwrap_or_else(MethodReturnType::unit_type))
    }

    /// Returns `#[slot]` type (or `()` if it is not used) if all methods have the same slots type.
    ///
    /// Lifetimes in the type are replaced with `'static`, like in [`Self::tmp_type()`].
    pub(super) fn slot_type<'a, I>(iter: I) -> Option<Type>
    where
        I: Iterator<Item = &'a Self> + 'a,
    {
        let mut slot_type = None;
        for slot in iter.flat_map(|method_details| &method_details.slots) {
            let type_name = with_lifetime(&slot.type_name, "'static");
            match &slot_type {
                Some(slot_type) => {
                    if slot_type != &type_name {
                        return None;
                    }
                }
                None => {
                    slot_type.replace(type_name);
                }
            }
        }

        Some(slot_type.unwrap_or_else(MethodReturnType::unit_type))
    }

    /// The kind of the last argument processed so far
    fn last_argument_kind(&self) -> Option<ArgumentKind> {
        if !self.outputs.is_empty() {
            Some(ArgumentKind::Output)
        } else if !self.inputs.is_empty() {
            Some(ArgumentKind::Input)
        } else if !self.slots.is_empty() {
            Some(ArgumentKind::Slot)
        } else if self.tmp.is_some() {
            Some(ArgumentKind::Tmp)
        } else if self.env.is_some() {
            Some(ArgumentKind::Env)
        } else {
            None
        }
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
        if self.last_argument_kind().is_some() {
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
        receiver: &Receiver,
    ) -> Result<(), Error> {
        self.process_state_arg(input_span, receiver, false)
    }

    pub(super) fn process_state_arg_rw(
        &mut self,
        input_span: Span,
        receiver: &Receiver,
    ) -> Result<(), Error> {
        self.process_state_arg(input_span, receiver, true)
    }

    fn process_state_arg(
        &mut self,
        input_span: Span,
        receiver: &Receiver,
        allow_mut: bool,
    ) -> Result<(), Error> {
        // Only accept `&self` or `&mut self`
        let ReceiverKind::Reference(_, lifetime, mutability) = &receiver.kind else {
            return Err(Error::new(
                input_span,
                "Can't consume `Self`, use `&self` or `&mut self` instead",
            ));
        };
        if let Some(lifetime) = named_lifetime(lifetime.as_ref()) {
            return Err(Error::new(
                lifetime.span(),
                "The state must not have a named lifetime, since it is trusted without being \
                parsed and must not be lent to calls with `Env::lend_output()`, lend a copy made \
                with `Lent::new()` instead",
            ));
        }
        if mutability.is_some() && !allow_mut {
            return Err(Error::new(
                input_span,
                "`#[arg]` is not allowed to mutate data here",
            ));
        }

        self.state.replace(*mutability);
        Ok(())
    }

    pub(super) fn process_tmp_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        if self.last_argument_kind() >= Some(ArgumentKind::Tmp) {
            return Err(Error::new(
                input_span,
                "`#[tmp]` must appear only once before any `#[slot]`, `#[input]` or `#[output]`",
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
            if let Some(lifetime) = named_lifetime_in_type(&pat_type.ty) {
                return Err(Error::new(
                    lifetime.span(),
                    "`#[tmp]` must not have a named lifetime, since it is trusted without being \
                    parsed and must not be lent to calls with `Env::lend_output()`, lend a copy \
                    made with `Lent::new()` instead",
                ));
            }

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
            shared or exclusive) like `&MaybeData<'_, Slot>` or `&mut VariableBytes<'_, 1024>`",
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
        if self.last_argument_kind() > Some(ArgumentKind::Slot) {
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
                if let Some(lifetime) = named_lifetime_in_type(&pat_type.ty) {
                    return Err(Error::new(lifetime.span(), SLOT_NAMED_LIFETIME_ERROR));
                }

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
                    if let Some(lifetime) = named_lifetime_in_type(&pat_type.ty) {
                        return Err(Error::new(lifetime.span(), SLOT_NAMED_LIFETIME_ERROR));
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
            shared or exclusive) like `&MaybeData<'_, Slot>` or a tuple of references to address \
            and to slot type like `(&Address, &mut VariableBytes<'_, 1024>)`",
        ))
    }

    pub(super) fn process_input_arg(
        &mut self,
        input_span: Span,
        pat_type: &PatType,
    ) -> Result<(), Error> {
        if self.last_argument_kind() > Some(ArgumentKind::Input) {
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
            // Unlike inputs, reference patterns like `&mut value` are not allowed, since the output
            // could not be modified through them
            let Pat::Ident(pat_ident) = &*pat_type.pat else {
                return Err(Error::new(
                    pat_type.span(),
                    "`#[output]` argument name must be a simple variable",
                ));
            };

            let mut type_name = type_reference.elem.as_ref().clone();
            let mut has_self = false;

            // Replace things like `MaybeData<'_, Self>` with `MaybeData<'_, #self_type>`
            if let Type::Path(type_path) = &mut type_name
                && let Some(path_segment) = type_path.path.segments.first_mut()
                && let PathArguments::AngleBracketed(generic_arguments) =
                    &mut path_segment.arguments
                && let Some(first_generic_argument) =
                    generic_arguments
                        .args
                        .iter_mut()
                        .find_map(|generic_argument| match generic_argument {
                            GenericArgument::Type(generic_argument) => Some(generic_argument),
                            _ => None,
                        })
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
                named_lifetime: named_lifetime_in_type(&pat_type.ty).cloned(),
            });
            Ok(())
        } else {
            Err(Error::new(
                pat_type.span(),
                "`#[output]` must be an exclusive reference to a type implementing `IoType` (it \
                can be both read and written to) like `&mut MaybeData<'_, Output>` or \
                `&mut VariableBytes<'_, 1024>`",
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
            // The host stores the return value as the state of the contract, and only without one
            // the last `#[output]`
            let state_is_self = if self.return_type.unit_return_type() {
                self.outputs.last().is_some_and(|output| output.has_self)
            } else {
                self.return_type.return_type() == self_type
            };

            if !state_is_self {
                return Err(Error::new(
                    fn_sig.span(),
                    "`#[init]` must return `Self` or, without a return value, have `Self` as the \
                    last `#[output]` argument",
                ));
            }

            if self.return_type.unit_return_type()
                && let Some(state_output) = self.outputs.last()
                && let Some(lifetime) = &state_output.named_lifetime
            {
                return Err(Error::new(
                    lifetime.span(),
                    "The last `#[output]` of `#[init]` without a return value is the initial \
                    state, which must not have a named lifetime, since the state is trusted \
                    without being parsed and must not be lent to calls with \
                    `Env::lend_output()`",
                ));
            }
        }

        let original_method_name = &fn_sig.ident;

        let guest_fn = self.generate_guest_fn(fn_sig, trait_name)?;
        let external_args_struct = self.generate_external_args_struct(fn_sig, trait_name)?;
        let metadata = self.generate_metadata(fn_sig, trait_name)?;

        Ok(quote_spanned! {fn_sig.span() =>
            #[expect(clippy::inline_modules, reason = "Macro-generated")]
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

        Ok(quote_spanned! {fn_sig.span() =>
            #[expect(clippy::inline_modules, reason = "Macro-generated")]
            pub mod #original_method_name {
                #[allow(
                    unused_imports,
                    reason = "Unused when types of the method need nothing from the parent module, \
                    which the macro can't tell without resolving names"
                )]
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
        // `preparation` will generate code used before calling the original function
        let mut preparation = Vec::new();
        // `views` will generate local variables with views of arguments, which live until the
        // function returns
        let mut views = Vec::new();
        // `env_derivation` will generate a local variable with `Env` for the original function
        let mut env_derivation = None;
        // `original_fn_args` will generate arguments for calling original method implementation
        let mut original_fn_args = Vec::new();

        // Optional state argument
        if let Some(mutability) = self.state {
            internal_args_pointers.push(quote! {
                pub self_ptr: ::core::ptr::NonNull<
                    <#self_type as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                /// Size of the contents `self_ptr` points to
                pub self_size: ::core::primitive::u32,
                /// Capacity of the allocated memory `self_ptr` points to
                pub self_capacity: ::core::primitive::u32,
            });

            if mutability.is_some() {
                original_fn_args.push(quote! {&mut *{
                    // Ensure the state type implements `IoType`, which is required for crossing the
                    // host/guest boundary
                    const {
                        const fn assert_impl_io_type<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoType,
                        {}
                        assert_impl_io_type::<#self_type>();
                    }

                    <#self_type as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                        &mut args.self_ptr,
                        &mut args.self_size,
                        args.self_capacity,
                    )
                }});
            } else {
                original_fn_args.push(quote! {&*{
                    // Ensure the state type implements `IoType`, which is required for crossing the
                    // host/guest boundary
                    const {
                        const fn assert_impl_io_type<T>()
                        where
                            T: ::ab_contracts_macros::__private::IoType,
                        {}
                        assert_impl_io_type::<#self_type>();
                    }

                    <#self_type as ::ab_contracts_macros::__private::IoType>::from_ptr(
                        &args.self_ptr,
                        &args.self_size,
                        args.self_capacity,
                    )
                }});
            }
        }

        // Optional environment argument
        if let Some(env) = &self.env {
            let env_field = &env.arg_name;
            let mutability = env.mutability;
            let env_local = format_ident!("macro_{env_field}");

            internal_args_pointers.push(quote! {
                // Use `Env` to check if the method argument had the correct type at compile time
                pub #env_field: &'internal_args #mutability ::ab_contracts_macros::__private::Env<'internal_args>,
            });

            // The lifetime of `Env` is inferred to cover the call, so that the method can declare
            // its outputs with it to lend them to calls (see `Env::lend_output()`). It is bounded
            // by the lifetime of the reference in `args`, otherwise a method that declares `Env`
            // with `'static` would get it with that lifetime and could keep it after the call.
            let (from_reference, as_reference) = if mutability.is_some() {
                (format_ident!("from_mut"), format_ident!("as_mut"))
            } else {
                (format_ident!("from_ref"), format_ident!("as_ref"))
            };
            env_derivation.replace(quote! {
                /// Shorten the lifetime of `Env` to that of the reference.
                ///
                /// # Safety
                /// The `Env` behind `env` must not be used after `'call` other than being dropped,
                /// since an `Env` of lifetime `'call` can be written into it through an exclusive
                /// reference. Memory borrowed for `'call` by code that uses the returned `Env` must
                /// not be accessed after `'call` other than by parsing it, unless its contents
                /// accept every bit pattern (see `Env::with_executor_context()`).
                // TODO: Drop the first requirement once `Env` is immovable with the `Move` trait
                //  (https://github.com/rust-lang/rust/issues/149607): no `Env` could be written
                //  into the one behind `env` then, so the type system would guarantee that it
                //  keeps its lifetime
                #[inline(always)]
                unsafe fn macro_shorten_env<'call>(
                    env: &'call #mutability ::ab_contracts_macros::__private::Env<'_>,
                ) -> &'call #mutability ::ab_contracts_macros::__private::Env<'call> {
                    // SAFETY: The pointer comes from a reference to a valid `Env` and is valid for
                    // `'call`, only the lifetime of `Env` is shortened, the rest is guaranteed by
                    // the function contract
                    unsafe {
                        ::core::ptr::NonNull::#from_reference(env)
                            .cast::<::ab_contracts_macros::__private::Env<'call>>()
                            .#as_reference()
                    }
                }

                // SAFETY: This meets both requirements of `macro_shorten_env()`. First, the method
                // may write an `Env` of the shorter lifetime into the one it gets. The executor
                // doesn't use that `Env` after the call other than to drop it
                // (`Env::with_executor_context()` contract), and this function doesn't use it
                // either. Second, the method may mutably borrow memory for the shorter lifetime and
                // lend it to calls. Named lifetimes in the types of the state, `#[tmp]`, `#[slot]`
                // and the last `#[output]` of `#[init]`, which is the initial state, are rejected,
                // including those of containers like `MaybeData<'a, _>`. So this memory is one of
                // its other `#[output]`s or memory that outlives this function. The method can only
                // reach the latter through references it owns, which are gone once it returns, so
                // afterward that memory is only accessible through the `Lent` it was lent in, which
                // parses it first. After the call, the caller of this function only accesses the
                // contents of outputs by parsing them, unless every bit pattern is valid
                // (`# Safety` of this function).
                let #env_local = unsafe { macro_shorten_env(args.#env_field) };
            });

            original_fn_args.push(quote! { #env_local });
        }

        // Optional tmp argument
        if let Some(tmp) = &self.tmp {
            // `'static` only names the type, the value the method gets has the lifetime of the
            // borrow of `args` (see `MethodDetails::tmp_type()`)
            let type_name = with_lifetime(&tmp.type_name, "'static");
            let mutability = tmp.mutability;
            let ptr_field = format_ident!("{}_ptr", tmp.arg_name);
            let size_field = format_ident!("{}_size", tmp.arg_name);
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", tmp.arg_name);
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");
            let tmp_local = format_ident!("macro_{}", tmp.arg_name);

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <
                        // Make sure `#[tmp]` type matches expected type
                        <#self_type as ::ab_contracts_macros::__private::Contract>::Tmp as ::ab_contracts_macros::__private::IoType
                    >::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: ::core::primitive::u32,
                #[doc = #capacity_doc]
                pub #capacity_field: ::core::primitive::u32,
            });

            let view = if mutability.is_some() {
                quote! {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                        &mut args.#ptr_field,
                        &mut args.#size_field,
                        args.#capacity_field,
                    )
                }
            } else {
                quote! {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_ptr(
                        &args.#ptr_field,
                        &args.#size_field,
                        args.#capacity_field,
                    )
                }
            };
            // A local variable rather than a temporary, so that it outlives the block that binds
            // it to the type of the argument (see `bind_to_type()`)
            views.push(quote! {let #mutability #tmp_local = {
                // Ensure tmp type implements `IoTypeOptional`, which is required for handling of
                // tmp that might be removed or not present and implies implementation of `IoType`,
                // which is required for crossing the host/guest boundary
                const {
                    const fn assert_impl_io_type_optional<T>()
                    where
                        T: ::ab_contracts_macros::__private::IoTypeOptional,
                    {}
                    assert_impl_io_type_optional::<#type_name>();
                }

                #view
            };});
            original_fn_args.push(bind_to_type(
                &tmp.type_name,
                mutability,
                quote! { &#mutability *#tmp_local },
            ));
        }

        // Slot arguments with:
        // * in case address is used: a pointer to address, a pointer to slot and size + capacity
        // * in case address is not used: a pointer to slot and size + capacity
        for slot in &self.slots {
            // `'static` only names the type, like for `#[tmp]` above
            let type_name = with_lifetime(&slot.type_name, "'static");
            let mutability = slot.mutability;
            let address_field = format_ident!("{}_address", slot.arg_name);
            let ptr_field = format_ident!("{}_ptr", slot.arg_name);
            let size_field = format_ident!("{}_size", slot.arg_name);
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{}_capacity", slot.arg_name);
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");
            let slot_local = format_ident!("macro_{}", slot.arg_name);

            internal_args_pointers.push(quote! {
                // Use `Address` to check if the method argument had the correct type at compile
                // time
                pub #address_field: &'internal_args ::ab_contracts_macros::__private::Address,
                pub #ptr_field: ::core::ptr::NonNull<
                    <
                        // Make sure the `#[slot]` type matches the expected type
                        <#self_type as ::ab_contracts_macros::__private::Contract>::Slot as ::ab_contracts_macros::__private::IoType
                    >::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: ::core::primitive::u32,
                #[doc = #capacity_doc]
                pub #capacity_field: ::core::primitive::u32,
            });

            let view = if mutability.is_some() {
                quote! {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_mut_ptr(
                        &mut args.#ptr_field,
                        &mut args.#size_field,
                        args.#capacity_field,
                    )
                }
            } else {
                quote! {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::from_ptr(
                        &args.#ptr_field,
                        &args.#size_field,
                        args.#capacity_field,
                    )
                }
            };
            // A local variable rather than a temporary, like for `#[tmp]` above
            views.push(quote! {let #mutability #slot_local = {
                // Ensure the slot type implements `IoTypeOptional`, which is required for handling
                // of slot that might be removed or not present and implies implementation of
                // `IoType`, which is required for crossing the host/guest boundary
                const {
                    const fn assert_impl_io_type_optional<T>()
                    where
                        T: ::ab_contracts_macros::__private::IoTypeOptional,
                    {}
                    assert_impl_io_type_optional::<#type_name>();
                }

                #view
            };});
            let arg_extraction = bind_to_type(
                &slot.type_name,
                mutability,
                quote! { &#mutability *#slot_local },
            );

            if slot.with_address_arg {
                original_fn_args.push(quote! {
                    (
                        args.#address_field,
                        #arg_extraction,
                    )
                });
            } else {
                original_fn_args.push(arg_extraction);
            }
        }

        // Inputs
        for input in &self.inputs {
            // `'static` only names the type, like for `#[tmp]` above
            let type_name = with_lifetime(&input.type_name, "'static");
            let arg_name = &input.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{arg_name}_capacity");
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");
            let input_local = format_ident!("macro_{arg_name}");

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: ::core::primitive::u32,
                #[doc = #capacity_doc]
                pub #capacity_field: ::core::primitive::u32,
            });

            // A local variable rather than a temporary, like for `#[tmp]` above
            views.push(quote! {let #input_local = {
                // Ensure the input type implements `IoType`, which is required for crossing the
                // host/guest boundary
                const {
                    const fn assert_impl_io_type<T>()
                    where
                        T: ::ab_contracts_macros::__private::IoType,
                    {}
                    assert_impl_io_type::<#type_name>();
                }

                // The caller provides inputs, so their contents are parsed
                // SAFETY: The pointer is aligned for the type's `PointerType`, the size doesn't
                // exceed the capacity and both are valid for the type, and the pointer is valid
                // for reads of the size, with that many bytes initialized and not written to
                // before this function returns, as required by the `# Safety` section of this
                // function
                match unsafe {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::try_from_ptr(
                        &args.#ptr_field,
                        &args.#size_field,
                        args.#capacity_field,
                    )
                } {
                    ::core::option::Option::Some(input) => input,
                    ::core::option::Option::None => {
                        return ::ab_contracts_macros::__private::ContractError::BadInput
                            .exit_code();
                    }
                }
            };});
            original_fn_args.push(bind_to_type(
                &input.type_name,
                None,
                quote! { &*#input_local },
            ));
        }

        // Outputs
        for output in &self.outputs {
            // `'static` only names the type, like for `#[tmp]` above
            let type_name = with_lifetime(&output.type_name, "'static");
            let arg_name = &output.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{arg_name}_capacity");
            let capacity_doc = format!("Capacity of the allocated memory `{ptr_field}` points to");
            let output_local = format_ident!("macro_{arg_name}");

            internal_args_pointers.push(quote! {
                pub #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                pub #size_field: ::core::primitive::u32,
                #[doc = #capacity_doc]
                pub #capacity_field: ::core::primitive::u32,
            });

            // A local variable rather than a temporary, so that the method can borrow the output
            // for the lifetime of `Env`
            views.push(quote! {let mut #output_local = {
                // Ensure the output type implements `IoType`, which is required for crossing the
                // host/guest boundary
                const {
                    const fn assert_impl_io_type<T>()
                    where
                        T: ::ab_contracts_macros::__private::IoType,
                    {}
                    assert_impl_io_type::<#type_name>();
                }

                // The caller provides initial contents of outputs, so they are parsed
                // SAFETY: The pointer is aligned for the type's `PointerType`, the size doesn't
                // exceed the capacity and both are valid for the type, and the pointer is valid
                // for writes of the capacity and for reads of the size, with that many bytes
                // initialized, and the memory is not accessed other than through this pointer
                // before this function returns, as required by the `# Safety` section of this
                // function. The returned value and a container that the method moves out of it
                // borrow `args`, and the borrow ends before this function returns: the method
                // returns a `TrivialType`, which can't hold a borrow, and statics require
                // `'static`, so only locals of the method or of this function can hold them.
                match unsafe {
                    <#type_name as ::ab_contracts_macros::__private::IoType>::try_from_mut_ptr(
                        &mut args.#ptr_field,
                        &mut args.#size_field,
                        args.#capacity_field,
                    )
                } {
                    ::core::option::Option::Some(output) => output,
                    ::core::option::Option::None => {
                        return ::ab_contracts_macros::__private::ContractError::BadInput
                            .exit_code();
                    }
                }
            };});
            original_fn_args.push(bind_to_type(
                &output.type_name,
                Some(<Token![mut]>::default()),
                quote! { &mut *#output_local },
            ));
        }

        // The host stores the last `#[output]` of `#[init]` without a return value as the state of
        // the contract, so it must be exactly `MaybeData<'_, Self>` and not, for example, an alias
        // of a container of multiple values
        if matches!(self.method_type, MethodType::Init)
            && self.return_type.unit_return_type()
            && let Some(state_output) = self.outputs.last()
        {
            let type_name = with_lifetime(&state_output.type_name, "'static");
            preparation.push(quote_spanned! {type_name.span() =>
                const {
                    let _: ::core::marker::PhantomData<
                        ::ab_contracts_macros::__private::MaybeData<'static, #self_type>,
                    > = ::core::marker::PhantomData::<#type_name>;
                }
            });
        }

        let original_method_name = &fn_sig.ident;
        let ffi_fn_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;
        let return_type = self.return_type.return_type();

        let internal_args_struct = {
            // No special handling of the return type is needed for a unit return type
            if !self.return_type.unit_return_type() {
                internal_args_pointers.push(quote! {
                    pub ok_result: &'internal_args mut ::core::mem::MaybeUninit<#return_type>,
                });

                preparation.push(quote_spanned! {return_type.span() =>
                    // Ensure the return type implements not only `IoType`, which is required for
                    // crossing host/guest boundary, but also `TrivialType` and result handling is
                    // trivial without the need to worry about size and capacity.
                    // `#[output]` must be used for a variable size result.
                    const {
                        const fn assert_impl_trivial_type<T>()
                        where
                            T: ::ab_contracts_macros::__private::TrivialType,
                        {}
                        assert_impl_trivial_type::<#return_type>();
                    }
                });
            }
            let args_struct_doc = format!(
                "Data structure containing expected input to [`{ffi_fn_name}()`], it is used \
                internally by the contract, there should be no need to construct it explicitly \
                except maybe in contract's own tests"
            );
            // A method without arguments and return value has no public fields in `InternalArgs`,
            // so `clippy::partial_pub_fields` doesn't fire there. The expectation keeps the span of
            // the signature, otherwise rustc doesn't report it when it is unfulfilled.
            let partial_pub_fields_expectation = (!internal_args_pointers.is_empty()).then(|| {
                quote_spanned! {fn_sig.span() =>
                    #[expect(
                        clippy::partial_pub_fields,
                        reason = "Not needed to be constructed publicly, but does need to always \
                        store the lifetime"
                    )]
                }
            });
            quote_spanned! {fn_sig.span() =>
                #[doc = #args_struct_doc]
                #[derive(::core::fmt::Debug)]
                #[repr(C)]
                #partial_pub_fields_expectation
                #[allow(clippy::pub_underscore_fields, reason = "Comes from user-provided arguments")]
                pub struct InternalArgs<'internal_args>
                {
                    #( #internal_args_pointers )*
                    lifetime: ::core::marker::PhantomData<&'internal_args ()>,
                }
            }
        };

        let result_var_name = format_ident!("result");
        let guest_fn = {
            // Depending on whether `T` or `Result<T, ContractError>` is used as a return type,
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
                        args.ok_result.write(#result_var_name);
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
                                args.ok_result.write(result);
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

            // A method without arguments and return value doesn't read `InternalArgs`, so the
            // argument isn't bound to a variable then
            let args_pattern = if internal_args_pointers.is_empty() {
                quote_spanned! {fn_sig.span() => _ }
            } else {
                quote_spanned! {fn_sig.span() => args }
            };

            // Generate FFI function with the original name
            quote_spanned! {fn_sig.span() =>
                /// FFI interface into a method, called by the host.
                ///
                /// NOTE: Calling this function directly shouldn't be necessary except maybe in
                /// contract's own tests.
                ///
                /// Contents of `#[input]` and `#[output]` arguments are parsed, the exit code of
                /// `ContractError::BadInput` is returned if they are not valid values of their
                /// types.
                ///
                /// # Safety
                ///
                /// Caller must ensure the provided pointer corresponds to the expected ABI.
                /// Sizes, capacities and alignment of inputs and outputs match the method's
                /// metadata, which the executor guarantees (the RISC-V VM checks them, the
                /// native executor trusts callers and only catches mismatches with debug
                /// assertions): pointers are aligned for the `IoType::PointerType` of their
                /// types, and sizes and capacities are valid for their types, as
                /// `IoType::try_from_ptr()` requires for inputs and `IoType::try_from_mut_ptr()`
                /// for outputs. The pointer of an input is valid for reads of its size, with that
                /// many bytes initialized and not written to during the call. The pointer of an
                /// output is valid for writes of its capacity and for reads of its size, with that
                /// many bytes initialized, and the memory is not accessed other than through this
                /// pointer during the call.
                ///
                /// The state, `#[tmp]` and `#[slot]` arguments are not parsed: their pointers
                /// are aligned and point to valid values of their types with valid sizes and
                /// capacities, accessed like inputs if they are read-only and like outputs
                /// otherwise.
                ///
                /// The `Env` behind `#[env]` (if present) is created as
                /// `Env::with_executor_context()` requires for an `Env` given to code by reference,
                /// or by the host in `guest` builds. The method gets it with a lifetime that only
                /// covers the call. For that lifetime, it can borrow its `#[output]`s other than
                /// the state and lend them to calls (see `Env::lend_output()`), which can leave
                /// contents behind that are not valid values. Hence, after the call, the contents
                /// of outputs must not be accessed as values of their types without being parsed
                /// first, unless every bit pattern is a valid value. `OutputArg` ensures this for
                /// callers of extension methods. The exception is the last `#[output]` of `#[init]`
                /// without a return value, which is the initial state. The method can't lend it
                /// with `Env::lend_output()`, since `#[contract]` rejects a named lifetime on it,
                /// so after a successful call it is either empty or holds a valid value of the
                /// state.
                #[cfg_attr(feature = "guest", unsafe(no_mangle))]
                #[allow(clippy::new_ret_no_self, reason = "Method was re-written for FFI purposes without `Self`")]
                #[allow(clippy::absurd_extreme_comparisons, reason = "Macro-generated code doesn't know the size upfront")]
                pub unsafe extern "C" fn #ffi_fn_name<'args>(
                    #args_pattern: &'args mut InternalArgs<'_>,
                ) -> ::ab_contracts_macros::__private::ExitCode {
                    #( #preparation )*
                    #( #views )*
                    #env_derivation

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
        };

        let fn_pointer_static = {
            let adapter_ffi_fn_name = format_ident!("{ffi_fn_name}_adapter");
            let args_struct_name =
                derive_external_args_struct_name(self_type, trait_name, original_method_name)?;

            quote! {
                #[doc(hidden)]
                #[expect(clippy::inline_modules, reason = "Macro-generated")]
                #[expect(clippy::wildcard_imports, reason = "Macro-generated")]
                pub mod fn_pointer {
                    use super::*;

                    unsafe extern "C" fn #adapter_ffi_fn_name(
                        args_ptr: ::core::ptr::NonNull<::core::ffi::c_void>,
                    ) -> ::ab_contracts_macros::__private::ExitCode {
                        // SAFETY: Caller must ensure correct ABI of the void pointer, little can be
                        // done here
                        unsafe {
                            let mut args = args_ptr.cast::<InternalArgs<'_>>();
                            debug_assert!(args.is_aligned(), "`args` pointer is misaligned");
                            let args = args.as_mut();
                            #ffi_fn_name(args)
                        }
                    }

                    pub const METHOD_FN_POINTER: ::ab_contracts_macros::__private::NativeExecutorContactMethod =
                        ::ab_contracts_macros::__private::NativeExecutorContactMethod {
                            method_fingerprint: &<#args_struct_name<'_> as ::ab_contracts_macros::__private::ExternalArgs>::FINGERPRINT,
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
        // Statements in `::new()` method before `Self` is created
        let mut method_preparation = Vec::new();
        // Fields set on `Self` in `::new()` method
        let mut method_args_fields = Vec::new();
        // Getters for fields that the caller needs to read after the method call
        let mut getters = Vec::new();
        // Lifetimes of outputs that `::new()` declares
        let mut output_lifetimes = Vec::new();

        // For slots in external args only the address pointer is needed
        for slot in &self.slots {
            let arg_name = &slot.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let ptr_doc = format!("Pointer to the address of `{arg_name}` slot");

            external_args_fields.push(quote! {
                #[doc = #ptr_doc]
                #ptr_field: ::core::ptr::NonNull<::ab_contracts_macros::__private::Address>,
            });

            method_args.push(quote! {
                #arg_name: &'external_args ::ab_contracts_macros::__private::Address,
            });
            method_args_fields.push(quote! {
                #ptr_field: ::core::ptr::NonNull::from_ref(#arg_name),
            });
        }

        // Inputs with pointers to data and size
        for input in &self.inputs {
            let type_name = with_lifetime(&input.type_name, "'static");
            let arg_type_name = with_lifetime(&input.type_name, "'_");
            let arg_name = &input.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let ptr_doc = format!("Pointer to the contents of `{arg_name}` input");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let capacity_field = format_ident!("{arg_name}_capacity");
            let capacity_doc = format!("Capacity of the contents `{ptr_field}` points to");

            external_args_fields.push(quote! {
                #[doc = #ptr_doc]
                #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                #size_field: ::core::primitive::u32,
                #[doc = #capacity_doc]
                #capacity_field: ::core::primitive::u32,
            });

            method_args.push(quote! {
                #arg_name: &'external_args #arg_type_name,
            });
            method_args_fields.push(quote! {
                // This pointer is used as input to FFI call, and underlying data will not be
                // modified, also the pointer will not outlive the reference from which it was
                // created despite copying
                #ptr_field: ::ab_contracts_macros::__private::IoType::as_ptr(#arg_name),
                #size_field: ::ab_contracts_macros::__private::IoType::size(#arg_name),
                #capacity_field: ::ab_contracts_macros::__private::IoType::capacity(#arg_name),
            });
        }

        // Outputs with pointers to data, size and capacity
        let mut outputs_iter = self.outputs.iter().peekable();
        while let Some(output) = outputs_iter.next() {
            let type_name = with_lifetime(&output.type_name, "'static");
            let arg_name = &output.arg_name;
            let ptr_field = format_ident!("{arg_name}_ptr");
            let ptr_doc = format!("Pointer to the contents of `{arg_name}` output");
            let size_field = format_ident!("{arg_name}_size");
            let size_doc = format!("Size of the contents `{ptr_field}` points to");
            let size_getter_doc =
                format!("Size of `{arg_name}` output, updated by the host after the method call");
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
            let (arg_type_name, lifetime) = output.output_arg_type();
            output_lifetimes.extend(lifetime);

            external_args_fields.push(quote! {
                #[doc = #ptr_doc]
                #ptr_field: ::core::ptr::NonNull<
                    <#type_name as ::ab_contracts_macros::__private::IoType>::PointerType,
                >,
                #[doc = #size_doc]
                #size_field: ::core::primitive::u32,
                #[doc = #capacity_doc]
                #capacity_field: ::core::primitive::u32,
            });

            method_args.push(quote! {
                #arg_name: &'external_args mut impl ::ab_contracts_macros::__private::OutputArg<
                    #arg_type_name,
                >,
            });
            getters.push(quote! {
                #[doc = #size_getter_doc]
                #[inline(always)]
                #[must_use]
                pub fn #size_field(&self) -> ::core::primitive::u32 {
                    self.#size_field
                }
            });
            // The memory is only accessed through the pointer while the output stays exclusively
            // borrowed, for the lifetime of the struct
            method_preparation.push(quote! {
                let (#ptr_field, #size_field, #capacity_field) =
                    ::ab_contracts_macros::__private::OutputArg::<#arg_type_name>::as_raw_output(
                        #arg_name,
                    );
            });
            method_args_fields.push(quote! {
                #ptr_field,
                #size_field,
                #capacity_field,
            });
        }

        let ffi_fn_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;

        // Initializer's return type will be `()` for the caller of `#[init]` since the state is
        // stored by the host and not returned to the caller and explicit argument is not needed in
        // the `ExternalArgs` struct. Similarly, it is skipped for a unit return type.
        if !(matches!(self.method_type, MethodType::Init) || self.return_type.unit_return_type()) {
            let return_type = &self.return_type.return_type();

            external_args_fields.push(quote! {
                /// Return value written by the host
                ok_result: &'external_args mut ::core::mem::MaybeUninit<#return_type>,
            });

            method_args.push(quote! {
                ok_result: &'external_args mut ::core::mem::MaybeUninit<#return_type>,
            });
            method_args_fields.push(quote! {
                ok_result,
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
            // Fields are private and can only be set by `Self::new()`, so that pointers in them
            // always match the method signature, according to which the host dereferences them
            #[derive(::core::fmt::Debug)]
            #[repr(C)]
            #[allow(rustdoc::redundant_explicit_links, reason = "Macro-generated")]
            pub struct #args_struct_name<'external_args> {
                #( #external_args_fields )*
                /// Lifetime of the struct
                _lifetime: ::core::marker::PhantomData<&'external_args ()>,
            }

            #[automatically_derived]
            unsafe impl ::ab_contracts_macros::__private::ExternalArgs for #args_struct_name<'_> {
                const FINGERPRINT: ::ab_contracts_macros::__private::MethodFingerprint =
                    ::ab_contracts_macros::__private::MethodFingerprint::new(METADATA)
                        .expect(
                            "Generated metadata is correct and its types are not nested deeper \
                            than `MAX_METADATA_DEPTH`; qed"
                        );
                const METADATA: &'static [::core::primitive::u8] = METADATA;
            }

            impl<'external_args> #args_struct_name<'external_args> {
                /// Create a new instance.
                ///
                /// A call can leave contents behind in outputs that are not valid values of their
                /// types, even when it succeeds. Hence outputs are given as `OutputArg`, which
                /// makes sure contents are parsed before they are accessed, unless every bit
                /// pattern is a valid value. After a successful call, query updated sizes of
                /// outputs with `*_size()` getters and set them with
                /// `OutputArg::set_output_size()`. The return value (if there is one) is
                /// initialized after a successful call, but it is not necessarily a valid value of
                /// its type and must be parsed, for example with `IoType::try_from_ptr()`.
                #[allow(
                    clippy::new_without_default,
                    reason = "Do not want `Default` in auto-generated code"
                )]
                #[allow(clippy::used_underscore_binding, reason = "Macro-generated")]
                #[allow(
                    clippy::impl_trait_in_params,
                    reason = "Macro-generated, a generic parameter per output is harder to \
                    generate and reads worse in documentation"
                )]
                #[allow(
                    clippy::too_many_arguments,
                    reason = "Macro-generated, one argument per argument of the method"
                )]
                pub fn new<#( #output_lifetimes ),*>(
                    #( #method_args )*
                ) -> Self {
                    #( #method_preparation )*

                    Self {
                        #( #method_args_fields )*
                        _lifetime: ::core::marker::PhantomData,
                    }
                }

                #( #getters )*
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
        // Assertions for types whose metadata is included in `method_metadata`
        let mut max_depth_assertions = Vec::new();

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
            let type_name = with_lifetime(&input.type_name, "'static");

            method_metadata.push(quote! {
                &[::ab_contracts_macros::__private::ContractMetadataKind::#io_metadata_type as ::core::primitive::u8],
                #arg_name_metadata,
                <#type_name as ::ab_contracts_macros::__private::IoType>::METADATA,
            });
            max_depth_assertions.push(max_depth_assertion(
                &type_name,
                "`#[input]` type must not be nested deeper than `MAX_METADATA_DEPTH`",
            ));
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
                let type_name = with_lifetime(&output.type_name, "'static");
                max_depth_assertions.push(max_depth_assertion(
                    &type_name,
                    "`#[output]` type must not be nested deeper than `MAX_METADATA_DEPTH`",
                ));
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
                max_depth_assertions.push(max_depth_assertion(
                    return_type,
                    "Return type must not be nested deeper than `MAX_METADATA_DEPTH`",
                ));
                Some(quote! {
                    <#return_type as ::ab_contracts_macros::__private::IoType>::METADATA,
                })
            };
            method_metadata.push(quote! {
                &[
                    ::ab_contracts_macros::__private::ContractMetadataKind::Return as ::core::primitive::u8,
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
        // `self` is not in metadata, but counts towards `MAX_TOTAL_METHOD_ARGS` in methods with it
        let total_number_of_arguments = Literal::u8_unsuffixed(
            number_of_arguments.saturating_add(u8::from(self.state.is_some())),
        );
        let number_of_arguments = Literal::u8_unsuffixed(number_of_arguments);

        let original_method_name = &fn_sig.ident;
        let ffi_fn_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;
        let method_name_metadata = derive_ident_metadata(&ffi_fn_name)?;
        Ok(quote_spanned! {fn_sig.span() =>
            #[expect(
                clippy::assertions_on_constants,
                reason = "Auto-generated compile-time check"
            )]
            const fn metadata()
                -> ([::core::primitive::u8; ::ab_contracts_macros::__private::MAX_METADATA_CAPACITY], usize)
            {
                assert!(
                    #total_number_of_arguments <= ::ab_contracts_macros::__private::MAX_TOTAL_METHOD_ARGS,
                    "Too many arguments"
                );
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
            pub const METADATA: &[::core::primitive::u8] = {
                // The reference extends the lifetime of metadata scratch to that of the constant
                let (metadata_scratch, metadata_size) = &metadata();
                <[_]>::split_at(metadata_scratch, *metadata_size).0
            };

            // Contracts decode metadata of these types with `MAX_METADATA_DEPTH` as the max
            // nesting depth, so deeper types must not compile
            const _: () = {
                #( #max_depth_assertions )*
            };
        })
    }

    pub(super) fn generate_trait_ext_components(
        &self,
        fn_sig: &Signature,
        fn_attrs: &[Attribute],
        trait_name: Option<&Ident>,
    ) -> Result<ExtTraitComponents, Error> {
        let self_type = &self.self_type;

        let mut method_args = Vec::new();
        let mut external_args_args = Vec::new();
        let mut result_processing_before = Vec::new();
        // Sizes of outputs are set before the return value is processed, so they match what the
        // callee left in outputs even when the return value is rejected
        let mut size_write_backs = Vec::new();
        // Lifetimes of outputs that the method declares
        let mut output_lifetimes = Vec::new();

        // Address of the contract
        method_args.push(quote_spanned! {fn_sig.span() =>
            contract: ::ab_contracts_macros::__private::Address,
        });

        // For each slot argument generate an address argument
        for slot in &self.slots {
            let arg_name = &slot.arg_name;

            method_args.push(quote_spanned! {fn_sig.span() =>
                #arg_name: &::ab_contracts_macros::__private::Address,
            });
            external_args_args.push(quote_spanned! {fn_sig.span() => #arg_name });
        }

        // For each input argument, generate a corresponding read-only argument
        for input in &self.inputs {
            let type_name = with_lifetime(&input.type_name, "'_");
            let arg_name = &input.arg_name;

            method_args.push(quote_spanned! {fn_sig.span() =>
                #arg_name: &#type_name,
            });
            external_args_args.push(quote_spanned! {fn_sig.span() => #arg_name });
        }

        // For each output argument, generate a corresponding read-write argument
        let mut outputs_iter = self.outputs.iter().peekable();
        while let Some(output) = outputs_iter.next() {
            let arg_name = &output.arg_name;
            let size_var = format_ident!("macro_{arg_name}_size");
            let size_field = format_ident!("{arg_name}_size");

            // Initializer's return type will be `()` for the caller of `#[init]`, state is stored
            // by the host and not returned to the caller
            if outputs_iter.is_empty()
                && self.return_type.unit_return_type()
                && matches!(self.method_type, MethodType::Init)
            {
                continue;
            }
            let (type_name, lifetime) = output.output_arg_type();
            output_lifetimes.extend(lifetime);

            // The bound has the span of the type, so that an error caused by it, like for an alias
            // that hides `'static` in a container, points to the type rather than to the method
            method_args.push(quote_spanned! {type_name.span() =>
                #arg_name: &mut impl ::ab_contracts_macros::__private::OutputArg<#type_name>,
            });
            external_args_args.push(quote_spanned! {fn_sig.span() => #arg_name });
            result_processing_before.push(quote_spanned! {fn_sig.span() =>
                let #size_var = args.#size_field();
            });
            size_write_backs.push(quote_spanned! {fn_sig.span() =>
                // SAFETY: The executor keeps the size of the output valid for its type and within
                // its capacity, with that many bytes initialized (`ExecutorContext` contract,
                // upheld by the host for guests)
                unsafe {
                    ::ab_contracts_macros::__private::OutputArg::<#type_name>::set_output_size(
                        #arg_name,
                        #size_var,
                    );
                }
            });
        }

        let original_method_name = &fn_sig.ident;
        let ext_method_name = derive_ffi_fn_name(self_type, trait_name, original_method_name)?;
        // Non-`#[view]` methods can only be called on `&mut Env`
        let env_self = if matches!(self.method_type, MethodType::View) {
            quote_spanned! {fn_sig.span() => &self }
        } else {
            quote_spanned! {fn_sig.span() => &mut self }
        };
        // `#[view]` methods do not require explicit method context
        let method_context_arg = (!matches!(self.method_type, MethodType::View)).then(|| {
            quote_spanned! {fn_sig.span() =>
                method_context: ::ab_contracts_macros::__private::MethodContext,
            }
        });
        let method_args_attrs = quote_spanned! {fn_sig.span() =>
            #[allow(dead_code, reason = "Macro-generated")]
            #[allow(clippy::used_underscore_binding, reason = "Macro-generated")]
            #[allow(
                clippy::impl_trait_in_params,
                reason = "Macro-generated, a generic parameter per output is harder to generate \
                and reads worse in documentation"
            )]
        };
        // Initializer's return type will be `()` for the caller of `#[init]` since the state is
        // stored by the host and not returned to the caller. Similarly, it is skipped for a unit
        // return type.
        let return_value = (!(matches!(self.method_type, MethodType::Init)
            || self.return_type.unit_return_type()))
        .then(|| self.return_type.return_type());
        let method_signature = if let Some(return_type) = return_value {
            quote_spanned! {fn_sig.span() =>
                #method_args_attrs
                fn #ext_method_name<#( #output_lifetimes ),*>(
                    #env_self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<
                    #return_type,
                    ::ab_contracts_macros::__private::ContractError,
                >
            }
        } else {
            quote_spanned! {fn_sig.span() =>
                #method_args_attrs
                #[allow(clippy::semicolon_if_nothing_returned, reason = "Macro-generated")]
                fn #ext_method_name<#( #output_lifetimes ),*>(
                    #env_self,
                    #method_context_arg
                    #( #method_args )*
                ) -> ::core::result::Result<(), ::ab_contracts_macros::__private::ContractError>
            }
        };

        let attrs = fn_attrs.iter().filter(|attr| {
            let path = match &attr.meta {
                Meta::Path(path) => path,
                Meta::List(list) => &list.path,
                Meta::NameValue(name_value) => &name_value.path,
            };

            if let Some(ident) = path.get_ident() {
                ident == "doc" || ident == "allow" || ident == "expect"
            } else {
                false
            }
        });
        let definition = quote_spanned! {fn_sig.span() =>
            #[allow(
                clippy::too_many_arguments,
                reason = "Generated code may have more arguments that source code"
            )]
            #( #attrs )*
            #method_signature;
        };

        let args_struct_name =
            derive_external_args_struct_name(self_type, trait_name, original_method_name)?;
        // `#[view]` methods do not require explicit method context
        let method_context_value = if matches!(self.method_type, MethodType::View) {
            quote_spanned! {fn_sig.span() =>
                ::ab_contracts_macros::__private::MethodContext::Reset
            }
        } else {
            quote_spanned! {fn_sig.span() =>
                method_context
            }
        };
        let ok_result_declaration = if return_value.is_some() {
            external_args_args.push(quote_spanned! {fn_sig.span() =>
                &mut ok_result
            });
            Some(quote_spanned! {fn_sig.span() =>
                let mut ok_result = ::core::mem::MaybeUninit::uninit();
            })
        } else {
            None
        };
        let call = quote_spanned! {fn_sig.span() =>
            #ok_result_declaration

            let mut args = #original_method_name::#args_struct_name::new(
                #( #external_args_args, )*
            );

            self.call(contract, &mut args, #method_context_value)?;

            #( #result_processing_before )*
            #( #size_write_backs )*
        };

        let Some(return_type) = return_value else {
            let r#impl = quote_spanned! {fn_sig.span() =>
                #[inline]
                #method_signature {
                    #call

                    Ok(())
                }
            };

            return Ok(ExtTraitComponents { definition, r#impl });
        };

        let r#impl = quote_spanned! {fn_sig.span() =>
            #[inline]
            #method_signature {
                // Ensure the return type implements not only `IoType`, which is required for
                // crossing host/guest boundary, but also `TrivialType` and result handling is
                // trivial without the need to worry about size and capacity.
                // `#[output]` must be used for a variable size result.
                const {
                    const fn assert_impl_trivial_type<T>()
                    where
                        T: ::ab_contracts_macros::__private::TrivialType,
                    {}
                    assert_impl_trivial_type::<#return_type>();
                }

                #call

                // The callee isn't trusted to write a valid value, so the return value is parsed
                let ok_result_ptr = ::core::ptr::NonNull::from_ref(&ok_result).cast::<
                    <#return_type as ::ab_contracts_macros::__private::IoType>::PointerType,
                >();
                // SAFETY: The pointer is aligned for `PointerType`, which is the return type itself
                // (`TrivialType`), the size and the capacity are its size, and the executor
                // initialized it before returning `Ok(())` (`ExecutorContext` contract, upheld by
                // the host for guests), while nothing writes to it during parsing
                match unsafe {
                    <#return_type as ::ab_contracts_macros::__private::IoType>::try_from_ptr(
                        &ok_result_ptr,
                        &<#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                        <#return_type as ::ab_contracts_macros::__private::TrivialType>::SIZE,
                    )
                } {
                    ::core::option::Option::Some(result) => ::core::result::Result::Ok(*result),
                    ::core::option::Option::None => ::core::result::Result::Err(
                        ::ab_contracts_macros::__private::ContractError::BadOutput,
                    ),
                }
            }
        };

        Ok(ExtTraitComponents { definition, r#impl })
    }
}

/// Returns `lifetime` unless it is elided or `'_`.
///
/// A method that gets `Env<'a>` can lend memory borrowed for `'a` to calls with
/// `Env::lend_output()`, after which its contents must be parsed before they are accessed.
/// Generated code gives a method `Env` with a lifetime that only covers the call, so the method can
/// only borrow arguments for it whose lifetimes it names.
fn named_lifetime(lifetime: Option<&Lifetime>) -> Option<&Lifetime> {
    lifetime.filter(|lifetime| lifetime.ident != "_")
}

/// Returns the first lifetime in `ty` that [`named_lifetime()`] returns.
///
/// Besides the lifetime of a reference, this finds the lifetime of a container, like `'a` in
/// `&mut MaybeData<'a, T>`. A method could move the container out with `mem::replace()` and lend
/// it with `Env<'a>`, just like a reference with a named lifetime.
fn named_lifetime_in_type(ty: &Type) -> Option<&Lifetime> {
    struct FindNamedLifetime<'ast> {
        found: Option<&'ast Lifetime>,
    }

    impl<'ast> Visit<'ast> for FindNamedLifetime<'ast> {
        #[expect(
            clippy::renamed_function_params,
            reason = "`syn` calls it `i`, which says nothing about what it is"
        )]
        fn visit_lifetime(&mut self, lifetime: &'ast Lifetime) {
            if self.found.is_none() {
                self.found = named_lifetime(Some(lifetime));
            }
        }
    }

    let mut visitor = FindNamedLifetime { found: None };
    visitor.visit_type(ty);
    visitor.found
}

/// Bind `value` to a reference to `type_name`, the type of an argument as the method declares it.
///
/// The binding has the span of `type_name`, so errors about the type point to it in the method
/// signature. For example, an alias that hides `'static` in a container gets an error at the alias
/// instead of one for the whole `impl`. The call checks the method signature as before, the binding
/// only adds a check.
fn bind_to_type(
    type_name: &Type,
    mutability: Option<Token![mut]>,
    value: TokenStream,
) -> TokenStream {
    let user_type = with_inferred_lifetimes(type_name);
    quote_spanned! {type_name.span() => {
        let argument: &#mutability #user_type = #value;
        argument
    }}
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
    let ffi_fn_prefix = trait_name.unwrap_or(type_name).to_string().to_snake_case();

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
        method_name.to_string().to_upper_camel_case()
    ))
}
