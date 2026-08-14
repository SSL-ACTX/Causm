use super::inference::infer_expression_type;
use crate::analyzer::EntropicAnalyzer;
use causm_core::types::Type;
use causm_core::*;

pub fn estimate_expression_cost(
    analyzer: &EntropicAnalyzer,
    expr: &Expression,
) -> u64 {
    match expr {
        Expression::Call { routine, args } => {
            let arg_cost: u64 = args
                .iter()
                .map(|a| estimate_expression_cost(analyzer, a))
                .sum();
            let taking_ms = analyzer
                .routines
                .get(routine)
                .map(|info| info.taking_ms)
                .unwrap_or(0);
            arg_cost + taking_ms
        }
        Expression::MethodCall {
            target,
            method,
            args,
            resolved_routine,
            resolved_budget,
        } => {
            let target_cost = estimate_expression_cost(analyzer, target);
            let arg_cost: u64 = args
                .iter()
                .map(|a| estimate_expression_cost(analyzer, a))
                .sum();
            let routine_opt = resolved_routine.borrow();
            let taking_ms = if let Some(ref routine) = *routine_opt {
                if routine == "<dynamic>" {
                    resolved_budget.borrow().unwrap_or(0)
                } else {
                    analyzer
                        .routines
                        .get(routine)
                        .map(|info| info.taking_ms)
                        .unwrap_or(0)
                }
            } else {
                let target_type =
                    infer_expression_type(analyzer, target).unwrap_or(Type::Unknown);
                if let Type::Custom(struct_name) = target_type {
                    if analyzer.interfaces.contains_key(&struct_name) {
                        let methods = &analyzer.interfaces[&struct_name];
                        methods
                            .iter()
                            .find(|m| &m.name == method)
                            .and_then(|m| m.taking_ms)
                            .unwrap_or(0)
                    } else {
                        let routine_name = format!("{}.{}", struct_name, method);
                        analyzer
                            .routines
                            .get(&routine_name)
                            .map(|info| info.taking_ms)
                            .unwrap_or(0)
                    }
                } else {
                    0
                }
            };
            target_cost + arg_cost + taking_ms
        }
        Expression::BinaryOp { left, right, .. } => {
            1 + estimate_expression_cost(analyzer, left)
                + estimate_expression_cost(analyzer, right)
        }
        Expression::UnaryOp { expr, .. } => {
            1 + estimate_expression_cost(analyzer, expr)
        }
        Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
            1 + fields
                .values()
                .map(|v| estimate_expression_cost(analyzer, v))
                .sum::<u64>()
        }
        Expression::IndexAccess { target, index } => {
            1 + estimate_expression_cost(analyzer, target)
                + estimate_expression_cost(analyzer, index)
        }
        Expression::ArrayLiteral(elements) => {
            1 + elements
                .iter()
                .map(|e| estimate_expression_cost(analyzer, e))
                .sum::<u64>()
        }
        Expression::FieldAccess { target, .. } => {
            let target_type =
                infer_expression_type(analyzer, target).unwrap_or(Type::Unknown);
            let base_cost = match target_type {
                Type::ConstantAccess { access_time_ms, .. } => access_time_ms,
                _ => 1,
            };
            base_cost + estimate_expression_cost(analyzer, target)
        }
        Expression::CloneOp(_)
        | Expression::ChannelReceive(_)
        | Expression::Identifier(_)
        | Expression::Literal(_)
        | Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Boolean(_)
        | Expression::Null
        | Expression::Deferred { .. } => 1,
        Expression::TypeAssertion { target, .. }
        | Expression::TypeCast { expr: target, .. }
        | Expression::TryUnwrap(target)
        | Expression::RefOp(target) => estimate_expression_cost(analyzer, target),
        Expression::Syscall { duration_ms, .. } => duration_ms.unwrap_or(1),
        Expression::EnumVariant { args, .. } => {
            1 + args
                .iter()
                .map(|e| estimate_expression_cost(analyzer, e))
                .sum::<u64>()
        }
        Expression::FString(parts) => {
            1 + parts
                .iter()
                .map(|p| match p {
                    FStringPart::Expr(e) => estimate_expression_cost(analyzer, e),
                    FStringPart::Text(_) => 0,
                })
                .sum::<u64>()
        }
    }
}
