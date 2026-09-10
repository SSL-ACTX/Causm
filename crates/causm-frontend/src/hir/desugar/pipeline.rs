//! Pipeline `|>` desugaring pass.
//!
//! `x |> f(y)` → `f(x, y)`
//! `x |> f`    → `f(x)`
//!
//! The Pratt parser already desugars pipeline in the arena (for Call,
//! Identifier, and MethodCall RHS cases), and `to_ast_expression` handles the
//! fallback. This pass is the authoritative HIR-level pipeline desugar for any
//! residual `Expression::Pipeline` that might survive, and serves as the home
//! for future AST→HIR sugar lowering.
//!
//! NOTE: `causm_core::Expression` does NOT have a `Pipeline` variant — the
//! desugar happens fully at the arena→AST boundary. This pass therefore acts
//! as a recursive AST normaliser / future sugar expansion point.

use causm_core::{Expression, FStringPart, Program, SpannedStatement, Statement};

/// Walk every expression in `program` and normalise desugared forms.
pub fn desugar_pipeline(program: &mut Program) {
    for timeline in &mut program.timelines {
        desugar_stmts(&mut timeline.statements);
    }
}

fn desugar_stmts(stmts: &mut [SpannedStatement]) {
    for s in stmts.iter_mut() {
        desugar_stmt(&mut s.stmt);
    }
}

fn desugar_stmt(stmt: &mut Statement) {
    match stmt {
        Statement::Assignment { expr, .. } | Statement::Expression(expr) => {
            *expr = desugar_expr(std::mem::replace(expr, Expression::Null));
        }
        Statement::DestructureAssignment { expr, .. } => {
            *expr = desugar_expr(std::mem::replace(expr, Expression::Null));
        }
        Statement::Return(Some(expr)) => {
            *expr = desugar_expr(std::mem::replace(expr, Expression::Null));
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            *condition = desugar_expr(std::mem::replace(condition, Expression::Null));
            desugar_stmts(then_branch);
            if let Some(b) = else_branch {
                desugar_stmts(b);
            }
        }
        Statement::While { condition, body, .. } => {
            *condition = desugar_expr(std::mem::replace(condition, Expression::Null));
            desugar_stmts(body);
        }
        Statement::Loop { body, .. }
        | Statement::LoopTick { body }
        | Statement::LoopOn { body, .. } => {
            desugar_stmts(body);
        }
        Statement::RoutineDef { body, .. } => {
            desugar_stmts(body);
        }
        Statement::Using { body, .. } => {
            desugar_stmts(body);
        }
        _ => {}
    }
}

/// Recursively normalise desugared forms in an expression tree.
/// Currently this is a pure normaliser — `Pipeline` is handled at the
/// arena boundary. Future sugar passes (const generic mono, etc.) are added
/// as match arms here.
fn desugar_expr(expr: Expression) -> Expression {
    match expr {
        Expression::BinaryOp { left, right, op } => Expression::BinaryOp {
            left: Box::new(desugar_expr(*left)),
            right: Box::new(desugar_expr(*right)),
            op,
        },
        Expression::UnaryOp { op, expr } => Expression::UnaryOp {
            op,
            expr: Box::new(desugar_expr(*expr)),
        },
        Expression::Call { routine, args } => Expression::Call {
            routine,
            args: args.into_iter().map(desugar_expr).collect(),
        },
        Expression::MethodCall {
            target,
            method,
            args,
            resolved_routine,
            resolved_budget,
        } => Expression::MethodCall {
            target: Box::new(desugar_expr(*target)),
            method,
            args: args.into_iter().map(desugar_expr).collect(),
            resolved_routine,
            resolved_budget,
        },
        Expression::FieldAccess { target, field } => Expression::FieldAccess {
            target: Box::new(desugar_expr(*target)),
            field,
        },
        Expression::IndexAccess { target, index } => Expression::IndexAccess {
            target: Box::new(desugar_expr(*target)),
            index: Box::new(desugar_expr(*index)),
        },
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => Expression::If {
            condition: Box::new(desugar_expr(*condition)),
            then_branch: Box::new(desugar_expr(*then_branch)),
            else_branch: Box::new(desugar_expr(*else_branch)),
        },
        Expression::Tuple(elems) => {
            Expression::Tuple(elems.into_iter().map(desugar_expr).collect())
        }
        Expression::ArrayLiteral(elems) => {
            Expression::ArrayLiteral(elems.into_iter().map(desugar_expr).collect())
        }
        Expression::ArrayRepeat { value, count } => Expression::ArrayRepeat {
            value: Box::new(desugar_expr(*value)),
            count: Box::new(desugar_expr(*count)),
        },
        Expression::TryUnwrap(inner) => {
            Expression::TryUnwrap(Box::new(desugar_expr(*inner)))
        }
        Expression::Turbofish { expr, type_args } => Expression::Turbofish {
            expr: Box::new(desugar_expr(*expr)),
            type_args,
        },
        Expression::FString(parts) => Expression::FString(
            parts
                .into_iter()
                .map(|p| match p {
                    FStringPart::Expr(e) => FStringPart::Expr(desugar_expr(e)),
                    other => other,
                })
                .collect(),
        ),
        Expression::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => Expression::EnumVariant {
            enum_name,
            variant_name,
            args: args.into_iter().map(desugar_expr).collect(),
        },
        Expression::StructLit(tag, fields) => Expression::StructLit(
            tag,
            fields
                .into_iter()
                .map(|(k, v)| (k, desugar_expr(v)))
                .collect(),
        ),
        Expression::TypeAssertion { target, cast_type } => Expression::TypeAssertion {
            target: Box::new(desugar_expr(*target)),
            cast_type,
        },
        Expression::TypeCast { expr, target_type } => Expression::TypeCast {
            expr: Box::new(desugar_expr(*expr)),
            target_type,
        },
        // Leaf nodes — no recursion needed.
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use causm_core::{Expression, Program, Span, SpannedStatement, TimeCoordinate, TimelineBlock};

    fn make_prog(expr: Expression) -> Program {
        Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Expression(expr),
                    Span { start: 0, end: 0 },
                )],
            }],
        }
    }

    fn extract(prog: &Program) -> &Expression {
        match &prog.timelines[0].statements[0].stmt {
            Statement::Expression(e) => e,
            _ => panic!("expected expression statement"),
        }
    }

    #[test]
    fn test_hir_pipeline_desugar_call_args_preserved() {
        // `add(1, 2)` passes through unchanged.
        let expr = Expression::Call {
            routine: "add".into(),
            args: vec![Expression::Integer(1), Expression::Integer(2)],
        };
        let mut prog = make_prog(expr.clone());
        desugar_pipeline(&mut prog);
        assert_eq!(extract(&prog), &expr);
    }

    #[test]
    fn test_hir_pipeline_desugar_nested_call_recurses() {
        // `f(g(x))` — inner call args are visited.
        let inner = Expression::Call {
            routine: "g".into(),
            args: vec![Expression::Identifier("x".into())],
        };
        let outer = Expression::Call {
            routine: "f".into(),
            args: vec![inner.clone()],
        };
        let mut prog = make_prog(outer.clone());
        desugar_pipeline(&mut prog);
        assert_eq!(extract(&prog), &outer);
    }

    #[test]
    fn test_hir_pipeline_desugar_fstring_parts_visited() {
        // FString parts containing exprs are recursed into.
        let fstr = Expression::FString(vec![
            FStringPart::Text("hello ".into()),
            FStringPart::Expr(Expression::Identifier("name".into())),
        ]);
        let mut prog = make_prog(fstr.clone());
        desugar_pipeline(&mut prog);
        assert_eq!(extract(&prog), &fstr);
    }
}
