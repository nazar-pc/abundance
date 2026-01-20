use std::collections::HashSet;
use syn::visit_mut::{VisitMut, visit_expr_mut};
use syn::{Block, Expr, ExprCall, ExprPath, ExprStruct, Ident};

fn extract_self_variant_ident(expr: &Expr) -> Option<Ident> {
    let path = match expr {
        Expr::Path(ExprPath { path, .. }) => path,
        Expr::Call(ExprCall { func, .. }) => {
            if let Expr::Path(ExprPath { path, .. }) = func.as_ref() {
                path
            } else {
                return None;
            }
        }
        Expr::Struct(ExprStruct { path, .. }) => path,
        _ => return None,
    };

    // Must be exactly Self::Variant with no path arguments or leading ::
    if path.leading_colon.is_some() {
        return None;
    }

    let mut segments = path.segments.iter();
    match (segments.next(), segments.next(), segments.next()) {
        (Some(self_seg), Some(var_seg), None)
            if self_seg.ident == "Self"
                && self_seg.arguments.is_empty()
                && var_seg.arguments.is_empty() =>
        {
            Some(var_seg.ident.clone())
        }
        _ => None,
    }
}

struct IgnoredVariantsRemover<'a> {
    allowed: &'a HashSet<Ident>,
}

impl VisitMut for IgnoredVariantsRemover<'_> {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Some(variant) = extract_self_variant_ident(expr)
            && !self.allowed.contains(&variant)
        {
            // Replace the entire variant construction with `None?`
            *expr = syn::parse_quote! { None? };
            // Do not recurse – the subtree is discarded
            return;
        }

        // Recurse
        visit_expr_mut(self, expr);
    }
}

/// Removes ignored (not allowed) variants from a given block in the form of
/// `Self::Variant[|( .. )|{ .. }` patterns, replacing them with `None?`
pub(super) fn remove_ignored_variants(block: &mut Block, allowed: &HashSet<Ident>) {
    let mut checker = IgnoredVariantsRemover { allowed };
    checker.visit_block_mut(block);
}
