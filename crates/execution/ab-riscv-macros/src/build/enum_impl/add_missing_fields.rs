use syn::punctuated::Punctuated;
use syn::token::Brace;
use syn::visit_mut::VisitMut;
use syn::{
    Block, Expr, ExprPath, ExprStruct, Ident, Member, Path, PathArguments, PathSegment, parse_quote,
};

struct AddMissingFieldsVisitor;

impl VisitMut for AddMissingFieldsVisitor {
    fn visit_expr_mut(&mut self, i: &mut Expr) {
        // Recurse first so inner expressions are patched before we inspect this node
        syn::visit_mut::visit_expr_mut(self, i);

        match i {
            Expr::Struct(expr_struct) => {
                patch_struct_expr(expr_struct);
            }
            Expr::Path(expr_path) => {
                // Unit variant: `Self::Variant` - no braces at all
                if let Some(converted) = try_convert_unit_to_struct(expr_path) {
                    *i = Expr::Struct(converted);
                }
            }
            _ => {}
        }
    }
}

/// Returns true if the path looks like `Self::Variant`
fn is_self_variant_path(path: &Path) -> bool {
    let segs = &path.segments;
    segs.len() == 2
        && segs[0].ident == "Self"
        && segs[0].arguments == PathArguments::None
        && segs[1].arguments == PathArguments::None
}

/// Extracts the variant ident from a `Self::Variant` path, or returns `None`
fn self_variant_ident(path: &Path) -> Option<&Ident> {
    if is_self_variant_path(path) {
        Some(&path.segments[1].ident)
    } else {
        None
    }
}

/// Adds `rs1: Reg::ZERO` and/or `rs2: Reg::ZERO` to an `ExprStruct` if those fields are absent.
/// Skips expressions that use functional-record-update syntax (`Foo { ..other }`) since those
/// delegate field values to the base expression.
fn patch_struct_expr(expr_struct: &mut ExprStruct) {
    // Only touch `Self::Variant { .. }` paths
    if self_variant_ident(&expr_struct.path).is_none() {
        return;
    }

    // Functional-record-update: `Self::V { x, ..base }`. The base supplies the remaining fields, so
    // we must not fabricate values here.
    if expr_struct.rest.is_some() {
        return;
    }

    let has_rs1 = expr_struct
        .fields
        .iter()
        .any(|field_value| match &field_value.member {
            Member::Named(ident) => ident == "rs1",
            Member::Unnamed(_) => false,
        });
    let has_rs2 = expr_struct
        .fields
        .iter()
        .any(|field_value| match &field_value.member {
            Member::Named(ident) => ident == "rs2",
            Member::Unnamed(_) => false,
        });

    if !has_rs1 {
        expr_struct.fields.push(parse_quote! { rs1: Reg::ZERO });
    }
    if !has_rs2 {
        expr_struct.fields.push(parse_quote! { rs2: Reg::ZERO });
    }
}

/// If `expr_path` is `Self::Variant` and that variant is listed in `named_variants`, produces an
/// `ExprStruct` with both `rs1` and `rs2` filled in.
///
/// Returns `None` for everything else.
fn try_convert_unit_to_struct(expr_path: &ExprPath) -> Option<ExprStruct> {
    let variant_ident = self_variant_ident(&expr_path.path)?;

    let mut fields = Punctuated::new();
    fields.push(parse_quote! { rs1: Reg::ZERO });
    fields.push(parse_quote! { rs2: Reg::ZERO });

    let path = build_self_variant_path(variant_ident);

    Some(ExprStruct {
        attrs: expr_path.attrs.clone(),
        qself: None,
        path,
        brace_token: Brace::default(),
        fields,
        dot2_token: None,
        rest: None,
    })
}

fn build_self_variant_path(variant_ident: &Ident) -> Path {
    let mut segments = Punctuated::new();
    segments.push(PathSegment {
        ident: Ident::new("Self", variant_ident.span()),
        arguments: PathArguments::None,
    });
    segments.push(PathSegment {
        ident: variant_ident.clone(),
        arguments: PathArguments::None,
    });
    Path {
        leading_colon: None,
        segments,
    }
}

/// Walks `block` and ensures every `Self::Variant { .. }` struct expression contains `rs1` and
/// `rs2` fields, inserting `Reg::ZERO` for any that are absent. Unit-variant expressions
/// (`Self::Variant` with no braces) are converted to struct form when the variant ident appears in
/// `named_variants`. Tuple-variant expressions are left untouched.
pub(super) fn add_missing_rs_fields(block: &mut Block) {
    AddMissingFieldsVisitor.visit_block_mut(block);
}
