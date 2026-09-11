//! Canonical High-Level Intermediate Representation (HIR)
//!
//! Complete, typed compiler stage sitting between surface AST and Bytecode IR.
//! All syntactic sugar (macros, derives, `f"..."`, `using`, `|>`) is eliminated
//! and canonicalized into typed HIR structures.

pub mod def;
pub mod desugar;
pub mod lower;

pub use def::*;
pub use lower::lower_ast_to_hir;

use causm_core::Program;

/// In-place AST desugaring (compatibility pass before full HIR transition)
pub fn desugar_program(program: &mut Program) {
    desugar::pipeline::desugar_pipeline(program);
}

#[cfg(test)]
mod tests {
    use super::*;
    use causm_core::*;

    #[test]
    fn test_hir_lowering_desugars_using_scope_to_autodrop_and_consume() {
        let ast = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Using {
                        binding: "lock".to_string(),
                        resource: Expression::Identifier("acquire_lock".to_string()),
                        body: vec![SpannedStatement::new(
                            Statement::Expression(Expression::Integer(100)),
                            Span { start: 10, end: 20 },
                        )],
                    },
                    Span { start: 0, end: 30 },
                )],
            }],
        };

        let hir = lower_ast_to_hir(&ast);
        assert_eq!(hir.timelines.len(), 1);
        let stmts = &hir.timelines[0].statements;
        // Should produce: 1. Assignment (lock), 2. inner statement, 3. AutoDrop (lock), 4. Consume (lock)
        assert_eq!(stmts.len(), 4);
        assert!(
            matches!(&stmts[0].stmt, HirStatement::Assignment { target, .. } if target == "lock")
        );
        assert!(matches!(
            &stmts[1].stmt,
            HirStatement::Expression(HirExpression::Integer(100))
        ));
        assert!(
            matches!(&stmts[2].stmt, HirStatement::AutoDrop { target } if target == "lock")
        );
        assert!(
            matches!(&stmts[3].stmt, HirStatement::Consume { target } if target == "lock")
        );
    }

    #[test]
    fn test_hir_lowering_desugars_fstring_interpolation_to_binary_add_chain() {
        let ast = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Expression(Expression::FString(vec![
                        FStringPart::Text("id=".to_string()),
                        FStringPart::Expr(Expression::Integer(42)),
                        FStringPart::Text("!".to_string()),
                    ])),
                    Span { start: 0, end: 15 },
                )],
            }],
        };

        let hir = lower_ast_to_hir(&ast);
        let expr = match &hir.timelines[0].statements[0].stmt {
            HirStatement::Expression(e) => e,
            _ => panic!("expected expression statement"),
        };

        // Should be a nested chain of BinaryOp { op: Add, ... }
        match expr {
            HirExpression::BinaryOp { op, left, right } => {
                assert_eq!(*op, BinaryOperator::Add);
                assert!(
                    matches!(right.as_ref(), HirExpression::Literal(s) if s == "!")
                );
                match left.as_ref() {
                    HirExpression::BinaryOp {
                        op: inner_op,
                        left: inner_left,
                        right: inner_right,
                    } => {
                        assert_eq!(*inner_op, BinaryOperator::Add);
                        assert!(
                            matches!(inner_left.as_ref(), HirExpression::Literal(s) if s == "id=")
                        );
                        assert!(
                            matches!(inner_right.as_ref(), HirExpression::ToStr(val) if matches!(val.as_ref(), HirExpression::Integer(42)))
                        );
                    }
                    other => panic!("expected inner BinaryOp, got {:?}", other),
                }
            }
            other => panic!("expected outer BinaryOp, got {:?}", other),
        }
    }
}
