use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::*;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;

pub fn parse_structural_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::timeline_block => {
            let mut inner = pair.into_inner();
            let time_coord_pair = inner.next().expect("Timeline missing time");
            let time_pair = time_coord_pair
                .into_inner()
                .next()
                .expect("Invalid time structure");

            let time = match time_pair.as_rule() {
                Rule::absolute_time => TimeCoordinate::Global(
                    time_pair.as_str().replace("ms", "").parse().unwrap_or(0),
                ),
                Rule::relative_time => TimeCoordinate::Relative(
                    time_pair
                        .as_str()
                        .replace("+", "")
                        .replace("ms", "")
                        .parse()
                        .unwrap_or(0),
                ),
                Rule::branch_name => {
                    TimeCoordinate::Branch(time_pair.as_str().to_string())
                }
                _ => TimeCoordinate::Global(0),
            };

            let mut statements = Vec::new();
            if let Some(block_inner) = inner.next() {
                for stmt_pair in block_inner.into_inner() {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        statements.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::RelativisticBlock {
                time,
                body: statements,
            }
        }
        Rule::isolate_stmt => {
            let inner = pair.into_inner();
            let mut name = None;
            let mut manifest = Manifest::default();
            let mut body = Vec::new();
            for current in inner {
                match current.as_rule() {
                    Rule::identifier => name = Some(current.as_str().to_string()),
                    Rule::manifest => manifest = parse_manifest(current),
                    Rule::statement => {
                        if let Some(s) = current.into_inner().next() {
                            body.push(parse_statement(s));
                        }
                    }
                    _ => {}
                }
            }
            Statement::Isolate(IsolateBlock {
                name,
                manifest,
                body,
            })
        }
        Rule::routine_stmt => {
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut params = Vec::new();
            let mut return_type = None;
            let mut taking_ms: Option<u64> = None;
            let mut state_constraint = None;
            let mut body = Vec::new();

            for current in inner {
                match current.as_rule() {
                    Rule::method_receiver => {
                        let mut decl = current.into_inner();
                        if let Some(mode) = decl.next() {
                            let mode = match mode.as_str() {
                                "consume" => ParamMode::Consume,
                                "clone" => ParamMode::Clone,
                                "decay" => ParamMode::Decay,
                                _ => ParamMode::Peek,
                            };
                            params.push(ParamDecl {
                                mode,
                                name: "self".to_string(),
                                typ: None,
                            });
                        }
                    }
                    Rule::param_decl | Rule::param_decl_list => {
                        let pairs_to_process: Vec<_> =
                            if current.as_rule() == Rule::param_decl {
                                vec![current]
                            } else {
                                current.into_inner().collect()
                            };
                        for p in pairs_to_process {
                            let mut decl = p.into_inner();
                            if let Some(mode) = decl.next() {
                                if let Some(param_name) = decl.next() {
                                    let param_type = decl
                                        .next()
                                        .and_then(|tp| tp.into_inner().next())
                                        .map(parse_type_name);
                                    let mode = match mode.as_str() {
                                        "consume" => ParamMode::Consume,
                                        "clone" => ParamMode::Clone,
                                        "decay" => ParamMode::Decay,
                                        _ => ParamMode::Peek,
                                    };
                                    params.push(ParamDecl {
                                        mode,
                                        name: param_name.as_str().to_string(),
                                        typ: param_type,
                                    });
                                }
                            }
                        }
                    }
                    Rule::return_annotation => {
                        if let Some(typ) = current.into_inner().next() {
                            return_type = Some(parse_type_name(typ));
                        }
                    }
                    Rule::duration_limit => {
                        if current.as_str().contains("_") {
                            taking_ms = None;
                        } else {
                            if let Some(amount_pair) = current
                                .into_inner()
                                .find(|p| p.as_rule() == Rule::amount)
                            {
                                taking_ms = amount_pair.as_str().parse::<u64>().ok();
                            }
                        }
                    }
                    Rule::state_constraint => {
                        let mut sc_inner = current.into_inner();
                        let var_name = sc_inner.next().unwrap().as_str().to_string();
                        let state_name =
                            sc_inner.next().unwrap().as_str().to_string();
                        state_constraint = Some((var_name, state_name));
                    }
                    Rule::statement => {
                        if let Some(s) = current.into_inner().next() {
                            body.push(parse_statement(s));
                        }
                    }
                    _ => {}
                }
            }

            Statement::RoutineDef {
                name,
                params,
                return_type,
                taking_ms,
                state_constraint,
                body,
            }
        }
        Rule::require_decl => Statement::Capability(parse_capability(pair)),
        Rule::anchor_stmt => Statement::Anchor(
            pair.into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default(),
        ),
        Rule::rewind_stmt => Statement::Rewind(
            pair.into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default(),
        ),
        Rule::reset_stmt => {
            let mut inner = pair.into_inner();
            let target = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let anchor_name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Statement::AcausalReset {
                target,
                anchor_name,
            }
        }
        Rule::directive_stmt => {
            let mut inner = pair.into_inner().peekable();
            let mut directives = Vec::new();
            while let Some(next_pair) = inner.peek() {
                if next_pair.as_rule() == Rule::timeline_directive {
                    let dir_pair = inner.next().unwrap();
                    let dir = match dir_pair.as_str() {
                        "@no_z3" => BlockDirective::NoZ3,
                        "@chaos" => BlockDirective::Chaos,
                        "@deterministic" => BlockDirective::Deterministic,
                        _ => continue,
                    };
                    directives.push(dir);
                } else {
                    break;
                }
            }

            let mut body = Vec::new();
            if let Some(block_inner) = inner.next() {
                for stmt_pair in block_inner.into_inner() {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }

            Statement::DirectiveBlock { directives, body }
        }
        _ => unreachable!(),
    }
}
