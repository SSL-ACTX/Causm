use crate::parser::expressions::parse_expression;
use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::parse_duration_limit;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;

pub fn parse_misc_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
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
                    let mut required_capabilities = Vec::new();
                    for p in r_inner {
                        match p.as_rule() {
                            Rule::requires_clause => {
                                required_capabilities
                                    .extend(super::utils::parse_requires_clause(p));
                            }
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
                                if str_val.contains('_') || str_val.contains('?') {
                                    taking_ms = None;
                                } else {
                                    taking_ms =
                                        Some(super::utils::parse_duration_limit(p));
                                }
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
                            required_capabilities,
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
        Rule::print_stmt => {
            // print_stmt → print_arg_list → expression*
            let arg_list = pair.into_inner().next().unwrap();
            let args: Vec<Expression> =
                arg_list.into_inner().map(parse_expression).collect();
            Statement::Print(args)
        }
        Rule::debug_stmt => {
            let mut inner = pair.into_inner();
            let expr = inner
                .next()
                .map(parse_expression)
                .unwrap_or(Expression::Literal("".into()));
            Statement::Debug(expr)
        }
        _ => unreachable!(),
    }
}

/// Parse `macro name!( $p1:kind, $p2:kind => { body } )` into `Statement::MacroDef`.
pub fn parse_macro_def(pair: Pair<Rule>) -> Statement {
    let mut inner = pair.into_inner();
    let name = inner.next().expect("macro name").as_str().to_string();

    let mut params: Vec<MacroParam> = Vec::new();
    for child in inner.by_ref() {
        match child.as_rule() {
            Rule::macro_param_list => {
                for param_pair in child.into_inner() {
                    if param_pair.as_rule() == Rule::macro_param {
                        let mut pi = param_pair.into_inner();
                        let pname = pi.next().unwrap().as_str().to_string();
                        let kind_str =
                            pi.next().map(|p| p.as_str()).unwrap_or("expr");
                        let kind = match kind_str {
                            "ident" => MacroParamKind::Ident,
                            "type" => MacroParamKind::Type,
                            "literal" => MacroParamKind::Literal,
                            _ => MacroParamKind::Expr,
                        };
                        params.push(MacroParam { name: pname, kind });
                    }
                }
            }
            Rule::macro_body => {
                // body_template is the raw content inside the braces
                let body_template = child
                    .into_inner()
                    .next()
                    .map(|p| p.as_str().to_string())
                    .unwrap_or_default();
                return Statement::MacroDef {
                    name,
                    params,
                    body_template,
                };
            }
            _ => {}
        }
    }
    Statement::MacroDef {
        name,
        params,
        body_template: String::new(),
    }
}

/// Parse `name!(arg1, arg2)` into `Statement::Expression(MacroCall)`.
/// The actual expansion happens in the macro_expand pass — here we emit a
/// sentinel `Expression::Call` with name `__macro_call__name` so the expander
/// can detect and replace it. The raw arg strings go in as `Expression::Literal`.
pub fn parse_macro_call(pair: Pair<Rule>) -> Statement {
    let mut inner = pair.into_inner();
    let name = inner.next().expect("macro call name").as_str().to_string();
    let mut args: Vec<Expression> = Vec::new();
    if let Some(arg_list) = inner.next() {
        for arg_pair in arg_list.into_inner() {
            if arg_pair.as_rule() == Rule::macro_call_arg {
                args.push(Expression::Literal(arg_pair.as_str().trim().to_string()));
            }
        }
    }
    // Encode as a synthetic Call node; the macro_expand pass replaces these
    Statement::Expression(Expression::Call {
        routine: format!("__macro_call__{}", name),
        args,
    })
}
