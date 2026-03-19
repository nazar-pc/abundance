use syn::visit::{Visit, visit_expr};
use syn::{Block, Expr, ExprReturn};

struct ForbiddenChecker {
    found: bool,
}

impl Visit<'_> for ForbiddenChecker {
    fn visit_expr(&mut self, expr: &Expr) {
        if self.found {
            return;
        }

        if let Expr::Return(ExprReturn { .. }) = expr {
            self.found = true;
        }

        // Recurse
        visit_expr(self, expr);
    }
}

/// Returns `true` if the block contains an explicit `return` expression
pub(super) fn block_contains_forbidden_syntax(block: &Block) -> bool {
    let mut checker = ForbiddenChecker { found: false };
    checker.visit_block(block);
    checker.found
}
