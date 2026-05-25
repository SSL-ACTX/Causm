use crate::parser::statements::parse_statement;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;

pub fn parse_temporal_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::watchdog_stmt => {
            let mut inner = pair.into_inner();
            let target = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let timeout_ms = inner
                .next()
                .map(|p| p.as_str().parse::<u64>().unwrap_or(0))
                .unwrap_or(0);

            let mut recovery = Vec::new();
            if let Some(recovery_pair) = inner.next() {
                for stmt_pair in recovery_pair.into_inner() {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        recovery.push(parse_statement(actual_stmt));
                    }
                }
            }

            Statement::Watchdog {
                target,
                timeout_ms,
                recovery,
            }
        }
        Rule::assert_time_stmt => {
            let mut inner = pair.into_inner();
            let op_str = inner.next().map(|p| p.as_str()).unwrap_or("==");
            let operator = match op_str {
                "==" => BinaryOperator::Eq,
                "!=" => BinaryOperator::Neq,
                "<" => BinaryOperator::Lt,
                ">" => BinaryOperator::Gt,
                "<=" => BinaryOperator::Le,
                ">=" => BinaryOperator::Ge,
                _ => BinaryOperator::Eq,
            };
            let limit_ms = inner
                .next()
                .and_then(|p| p.as_str().parse::<u64>().ok())
                .unwrap_or(0);

            let mut fallback = None;
            if let Some(fb_pair) = inner.next() {
                let mut body = Vec::new();
                for stmt_pair in fb_pair.into_inner() {
                    if stmt_pair.as_rule() == Rule::statement {
                        if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                            body.push(parse_statement(actual_stmt));
                        }
                    } else if stmt_pair.as_rule() == Rule::statement_block {
                        for inner_stmt in stmt_pair.into_inner() {
                            if inner_stmt.as_rule() == Rule::statement {
                                if let Some(actual_stmt) =
                                    inner_stmt.into_inner().next()
                                {
                                    body.push(parse_statement(actual_stmt));
                                }
                            }
                        }
                    }
                }
                fallback = Some(body);
            }

            Statement::AssertTime {
                operator,
                limit_ms,
                fallback,
            }
        }
        Rule::slice_stmt => {
            let amount = pair
                .into_inner()
                .next()
                .and_then(|p| p.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            Statement::Slice {
                milliseconds: amount,
            }
        }
        Rule::await_stmt => {
            let target = pair
                .into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Statement::Await(target)
        }
        Rule::await_chan_stmt => {
            let target = pair
                .into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Statement::AwaitChan(target)
        }
        Rule::lease_stmt => {
            let mut inner = pair.into_inner();
            let binding = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let source = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let duration_ms = inner
                .next()
                .and_then(|p| p.as_str().parse::<u64>().ok())
                .unwrap_or(0);
            let mut body = Vec::new();
            if let Some(block) = inner.next() {
                for stmt_pair in block.into_inner() {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::Lease {
                binding,
                source,
                duration_ms,
                body,
            }
        }
        _ => unreachable!(),
    }
}
