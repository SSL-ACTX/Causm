use crate::parser::expressions::{parse_expression, parse_pattern};
use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::*;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;
use std::collections::HashMap;

pub fn parse_control_flow_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::match_stmt => {
            let mut inner = pair.into_inner();
            let target = parse_expression(inner.next().unwrap());
            let mut arms = Vec::new();
            for arm_pair in inner {
                let mut arm_inner = arm_pair.into_inner();
                let pattern = parse_pattern(arm_inner.next().unwrap());
                let next = arm_inner.next().unwrap();
                let (guard, body_pair) = if next.as_rule() == Rule::guard_clause {
                    let g = parse_expression(next.into_inner().next().unwrap());
                    (Some(g), arm_inner.next().unwrap())
                } else {
                    (None, next)
                };

                let body = match body_pair.as_rule() {
                    Rule::statement_block => body_pair
                        .into_inner()
                        .filter_map(|stmt_pair| stmt_pair.into_inner().next())
                        .map(parse_statement)
                        .collect(),
                    _ => vec![parse_statement(body_pair)],
                };

                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                });
            }
            Statement::Match { target, arms }
        }
        Rule::if_let_stmt => {
            let mut inner = pair.into_inner();
            let pattern = parse_pattern(inner.next().unwrap());
            let expr = parse_expression(inner.next().unwrap());
            let then_branch = if let Some(b) = inner.next() {
                b.into_inner()
                    .filter_map(|stmt_pair| stmt_pair.into_inner().next())
                    .map(parse_statement)
                    .collect()
            } else {
                Vec::new()
            };

            let mut else_branch = None;
            let mut reconcile_rules = None;

            if let Some(next_pair) = inner.next() {
                match next_pair.as_rule() {
                    Rule::statement_block => {
                        else_branch = Some(
                            next_pair
                                .into_inner()
                                .filter_map(|stmt_pair| {
                                    stmt_pair.into_inner().next()
                                })
                                .map(parse_statement)
                                .collect(),
                        );
                        if let Some(rec_pair) = inner.next() {
                            reconcile_rules = Some(parse_reconcile_clause(rec_pair));
                        }
                    }
                    Rule::reconcile_clause => {
                        reconcile_rules = Some(parse_reconcile_clause(next_pair));
                    }
                    _ => {}
                }
            }

            // If pattern is simple identifier and expr is TypeAssertion, we can also maintain Statement::If compatibility
            if let Pattern::Identifier(ref id) = pattern {
                if let Expression::TypeAssertion { .. } = expr {
                    return Statement::If {
                        binding: Some(id.clone()),
                        condition: expr,
                        then_branch,
                        else_branch,
                        reconcile: reconcile_rules,
                    };
                }
            }

            Statement::IfLet {
                pattern,
                expr,
                then_branch,
                else_branch,
                reconcile: reconcile_rules,
            }
        }
        Rule::if_stmt => {
            let mut inner = pair.into_inner();
            let condition = inner
                .next()
                .map(parse_expression)
                .unwrap_or(Expression::Literal("false".into()));
            let then_branch = if let Some(b) = inner.next() {
                b.into_inner()
                    .filter_map(|stmt_pair| stmt_pair.into_inner().next())
                    .map(parse_statement)
                    .collect()
            } else {
                Vec::new()
            };

            let mut else_branch = None;
            let mut reconcile_rules = None;

            if let Some(next_pair) = inner.next() {
                match next_pair.as_rule() {
                    Rule::statement_block => {
                        else_branch = Some(
                            next_pair
                                .into_inner()
                                .filter_map(|stmt_pair| {
                                    stmt_pair.into_inner().next()
                                })
                                .map(parse_statement)
                                .collect(),
                        );
                        if let Some(rec_pair) = inner.next() {
                            reconcile_rules = Some(parse_reconcile_clause(rec_pair));
                        }
                    }
                    Rule::if_stmt => {
                        let parsed_else_if = parse_control_flow_stmt(next_pair);
                        else_branch = Some(vec![SpannedStatement {
                            stmt: parsed_else_if,
                            span: Span { start: 0, end: 0 },
                        }]);
                        if let Some(rec_pair) = inner.next() {
                            reconcile_rules = Some(parse_reconcile_clause(rec_pair));
                        }
                    }
                    Rule::reconcile_clause => {
                        reconcile_rules = Some(parse_reconcile_clause(next_pair));
                    }
                    _ => {}
                }
            }

            Statement::If {
                binding: None,
                condition,
                then_branch,
                else_branch,
                reconcile: reconcile_rules,
            }
        }
        Rule::loop_stmt => {
            let s_str = pair.as_str().trim_start();
            let mut inner = pair.into_inner();
            let first = inner.next();
            if s_str.starts_with("loop on ") {
                let target_expr = parse_expression(first.unwrap());
                let body_pair = inner.next().unwrap();
                let body = match body_pair.as_rule() {
                    Rule::statement_block => body_pair
                        .into_inner()
                        .filter_map(|p| p.into_inner().next())
                        .map(parse_statement)
                        .collect(),
                    _ => vec![parse_statement(body_pair)],
                };
                Statement::LoopOn {
                    target: target_expr,
                    body,
                }
            } else if let Some(first) = first {
                if first.as_rule() == Rule::duration_limit {
                    let max_value = parse_duration_limit(first);
                    let mut body = Vec::new();
                    for stmt_pair in inner {
                        if stmt_pair.as_rule() == Rule::statement {
                            if let Some(actual_stmt) = stmt_pair.into_inner().next()
                            {
                                body.push(parse_statement(actual_stmt));
                            }
                        }
                    }
                    Statement::Loop {
                        max_ms: max_value,
                        body,
                    }
                } else {
                    let mut body = Vec::new();
                    if first.as_rule() == Rule::statement {
                        if let Some(actual_stmt) = first.into_inner().next() {
                            body.push(parse_statement(actual_stmt));
                        }
                    }
                    for stmt_pair in inner {
                        if stmt_pair.as_rule() == Rule::statement {
                            if let Some(actual_stmt) = stmt_pair.into_inner().next()
                            {
                                body.push(parse_statement(actual_stmt));
                            }
                        }
                    }
                    Statement::LoopTick { body }
                }
            } else {
                Statement::LoopTick { body: Vec::new() }
            }
        }
        Rule::while_stmt => {
            let is_valid_check =
                pair.as_str().trim_start().starts_with("while valid");
            let mut inner = pair.into_inner();
            let condition = parse_expression(inner.next().unwrap());
            let max_ms = parse_duration_limit(inner.next().unwrap());
            let mut body = Vec::new();
            for stmt_pair in inner {
                if stmt_pair.as_rule() == Rule::statement {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::While {
                condition,
                is_valid_check,
                max_ms,
                body,
            }
        }
        Rule::for_step_stmt => {
            let mut inner = pair.into_inner();
            let item_name = inner.next().unwrap().as_str().to_string();
            let range_pair = inner.next().unwrap();
            let source = if range_pair.as_rule() == Rule::range_expr {
                let r_str = range_pair.as_str();
                if let Some((start_str, end_str)) = r_str.split_once("..") {
                    let start_ms = parse_duration_to_ms(start_str);
                    let end_ms = parse_duration_to_ms(end_str);
                    let elems: Vec<Expression> = (start_ms..end_ms)
                        .map(|v| Expression::Integer(v as i64))
                        .collect();
                    Expression::ArrayLiteral(if elems.is_empty() {
                        vec![Expression::Integer(0)]
                    } else {
                        elems
                    })
                } else {
                    parse_expression(range_pair.into_inner().next().unwrap())
                }
            } else {
                parse_expression(range_pair)
            };
            let step_pair = inner.next().unwrap();
            // step_spec contains either duration_wildcard or duration_literal
            let step_ms = {
                let inner_step = step_pair.into_inner().next();
                match inner_step {
                    Some(p)
                        if p.as_rule() == Rule::duration_wildcard
                            || p.as_str() == "_"
                            || p.as_str() == "?" =>
                    {
                        None
                    }
                    Some(p) => Some(parse_duration_to_ms(p.as_str())),
                    None => None,
                }
            };
            let mut body = Vec::new();
            for stmt_pair in inner {
                if stmt_pair.as_rule() == Rule::statement {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::ForStep {
                item_name,
                source,
                step_ms,
                body,
            }
        }
        Rule::for_stmt => {
            let mut inner = pair.into_inner();
            let item = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mode = inner
                .next()
                .map(|p| match p.as_str() {
                    "consume" => ParamMode::Consume,
                    "clone" => ParamMode::Clone,
                    "decay" => ParamMode::Decay,
                    _ => ParamMode::Peek,
                })
                .unwrap_or(ParamMode::Consume);
            let source = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            let mut pacing_ms = None;
            let mut max_ms = None;
            let mut body = Vec::new();

            for next in inner {
                match next.as_rule() {
                    Rule::pacing_opt => {
                        if let Some(dl) = next.into_inner().next() {
                            pacing_ms = Some(parse_duration_limit(dl));
                        }
                    }
                    Rule::duration_limit => {
                        max_ms = Some(parse_duration_limit(next));
                    }
                    Rule::statement => {
                        if let Some(actual_stmt) = next.into_inner().next() {
                            body.push(parse_statement(actual_stmt));
                        }
                    }
                    _ => {}
                }
            }

            Statement::For {
                item_name: item,
                mode,
                source,
                body,
                pacing_ms,
                max_ms,
            }
        }
        Rule::split_stmt => {
            let mut inner = pair.into_inner();
            let parent = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let branches = inner
                .next()
                .map(|p| p.into_inner().map(|id| id.as_str().to_string()).collect())
                .unwrap_or_default();
            Statement::Split { parent, branches }
        }
        Rule::merge_stmt => {
            let mut inner = pair.into_inner();
            let branches = inner
                .next()
                .map(|p| p.into_inner().map(|id| id.as_str().to_string()).collect())
                .unwrap_or_default();
            let target = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut taking_ms = None;
            let mut resolutions = MergeResolution {
                rules: HashMap::new(),
                auto: false,
                fallback: None,
                taking_ms: None,
            };
            let mut fallback = None;

            for element in inner {
                match element.as_rule() {
                    Rule::duration_limit => {
                        taking_ms = Some(parse_duration_limit(element));
                    }
                    Rule::reconcile_clause => {
                        let parsed = parse_reconcile_clause(element);
                        resolutions.rules = parsed.rules;
                        resolutions.auto = parsed.auto;
                    }
                    Rule::fallback_stmt => {
                        let mut fb = Vec::new();
                        for stmt_pair in element.into_inner() {
                            if let Some(actual_stmt) = stmt_pair.into_inner().next()
                            {
                                fb.push(parse_statement(actual_stmt));
                            }
                        }
                        fallback = Some(fb);
                    }
                    _ => {}
                }
            }
            resolutions.fallback = fallback;
            resolutions.taking_ms = taking_ms;
            Statement::Merge {
                branches,
                target,
                resolutions,
            }
        }
        Rule::select_stmt => {
            let mut inner = pair.into_inner();
            let max_ms = inner.next().map(parse_duration_limit).unwrap_or(0);
            let mut cases = Vec::new();
            let mut timeout = None;
            let mut reconcile = None;

            for element in inner {
                match element.as_rule() {
                    Rule::select_case => {
                        let mut case_inner = element.into_inner();
                        let binding = case_inner
                            .next()
                            .map(|p| p.as_str().to_string())
                            .unwrap_or_default();
                        let source = case_inner
                            .next()
                            .map(parse_expression)
                            .unwrap_or(Expression::Literal("".into()));
                        let body = case_inner
                            .next()
                            .map(|stmt_block| {
                                stmt_block
                                    .into_inner()
                                    .filter_map(|s| s.into_inner().next())
                                    .map(parse_statement)
                                    .collect()
                            })
                            .unwrap_or_default();
                        cases.push(SelectCase {
                            binding,
                            source,
                            body,
                        });
                    }
                    Rule::timeout_clause => {
                        if let Some(block) = element.into_inner().next() {
                            let body = block
                                .into_inner()
                                .filter_map(|s| s.into_inner().next())
                                .map(parse_statement)
                                .collect();
                            timeout = Some(body);
                        }
                    }
                    Rule::reconcile_clause => {
                        reconcile = Some(parse_reconcile_clause(element));
                    }
                    _ => {}
                }
            }
            Statement::Select {
                max_ms,
                cases,
                timeout,
                reconcile,
            }
        }
        Rule::split_map_stmt => {
            let mut inner = pair.into_inner();
            let item_name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mode = inner.next().map(|p| p.as_str()).unwrap_or("consume");
            let source = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mode_enum = match mode {
                "consume" => ParamMode::Consume,
                "clone" => ParamMode::Clone,
                "decay" => ParamMode::Decay,
                _ => ParamMode::Peek,
            };

            let mut body = Vec::new();
            let mut reconcile = None;

            for next in inner {
                match next.as_rule() {
                    Rule::statement => {
                        if let Some(actual_stmt) = next.into_inner().next() {
                            body.push(parse_statement(actual_stmt));
                        }
                    }
                    Rule::reconcile_clause => {
                        reconcile = Some(parse_reconcile_clause(next));
                    }
                    _ => {}
                }
            }

            Statement::SplitMap {
                item_name,
                mode: mode_enum,
                source,
                body,
                reconcile,
            }
        }
        Rule::yield_stmt => {
            let expr = pair.into_inner().next().map(parse_expression);
            Statement::Yield(expr)
        }
        Rule::return_stmt => {
            let expr = pair.into_inner().next().map(parse_expression);
            Statement::Return(expr)
        }
        Rule::break_stmt => Statement::Break,
        Rule::using_stmt => {
            let mut inner = pair.into_inner();
            let binding = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let resource = inner
                .next()
                .map(parse_expression)
                .unwrap_or(Expression::Null);
            let mut body = Vec::new();
            if let Some(block_pair) = inner.next() {
                for stmt_pair in block_pair.into_inner() {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::Using {
                binding,
                resource,
                body,
            }
        }
        _ => unreachable!(),
    }
}
