use crate::analyzer::{EntropicAnalyzer, SemanticError};
use causm_core::*;

impl EntropicAnalyzer {
    pub(crate) fn analyze_statement(
        &mut self,
        stmt: &SpannedStatement,
    ) -> Result<(), SemanticError> {
        {
            let is_declaration = matches!(
                &stmt.stmt,
                Statement::RoutineDef { .. }
                    | Statement::TypeDecl { .. }
                    | Statement::EnumDecl { .. }
                    | Statement::InterfaceDecl { .. }
                    | Statement::ForeignBlock { .. }
            );
            if !is_declaration {
                let branch =
                    self.branch_contexts.get_mut(&self.current_branch).unwrap();
                branch.accumulated_cost += 1;
                if let Statement::NetworkRequest { .. } = &stmt.stmt {
                    branch.accumulated_cost += 5;
                }
            }
        }

        macro_rules! dispatch_one {
            ($name:ident { $($field:ident: $type:ty),* }) => {
                if let Statement::$name { $($field),* } = &stmt.stmt {
                    return self.$name($($field),*);
                }
            };
            ($name:ident($tuple_type:ty)) => {
                if let Statement::$name(ref v0) = &stmt.stmt {
                    return self.$name(v0);
                }
            };
            ($name:ident) => {
                if let Statement::$name = &stmt.stmt {
                    return self.$name();
                }
            };
        }

        #[allow(non_snake_case)]
        macro_rules! dispatch_statement {
            ($($name:ident $({ $($field:ident: $type:ty),* })? $(( $($tuple_type:ty),* ))?),*) => {
                $(
                    dispatch_one!($name $({ $($field: $type),* })? $(( $($tuple_type),* ))?);
                )*
            };
        }

        causm_core::statements!(dispatch_statement);

        Ok(())
    }
}

pub fn estimate_block_cost(
    analyzer: &EntropicAnalyzer,
    block: &[SpannedStatement],
) -> u64 {
    block
        .iter()
        .map(|stmt| estimate_statement_cost(analyzer, &stmt.stmt))
        .sum()
}

pub fn estimate_statement_cost(
    analyzer: &EntropicAnalyzer,
    stmt: &Statement,
) -> u64 {
    let expr_cost = match stmt {
        Statement::Assignment { expr, .. }
        | Statement::FieldUpdate { value: expr, .. }
        | Statement::Expression(expr)
        | Statement::Print(expr)
        | Statement::Debug(expr) => {
            crate::expression::estimate_expression_cost(analyzer, expr)
        }
        _ => 0,
    };
    expr_cost + stmt.estimate_cost(|b| estimate_block_cost(analyzer, b))
}
