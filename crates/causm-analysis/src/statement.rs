use crate::analyzer::{EntropicAnalyzer, SemanticError};
use causm_core::*;

impl EntropicAnalyzer {
    pub(crate) fn analyze_statement(
        &mut self,
        stmt: &SpannedStatement,
    ) -> Result<(), SemanticError> {
        {
            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.accumulated_cost += 1;
            if let Statement::NetworkRequest { .. } = &stmt.stmt {
                branch.accumulated_cost += 5;
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
    let base = 1;
    let extra = match stmt {
        Statement::NetworkRequest { .. } => 5,
        Statement::Split { .. }
        | Statement::Merge { .. }
        | Statement::Anchor(_)
        | Statement::Rewind(_)
        | Statement::Commit(_)
        | Statement::Send { .. }
        | Statement::ChannelOpen { .. }
        | Statement::ChannelSend { .. }
        | Statement::AcausalReset { .. }
        | Statement::Capability(_) => 0,
        Statement::Assignment { expr, .. } => {
            crate::expression::estimate_expression_cost(analyzer, expr)
        }
        Statement::FieldUpdate { value, .. } => {
            crate::expression::estimate_expression_cost(analyzer, value)
        }
        Statement::Expression(expr) => {
            crate::expression::estimate_expression_cost(analyzer, expr)
        }
        Statement::RelativisticBlock { body, .. } => {
            estimate_block_cost(analyzer, body)
        }
        Statement::Isolate(block) => estimate_block_cost(analyzer, &block.body),
        Statement::Inspect { body, .. } => estimate_block_cost(analyzer, body),
        Statement::Lease { duration_ms, .. } => *duration_ms,
        Statement::Watchdog { recovery, .. } => {
            estimate_block_cost(analyzer, recovery)
        }
        Statement::If {
            then_branch,
            else_branch,
            ..
        } => {
            1 + estimate_block_cost(analyzer, then_branch).max(estimate_block_cost(
                analyzer,
                else_branch.as_ref().unwrap_or(&Vec::new()),
            ))
        }
        Statement::For { pacing_ms, .. } => pacing_ms.unwrap_or(1),
        Statement::Print(expr) | Statement::Debug(expr) => {
            1 + crate::expression::estimate_expression_cost(analyzer, expr)
        }
        Statement::Speculate { body, fallback, .. } => {
            let fallback_cost = estimate_block_cost(
                analyzer,
                fallback.as_ref().unwrap_or(&Vec::new()),
            );
            let body_cost = estimate_block_cost(analyzer, body);
            1 + body_cost + fallback_cost
        }
        Statement::Select {
            max_ms,
            cases,
            timeout,
            ..
        } => {
            let case_max_cost = cases
                .iter()
                .map(|c| estimate_block_cost(analyzer, &c.body))
                .max()
                .unwrap_or(0);
            let timeout_cost = timeout
                .as_ref()
                .map(|b| estimate_block_cost(analyzer, b))
                .unwrap_or(0);
            *max_ms + 1 + case_max_cost.max(timeout_cost)
        }
        Statement::MatchEntropy {
            valid_branch,
            decayed_branch,
            pending_branch,
            consumed_branch,
            ..
        } => {
            let valid_cost = valid_branch
                .as_ref()
                .map(|(_, body)| estimate_block_cost(analyzer, body))
                .unwrap_or(0);
            let decayed_cost = decayed_branch
                .as_ref()
                .map(|(_, body)| estimate_block_cost(analyzer, body))
                .unwrap_or(0);
            let pending_cost = pending_branch
                .as_ref()
                .map(|body| estimate_block_cost(analyzer, body))
                .unwrap_or(0);
            let consumed_cost = consumed_branch
                .as_ref()
                .map(|body| estimate_block_cost(analyzer, body))
                .unwrap_or(0);
            1 + valid_cost
                .max(decayed_cost)
                .max(pending_cost)
                .max(consumed_cost)
        }
        Statement::Collapse => 0,
        Statement::SplitMap { .. } => 1,
        Statement::Yield(_) => 0,
        Statement::Loop { max_ms, .. } => *max_ms,
        Statement::LoopTick { .. } => 1,
        Statement::Slice { .. } => 0,
        Statement::Await(_) => 1,
        Statement::AwaitChan(_) => 1,
        Statement::SpeculationMode(_) => 0,
        Statement::Break => 0,
        Statement::Entangle { .. } => 0,
        Statement::TypeDecl { .. } => 0,
        Statement::DecayHandler { body, .. } => estimate_block_cost(analyzer, body),
        Statement::AssertTime { fallback, .. } => {
            1 + fallback
                .as_ref()
                .map(|b| estimate_block_cost(analyzer, b))
                .unwrap_or(0)
        }
        Statement::RoutineDef { taking_ms, .. } => taking_ms.unwrap_or(0),
        Statement::Return(_) => 0,
    };
    base + extra
}
