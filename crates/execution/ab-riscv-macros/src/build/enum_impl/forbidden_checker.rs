use syn::visit::{Visit, visit_expr};
use syn::{Block, Expr, ExprCall, ExprPath, ExprReturn, ExprStruct, Ident, Path, PathArguments};

struct ForbiddenChecker<'a> {
    found: bool,
    enum_name: &'a Ident,
}

impl<'ast> Visit<'ast> for ForbiddenChecker<'ast> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        if self.found {
            return;
        }

        match expr {
            Expr::Return(ExprReturn { .. }) => {
                self.found = true;
            }

            // Unit variant: `Enum::Foo` (qself must be None to avoid <T as Trait>::Assoc false
            // positives)
            Expr::Path(ExprPath {
                qself: None, path, ..
            }) if is_forbidden_variant_path(path, self.enum_name) => {
                self.found = true;
            }

            // Tuple variant: `Enum::Foo(...)`
            Expr::Call(ExprCall { func, .. }) => {
                if let Expr::Path(ExprPath {
                    qself: None, path, ..
                }) = func.as_ref()
                    && is_forbidden_variant_path(path, self.enum_name)
                {
                    self.found = true;
                }
            }

            // Struct variant: `Enum::Foo { .. }`
            Expr::Struct(ExprStruct { path, .. }) => {
                if is_forbidden_variant_path(path, self.enum_name) {
                    self.found = true;
                }
            }

            _ => {}
        }

        // Recurse
        visit_expr(self, expr);
    }
}

fn is_forbidden_variant_path(path: &Path, enum_name: &Ident) -> bool {
    let mut segments = path.segments.iter().rev();

    let Some(variant_segment) = segments.next() else {
        return false;
    };
    if !matches!(variant_segment.arguments, PathArguments::None) {
        return false;
    }

    let Some(enum_segment) = segments.next() else {
        return false;
    };
    &enum_segment.ident == enum_name
}

/// Returns `true` if the block contains either an explicit `return` expression or a direct
/// construction of any variant of the given enum (e.g. `Enum::Foo`, `Enum::Foo(arg)`,
/// `Enum::Foo { ... }`, including with generics like `MyError::<T>::Foo`).
pub(super) fn block_contains_forbidden_syntax(block: &Block, enum_name: &Ident) -> bool {
    let mut checker = ForbiddenChecker {
        found: false,
        enum_name,
    };
    checker.visit_block(block);
    checker.found
}
