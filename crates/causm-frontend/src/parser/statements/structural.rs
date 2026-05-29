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
            let mut params = Vec::new();
            let mut return_type = None;
            let mut taking_ms: Option<u64> = None;
            let mut taking_cycles: Option<u64> = None;
            let mut body = Vec::new();

            let routine_str = pair.as_str().to_string();
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            for current in inner {
                match current.as_rule() {
                    Rule::param_decl_list => {
                        for p in current.into_inner() {
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
                    Rule::amount => {
                        let val = current.as_str().parse::<u64>().unwrap_or(0);
                        if routine_str.contains(&format!("{} ms", val))
                            || routine_str.contains(&format!("{}ms", val))
                        {
                            taking_ms = Some(val);
                        } else if routine_str.contains(&format!("{} cycles", val))
                            || routine_str.contains(&format!("{}cycles", val))
                        {
                            taking_cycles = Some(val);
                        }
                    }
                    Rule::statement => {
                        if let Some(s) = current.into_inner().next() {
                            body.push(parse_statement(s));
                        }
                    }
                    _ => {
                        if current.as_str() == "_" {
                            taking_ms = None;
                        }
                    }
                }
            }

            Statement::RoutineDef {
                name,
                params,
                return_type,
                taking_ms,
                taking_cycles,
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
            Statement::Reset {
                target,
                anchor_name,
            }
        }
        _ => unreachable!(),
    }
}
