use anyhow::Context;
use heck::ToSnakeCase;
use quote::{ToTokens, format_ident};
use std::mem;
use std::rc::Rc;
use syn::visit_mut::{self, VisitMut};
use syn::{
    Arm, Attribute, Expr, ExprPath, Fields, FnArg, GenericArgument, Generics, Ident, ItemFn,
    Member, Pat, PatIdent, PatType, Path, PathArguments, QSelf, Signature, Token, Type, TypePath,
    Variant, parse_quote,
};

/// A single named+typed parameter, used both for variant-specific fields and for arguments
/// shared by all generated functions (derived from `execute()`'s own signature)
struct Param<'a> {
    /// Local identifier used both as the parameter name and as the name of the value passed at
    /// the call site
    ident: &'a Ident,
    ty: Type,
}

/// This rewrites every `Self` into a fully qualified concrete instruction enum type instead.
///
/// `execute()`'s own body is written against `Self`, but the functions generated here are
/// standalone functions without `Self`, which needs correction.
struct SelfRewriter<'a> {
    self_ty: &'a Type,
}

impl SelfRewriter<'_> {
    /// If `path` starts with `Self::`, returns the `<self_ty as Instruction>` qualifier and the
    /// remaining path (`Instruction` followed by whatever came after `Self::`)
    fn qualify_self_item(&self, path: &Path) -> Option<(QSelf, Path)> {
        let first = path.segments.first()?;
        if first.ident != "Self" || !first.arguments.is_none() || path.segments.len() < 2 {
            return None;
        }

        let self_ty = &self.self_ty;
        let rest = path.segments.iter().skip(1);
        let TypePath {
            attrs: _,
            qself,
            path,
        } = parse_quote! {
            <#self_ty as Instruction>::#(#rest)::*
        };

        Some((qself.expect("Statically known to be present; qed"), path))
    }
}

impl VisitMut for SelfRewriter<'_> {
    fn visit_type_mut(&mut self, i: &mut Type) {
        if let Type::Path(TypePath {
            qself: None,
            path,
            attrs: _,
        }) = i
        {
            if path.segments.len() == 1 && path.segments[0].ident == "Self" {
                *i = self.self_ty.clone();
            } else if let Some((qself, path)) = self.qualify_self_item(path) {
                *i = Type::Path(TypePath {
                    attrs: Vec::new(),
                    qself: Some(qself),
                    path,
                });
            }
        }
        visit_mut::visit_type_mut(self, i);
    }

    fn visit_expr_mut(&mut self, i: &mut Expr) {
        if let Expr::Path(ExprPath {
            qself: None,
            path,
            attrs,
        }) = i
            && let Some((qself, path)) = self.qualify_self_item(path)
        {
            *i = Expr::Path(ExprPath {
                attrs: mem::take(attrs),
                qself: Some(qself),
                path,
            });
        }
        visit_mut::visit_expr_mut(self, i);
    }
}

/// Extracts the single generic type argument out of a type like `Foo<Bar>`, returning `Bar`.
///
/// This is used to recover field types of destructured arguments of `execute()` (such as
/// `Rs1Rs2OperandValues<T>`) without having access to the original struct definition.
fn extract_single_generic_type(ty: &Type) -> anyhow::Result<&Type> {
    let Type::Path(type_path) = ty else {
        return Err(anyhow::anyhow!(
            "Expected a generic path type for a destructured `execute()` argument, found: {}",
            ty.to_token_stream()
        ));
    };
    let last_segment = type_path
        .path
        .segments
        .last()
        .expect("Path is never empty; qed");
    let PathArguments::AngleBracketed(generic_args) = &last_segment.arguments else {
        return Err(anyhow::anyhow!(
            "Expected a single generic argument in destructured `execute()` argument type `{}`",
            ty.to_token_stream()
        ));
    };
    let mut args = generic_args.args.iter();
    let (Some(GenericArgument::Type(inner_ty)), None) = (args.next(), args.next()) else {
        return Err(anyhow::anyhow!(
            "Expected exactly one generic type argument in destructured `execute()` argument \
            type `{}`",
            ty.to_token_stream()
        ));
    };

    Ok(inner_ty)
}

/// Extracts parameters shared by all generated per-variant functions from `execute()`'s own
/// signature (excluding `self`).
///
/// Arguments (or destructured fields of a struct argument) whose identifier starts with `_` are
/// considered unused and skipped.
fn extract_shared_params(sig: &Signature) -> anyhow::Result<Vec<Param<'_>>> {
    let mut shared_params = Vec::new();

    for input in &sig.inputs {
        let FnArg::Typed(PatType {
            pat,
            ty,
            attrs: _,
            colon_token: _,
        }) = input
        else {
            // `self` receiver is ignored (matched on by `match` and unused after that)
            continue;
        };

        match pat.as_ref() {
            Pat::Ident(PatIdent {
                ident,
                attrs: _,
                by_ref: _,
                mutability: _,
                subpat: _,
            }) => {
                if !ident.to_string().starts_with('_') {
                    shared_params.push(Param {
                        ident,
                        ty: ty.as_ref().clone(),
                    });
                }
            }
            Pat::Struct(pat_struct) => {
                let field_ty = extract_single_generic_type(ty)?;
                for field in &pat_struct.fields {
                    let Pat::Ident(PatIdent {
                        ident,
                        attrs: _,
                        by_ref: _,
                        mutability: _,
                        subpat: _,
                    }) = field.pat.as_ref()
                    else {
                        // `_` or another unsupported nested pattern, treated as unused
                        continue;
                    };
                    if !ident.to_string().starts_with('_') {
                        shared_params.push(Param {
                            ident,
                            ty: field_ty.clone(),
                        });
                    }
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported `execute()` argument pattern `{}`",
                    pat.to_token_stream()
                ));
            }
        }
    }

    Ok(shared_params)
}

/// Extracts variant-specific parameters (fields bound by the match arm's pattern that are not `_`)
/// together with their types looked up from the enum variant's definition
fn extract_variant_params<'a>(
    arm_pat: &'a Pat,
    variant: &'a Variant,
) -> anyhow::Result<Vec<Param<'a>>> {
    let Pat::Struct(pat_struct) = arm_pat else {
        return Err(anyhow::anyhow!(
            "Expected a struct pattern for instruction variant `{}`, found: {}",
            variant.ident,
            arm_pat.to_token_stream()
        ));
    };
    let Fields::Named(fields_named) = &variant.fields else {
        return Err(anyhow::anyhow!(
            "Instruction variant `{}` must have named fields",
            variant.ident
        ));
    };

    let mut params = Vec::new();
    for field_pat in &pat_struct.fields {
        let Pat::Ident(PatIdent {
            ident: local_ident,
            attrs: _,
            by_ref: _,
            mutability: _,
            subpat: _,
        }) = field_pat.pat.as_ref()
        else {
            // `_` or another unsupported nested pattern, treated as unused
            continue;
        };

        let Member::Named(field_name) = &field_pat.member else {
            return Err(anyhow::anyhow!(
                "Instruction variant `{}` fields must be named",
                variant.ident
            ));
        };

        let field = fields_named
            .named
            .iter()
            .find(|field| field.ident.as_ref() == Some(field_name))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Field `{field_name}` not found in instruction variant `{}`",
                    variant.ident
                )
            })?;

        params.push(Param {
            ident: local_ident,
            ty: field.ty.clone(),
        });
    }

    Ok(params)
}

/// Splits a composed `execute()` method into a `#[inline(always)]` function generated for each
/// enum variant, replacing its match arm bodies with calls to those functions.
///
/// `execute()`'s generic parameters are kept in full on every generated function, and call sites
/// always specify them explicitly. This is much simpler than tracking their usage. Same with
/// arguments.
///
/// `variants` and `arms` must have the same length and be in the same order (typically
/// `enum_definition.instructions` and the correspondingly ordered arms extracted from `execute()`).
#[expect(clippy::allow_attributes, reason = "Attribute below")]
#[allow(
    clippy::too_many_arguments,
    reason = "Each parameter is a distinct piece of already-parsed `execute()` context; bundling \
    them would not make this any clearer"
)]
pub(super) fn generate_variant_fns(
    enum_name: &Ident,
    self_ty: &Type,
    generics: &Generics,
    constness: Option<Token![const]>,
    no_panic_attr: Option<&Attribute>,
    execute_sig: &Signature,
    variants: &[Rc<Variant>],
    match_arms: &mut [Arm],
) -> anyhow::Result<Vec<ItemFn>> {
    let mut self_rewriter = SelfRewriter { self_ty };

    let mut shared_params = extract_shared_params(execute_sig)?;
    let mut output = execute_sig.output.clone();
    for param in &mut shared_params {
        self_rewriter.visit_type_mut(&mut param.ty);
    }
    self_rewriter.visit_return_type_mut(&mut output);

    let mut generated_fns = Vec::with_capacity(variants.len());
    let generic_params = &generics.params;
    let where_clause = &generics.where_clause;

    for (variant, arm) in variants.iter().zip(match_arms) {
        let mut variant_params = extract_variant_params(&arm.pat, variant)
            .with_context(|| format!("Instruction variant `{}`", variant.ident))?;
        for param in &mut variant_params {
            self_rewriter.visit_type_mut(&mut param.ty);
        }

        let fn_name = format_ident!(
            "execute_{}_{}",
            enum_name.to_string().to_snake_case(),
            variant.ident.to_string().to_snake_case(),
        );

        let call_args = variant_params
            .iter()
            .chain(&shared_params)
            .map(|param| &param.ident);
        let mut original_arm_body = mem::replace(
            arm.body.as_mut(),
            parse_quote! { #fn_name::<#generic_params>(#( #call_args ),*) },
        );
        arm.comma = Some(<Token![,]>::default());

        self_rewriter.visit_expr_mut(&mut original_arm_body);

        let call_args = variant_params
            .iter()
            .chain(&shared_params)
            .map(|param| &param.ident);
        let args = variant_params
            .iter()
            .chain(&shared_params)
            .map(|Param { ident, ty }| {
                parse_quote! { #ident: #ty }
            })
            .collect::<Vec<FnArg>>();

        generated_fns.push(parse_quote! {
            #[expect(clippy::allow_attributes, reason = "Attributes below")]
            #[allow(
                clippy::extra_unused_type_parameters,
                reason = "Much easier in generated code than properly tracking used generics"
            )]
            #[allow(clippy::too_many_arguments, reason = "Generated and force-inlined")]
            // Comments will be stripped, this will suppress some of the lints that are caused by it
            #[allow(
                clippy::undocumented_unsafe_blocks,
                reason = "Comments will be stripped, this will suppress some of the lints \
                that are caused by it"
            )]
            #no_panic_attr
            #[inline(always)]
            #constness fn #fn_name<#generic_params>(#( #args ),*) #output
                #where_clause
            {
                // This prevents warnings about unused arguments in a way that avoids suppression of
                // lints for the implementation itself
                #[expect(clippy::let_underscore_untyped, reason = "Generated code")]
                {
                    #( let _ = #call_args; )*
                }
                #[allow(unused_braces, reason = "Generated code")]
                #original_arm_body
            }
        });
    }

    Ok(generated_fns)
}
