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
        Rule::import_stmt => {
            let mut inner = pair.into_inner();
            let raw_path = inner.next().unwrap().as_str().replace('"', "");
            let alias = inner.next().map(|p| p.as_str().to_string());
            Statement::Import {
                path: raw_path,
                alias,
            }
        }
        Rule::foreign_block_stmt => {
            let full_span = Span {
                start: pair.as_span().start(),
                end: pair.as_span().end(),
            };
            let mut inner = pair.into_inner();
            let lib_name = inner.next().unwrap().as_str().replace('"', "");
            let abi = inner.next().unwrap().as_str().replace('"', "");
            let mut routines = Vec::new();
            for item in inner {
                if item.as_rule() == Rule::foreign_routine {
                    let mut r_inner = item.into_inner();
                    let mut name_pair = r_inner.next().unwrap();
                    if name_pair.as_rule() == Rule::pub_opt {
                        name_pair = r_inner.next().unwrap();
                    }
                    let name = name_pair.as_str().to_string();
                    let mut params = Vec::new();
                    let mut return_type = None;
                    let mut taking_ms = None;
                    for p in r_inner {
                        match p.as_rule() {
                            Rule::param_decl | Rule::param_decl_list => {
                                let pairs: Vec<_> =
                                    if p.as_rule() == Rule::param_decl {
                                        vec![p]
                                    } else {
                                        p.into_inner().collect()
                                    };
                                for pd in pairs {
                                    let decl = pd.into_inner();
                                    let mut mode = ParamMode::Peek;
                                    let mut p_name = String::new();
                                    let mut typ = None;
                                    for sub in decl {
                                        match sub.as_rule() {
                                            Rule::param_mode => {
                                                mode = match sub.as_str() {
                                                    "consume" => ParamMode::Consume,
                                                    "clone" => ParamMode::Clone,
                                                    "decay" => ParamMode::Decay,
                                                    "lease" => ParamMode::Lease,
                                                    _ => ParamMode::Peek,
                                                };
                                            }
                                            Rule::identifier => {
                                                p_name = sub.as_str().to_string();
                                            }
                                            Rule::type_annotation => {
                                                if let Some(t_pair) =
                                                    sub.into_inner().next()
                                                {
                                                    typ = Some(super::utils::parse_type_name(t_pair));
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    params.push(ParamDecl {
                                        mode,
                                        name: p_name,
                                        typ,
                                    });
                                }
                            }
                            Rule::return_annotation => {
                                if let Some(t_pair) = p.into_inner().next() {
                                    return_type =
                                        Some(super::utils::parse_type_name(t_pair));
                                }
                            }
                            Rule::duration_limit => {
                                let str_val = p.as_str();
                                let digits: String = str_val
                                    .chars()
                                    .filter(|c| c.is_ascii_digit())
                                    .collect();
                                taking_ms = digits.parse::<u64>().ok();
                            }
                            _ => {}
                        }
                    }
                    routines.push(SpannedStatement {
                        stmt: Statement::RoutineDef {
                            name,
                            params,
                            return_type,
                            taking_ms,
                            state_constraint: None,
                            body: Vec::new(),
                        },
                        span: full_span.clone(),
                    });
                }
            }
            Statement::ForeignBlock {
                lib_name,
                abi,
                routines,
            }
        }
        Rule::from_import_stmt => {
            let mut inner = pair.into_inner();
            let raw_path = inner.next().unwrap().as_str().replace('"', "");
            let mut symbols = Vec::new();
            if let Some(list) = inner.next() {
                for sym_pair in list.into_inner() {
                    if sym_pair.as_rule() == Rule::wildcard_symbol {
                        symbols.push(("*".to_string(), None));
                    } else {
                        let mut sym_inner = sym_pair.into_inner();
                        let name = sym_inner.next().unwrap().as_str().to_string();
                        let sym_alias =
                            sym_inner.next().map(|p| p.as_str().to_string());
                        symbols.push((name, sym_alias));
                    }
                }
            }
            Statement::FromImport {
                path: raw_path,
                symbols,
            }
        }
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
