use crate::parser::expressions::parse_expression;
use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::parse_duration_limit;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;

pub fn parse_misc_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::open_chan_stmt => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let capacity = inner
                .next()
                .map(|p| p.as_str().parse::<usize>().unwrap_or(1))
                .unwrap_or(1);
            let decay_after_ms = inner.next().and_then(|p| {
                if p.as_rule() == Rule::decay_opt {
                    p.into_inner()
                        .next()
                        .and_then(|p2| p2.as_str().parse::<u64>().ok())
                } else {
                    None
                }
            });
            Statement::ChannelOpen {
                name,
                capacity,
                decay_after_ms,
            }
        }
        Rule::chan_send_stmt => {
            let mut inner = pair.into_inner();
            let chan_id = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let value_id = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Statement::ChannelSend { chan_id, value_id }
        }
        Rule::commit_stmt => {
            let mut body = Vec::new();
            for stmt_pair in pair.into_inner() {
                if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                    body.push(parse_statement(actual_stmt));
                }
            }
            Statement::Commit(body)
        }
        Rule::speculate_stmt => {
            let mut inner = pair.into_inner();
            let max_ms = inner.next().map(parse_duration_limit).unwrap_or(0);
            let mut body = Vec::new();
            let mut fallback = None;

            for element in inner {
                match element.as_rule() {
                    Rule::statement => {
                        if let Some(actual_stmt) = element.into_inner().next() {
                            body.push(parse_statement(actual_stmt));
                        }
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

            Statement::Speculate {
                max_ms,
                body,
                fallback,
            }
        }
        Rule::collapse_stmt => Statement::Collapse,
        Rule::speculation_mode_stmt => {
            let mode_str = pair
                .into_inner()
                .next()
                .map(|p| p.as_str())
                .unwrap_or("selective");
            let mode = match mode_str {
                "full" => SpeculationCommitMode::Full,
                _ => SpeculationCommitMode::Selective,
            };
            Statement::SpeculationMode(mode)
        }
        Rule::network_request_stmt => {
            let domain = pair
                .into_inner()
                .next()
                .map(|p| p.as_str().replace("\"", ""))
                .unwrap_or_default();
            Statement::NetworkRequest { domain }
        }
        Rule::print_stmt => {
            let mut inner = pair.into_inner();
            let expr = inner
                .next()
                .map(parse_expression)
                .unwrap_or(Expression::Literal("".into()));
            Statement::Print(expr)
        }
        Rule::debug_stmt => {
            let mut inner = pair.into_inner();
            let expr = inner
                .next()
                .map(parse_expression)
                .unwrap_or(Expression::Literal("".into()));
            Statement::Debug(expr)
        }
        Rule::inspect_stmt => {
            let mut inner = pair.into_inner();
            let binding = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let target = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut body = Vec::new();
            if let Some(block) = inner.next() {
                for stmt_pair in block.into_inner() {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::Inspect {
                binding,
                target,
                body,
            }
        }
        _ => unreachable!(),
    }
}
