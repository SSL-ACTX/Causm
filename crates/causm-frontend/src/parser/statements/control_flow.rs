use crate::parser::expressions::parse_expression;
use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::*;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;
use std::collections::HashMap;

pub fn parse_control_flow_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
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
            let else_branch = inner.next().map(|else_pair| {
                else_pair
                    .into_inner()
                    .filter_map(|stmt_pair| stmt_pair.into_inner().next())
                    .map(parse_statement)
                    .collect()
            });
            let reconcile_rules = if let Some(reconcile_pair) = inner.next() {
                let mut rules = HashMap::new();
                let is_auto = reconcile_pair.as_str().contains("auto");
                for child in reconcile_pair.into_inner() {
                    if child.as_rule() == Rule::resolution_rules {
                        for rule in child.into_inner() {
                            let mut r_inner = rule.into_inner();
                            if let (Some(k), Some(v)) =
                                (r_inner.next(), r_inner.next())
                            {
                                let value = v.as_str();
                                let strat = if value == "first_wins" {
                                    ResolutionStrategy::FirstWins
                                } else if value == "decay" {
                                    ResolutionStrategy::Decay
                                } else if let Some(inner) =
                                    value.strip_prefix("priority(")
                                {
                                    if let Some(branch_name) =
                                        inner.strip_suffix(")")
                                    {
                                        ResolutionStrategy::Priority(
                                            branch_name.to_string(),
                                        )
                                    } else {
                                        ResolutionStrategy::Custom(value.to_string())
                                    }
                                } else {
                                    ResolutionStrategy::Priority(value.to_string())
                                };
                                rules.insert(k.as_str().to_string(), strat);
                            }
                        }
                    }
                }
                Some(MergeResolution {
                    rules,
                    auto: is_auto,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };

            Statement::If {
                condition,
                then_branch,
                else_branch,
                reconcile: reconcile_rules,
            }
        }
        Rule::loop_stmt => {
            let mut inner = pair.into_inner();
            let first = inner.next();
            if let Some(first) = first {
                if first.as_rule() == Rule::amount {
                    let max_value = first.as_str().parse::<u64>().unwrap_or(0);
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
                    if first.as_rule() == Rule::statement_block {
                        for stmt_pair in first.into_inner() {
                            if let Some(actual_stmt) = stmt_pair.into_inner().next()
                            {
                                body.push(parse_statement(actual_stmt));
                            }
                        }
                    } else if first.as_rule() == Rule::statement {
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
                Statement::Loop {
                    max_ms: 0,
                    body: Vec::new(),
                }
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
                    "consume" => ForMode::Consume,
                    _ => ForMode::Clone,
                })
                .unwrap_or(ForMode::Consume);
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
                        let amount = next
                            .into_inner()
                            .next()
                            .and_then(|p| p.as_str().parse::<u64>().ok());
                        pacing_ms = amount;
                    }
                    Rule::max_opt => {
                        let amount = next
                            .into_inner()
                            .next_back()
                            .and_then(|p| p.as_str().parse::<u64>().ok());
                        max_ms = amount;
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
                    Rule::merge_taking_opt => {
                        let ms = element
                            .into_inner()
                            .next()
                            .and_then(|p| p.as_str().parse::<u64>().ok());
                        taking_ms = ms;
                    }
                    Rule::resolution_rules => {
                        let mut rules = HashMap::new();
                        for rule in element.into_inner() {
                            let mut r_inner = rule.into_inner();
                            if let (Some(k), Some(v)) =
                                (r_inner.next(), r_inner.next())
                            {
                                let strat = parse_resolution_strategy(v);
                                rules.insert(k.as_str().to_string(), strat);
                            }
                        }
                        resolutions.rules = rules;
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
            let max_ms = inner
                .next()
                .and_then(|p| p.as_str().parse::<u64>().ok())
                .unwrap_or(0);
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
                    Rule::resolution_rules => {
                        let mut rules = HashMap::new();
                        for rule in element.into_inner() {
                            let mut r_inner = rule.into_inner();
                            if let (Some(k), Some(v)) =
                                (r_inner.next(), r_inner.next())
                            {
                                let value = v.as_str();
                                let strat = if value == "first_wins" {
                                    ResolutionStrategy::FirstWins
                                } else if value == "decay" {
                                    ResolutionStrategy::Decay
                                } else if let Some(inner) =
                                    value.strip_prefix("priority(")
                                {
                                    if let Some(branch_name) =
                                        inner.strip_suffix(")")
                                    {
                                        ResolutionStrategy::Priority(
                                            branch_name.to_string(),
                                        )
                                    } else {
                                        ResolutionStrategy::Custom(value.to_string())
                                    }
                                } else {
                                    ResolutionStrategy::Priority(value.to_string())
                                };
                                rules.insert(k.as_str().to_string(), strat);
                            }
                        }
                        reconcile = Some(MergeResolution {
                            rules,
                            auto: false,
                            fallback: None,
                            taking_ms: None,
                        });
                    }
                    Rule::reconcile_clause if element.as_str().contains("auto") => {
                        reconcile = Some(MergeResolution {
                            rules: HashMap::new(),
                            auto: true,
                            fallback: None,
                            taking_ms: None,
                        });
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
            let mode_enum = if mode == "clone" {
                ForMode::Clone
            } else {
                ForMode::Consume
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
                    Rule::resolution_rules => {
                        let mut rules = HashMap::new();
                        for rule in next.into_inner() {
                            let mut r_inner = rule.into_inner();
                            if let (Some(k), Some(v)) =
                                (r_inner.next(), r_inner.next())
                            {
                                let strat = match v.as_str() {
                                    "first_wins" => ResolutionStrategy::FirstWins,
                                    "decay" => ResolutionStrategy::Decay,
                                    p if p.starts_with("priority(") => {
                                        let name = p
                                            .trim_start_matches("priority(")
                                            .trim_end_matches(")");
                                        ResolutionStrategy::Priority(
                                            name.to_string(),
                                        )
                                    }
                                    _ => ResolutionStrategy::Custom(
                                        v.as_str().to_string(),
                                    ),
                                };
                                rules.insert(k.as_str().to_string(), strat);
                            }
                        }
                        reconcile = Some(MergeResolution {
                            rules,
                            auto: false,
                            fallback: None,
                            taking_ms: None,
                        });
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
            let item = pair
                .into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Statement::Yield(item)
        }
        Rule::break_stmt => Statement::Break,
        _ => unreachable!(),
    }
}
