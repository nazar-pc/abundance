use quote::{ToTokens, quote};
use std::iter;
use syn::{Arm, Block, Expr, ExprBlock, ExprMatch, Ident, Pat, PathArguments, Stmt, parse_quote};

fn is_exact_ok(expr: &Expr) -> bool {
    let expected = quote! { Ok(ControlFlow::Continue(())) };
    let actual = expr.to_token_stream();
    actual.to_string() == expected.to_string()
}

fn validate_match_on_self(expr_match: &ExprMatch) -> anyhow::Result<()> {
    if let Expr::Path(expr_path) = &*expr_match.expr
        && expr_path.path.is_ident("self")
    {
        Ok(())
    } else {
        Err(anyhow::anyhow!("`match` must be on literal `self`"))
    }
}

fn get_variant_ident_and_block(arm: &Arm) -> anyhow::Result<(Ident, Arm)> {
    if let Some(guard) = &arm.guard {
        return Err(anyhow::anyhow!(
            "`match` arms must not have guards: {guard:?}"
        ));
    }

    let Expr::Block(ExprBlock { .. }) = arm.body.as_ref() else {
        return Err(anyhow::anyhow!(
            "`match` arm body must be a block {{ .. }}: {}",
            arm.body.to_token_stream()
        ));
    };

    let path = match &arm.pat {
        Pat::Struct(pat_struct) => &pat_struct.path,
        Pat::TupleStruct(pat_tuple_struct) => &pat_tuple_struct.path,
        Pat::Path(expr_path) => &expr_path.path,
        _ => Err(anyhow::anyhow!(
            "`match` pattern must be `Self::Variant`, `Self::Variant {{ .. }}` or \
            `Self::Variant(..)`: {}",
            arm.pat.to_token_stream()
        ))?,
    };

    let mut segments = path.segments.iter();

    if let Some(first_path_segment) = segments.next()
        && first_path_segment.ident == "Self"
        && first_path_segment.arguments == PathArguments::None
        && let Some(second_path_segment) = segments.next()
        && second_path_segment.arguments == PathArguments::None
        && segments.next().is_none()
    {
        let mut arm = arm.clone();
        let Expr::Block(ExprBlock { block, .. }) = arm.body.as_mut() else {
            unreachable!("Checked above; qed")
        };

        let continue_expr = parse_quote! { Ok(ControlFlow::Continue(())) };
        if let Some(last_statement) = block.stmts.last_mut() {
            // Has at least one statement
            if let Stmt::Expr(expr, maybe_semicolon) = last_statement {
                // Statement ends with `;`
                match expr {
                    Expr::Return(expr_return) => {
                        // Expression is `return?;`

                        // Remove `;` first
                        maybe_semicolon.take();

                        *expr = if let Some(inner_expr) = &mut expr_return.expr {
                            // Replace `return T` with `T`
                            inner_expr.as_ref().clone()
                        } else {
                            // Replace `return` with `Ok(ControlFlow::Continue(()))`
                            continue_expr
                        }
                    }
                    Expr::If(expr_if) => {
                        if expr_if.else_branch.is_none() {
                            // If branch with `else`, insert `Ok(ControlFlow::Continue(()))` at the
                            // end of the block
                            block.stmts.push(Stmt::Expr(continue_expr, None));
                        }
                    }
                    _ => {
                        if maybe_semicolon.is_some() {
                            // Something other than `return` ended with semicolon, insert
                            // `Ok(ControlFlow::Continue(()))` at the end of the block
                            block.stmts.push(Stmt::Expr(continue_expr, None));
                        }
                    }
                }
            }
        } else {
            // No statements, insert `Ok(ControlFlow::Continue(()))` at the end of the block
            block.stmts.push(Stmt::Expr(continue_expr, None));
        }

        Ok((second_path_segment.ident.clone(), arm))
    } else {
        Err(anyhow::anyhow!(
            "`match` pattern must be unqualified `Self::Variant`: {}",
            path.to_token_stream()
        ))
    }
}

fn process_match(
    expr_match: &ExprMatch,
) -> anyhow::Result<impl Iterator<Item = anyhow::Result<(Ident, Arm)>>> {
    validate_match_on_self(expr_match)?;

    Ok(expr_match.arms.iter().map(get_variant_ident_and_block))
}

#[expect(clippy::type_complexity, reason = "Internal API")]
pub(super) fn extract_variant_arms(
    block: &Block,
) -> anyhow::Result<Box<dyn Iterator<Item = anyhow::Result<(Ident, Arm)>> + '_>> {
    let mut stmts_iter = block.stmts.iter();

    let first_stmt = stmts_iter.next().ok_or_else(|| {
        anyhow::anyhow!("Function body must have exactly 1 or 2 statements, but it is empty")
    })?;

    let maybe_second_stmt = stmts_iter.next();

    stmts_iter.next().is_none().ok_or_else(|| {
        anyhow::anyhow!("Function body must have exactly 1 or 2 statements, but more is present")
    })?;

    if let Some(second_stmt) = maybe_second_stmt {
        let Stmt::Expr(Expr::Match(expr_match), None) = first_stmt else {
            return Err(anyhow::anyhow!(
                "First statement must be a `match` on `self`: {}",
                first_stmt.to_token_stream()
            ));
        };

        let Stmt::Expr(ok_expr, None) = second_stmt else {
            return Err(anyhow::anyhow!(
                "Second statement must be exactly `Ok(ControlFlow::Continue(()))`: {}",
                second_stmt.to_token_stream()
            ));
        };

        if !is_exact_ok(ok_expr) {
            return Err(anyhow::anyhow!(
                "Second statement must be exactly `Ok(ControlFlow::Continue(()))`: {}",
                second_stmt.to_token_stream()
            ));
        }

        Ok(Box::new(process_match(expr_match)?))
    } else {
        let Stmt::Expr(expr, None) = first_stmt else {
            return Err(anyhow::anyhow!(
                "Single statement must be a tail expression (no semicolon)"
            ));
        };

        if is_exact_ok(expr) {
            Ok(Box::new(iter::empty()))
        } else if let Expr::Match(expr_match) = expr {
            Ok(Box::new(process_match(expr_match)?))
        } else {
            Err(anyhow::anyhow!(
                "Single tail expression must be either `match` on `self` or \
                `Ok(ControlFlow::Continue(()))`"
            ))
        }
    }
}
