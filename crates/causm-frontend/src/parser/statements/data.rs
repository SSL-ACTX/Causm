use crate::parser::expressions::parse_expression;
use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::*;
use crate::parser::Rule;
use causm_core::*;
use pest::iterators::Pair;
use std::collections::HashMap;

pub fn parse_data_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::assignment_stmt => {
            let mut inner = pair.into_inner();
            let mut mutable = false;
            if let Some(first) = inner.peek() {
                if first.as_str() == "mut" {
                    mutable = true;
                    inner.next();
                }
            }

            let target = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut var_type: Option<TypeName> = None;
            if let Some(next_pair) = inner.peek() {
                if next_pair.as_rule() == Rule::type_annotation {
                    let type_annotation_pair = inner.next().unwrap();
                    if let Some(type_name_pair) =
                        type_annotation_pair.into_inner().next()
                    {
                        var_type = Some(parse_type_name(type_name_pair));
                    }
                }
            }

            let expr = inner
                .next()
                .map(parse_expression)
                .unwrap_or(Expression::Literal("void".into()));

            Statement::Assignment {
                target,
                mutable,
                var_type,
                expr,
            }
        }
        Rule::type_decl => {
            let mut inner = pair.into_inner().peekable();
            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

            if let Some(p) = inner.peek() {
                if p.as_rule() == Rule::generic_param_list {
                    inner.next(); // Consume generic_param_list
                }
            }

            let mut extends = None;
            if let Some(p) = inner.peek() {
                if p.as_rule() == Rule::base_type || p.as_rule() == Rule::identifier
                {
                    let text = p.as_str();
                    let base_name =
                        text.split('<').next().unwrap_or(text).trim().to_string();
                    extends = Some(base_name);
                }
            }
            if extends.is_some() {
                inner.next(); // Consume the extends base_type/identifier
            }

            let mut decay_after_ms = None;
            let mut scoped_branch = None;
            let mut fields = HashMap::new();

            for current in inner {
                match current.as_rule() {
                    Rule::decay_opt => {
                        decay_after_ms = current
                            .into_inner()
                            .next()
                            .and_then(|p| p.as_str().parse::<u64>().ok());
                    }
                    Rule::scoped_opt => {
                        scoped_branch = current
                            .into_inner()
                            .next()
                            .map(|p| p.as_str().to_string());
                    }
                    Rule::type_field_list => {
                        for field_pair in current.into_inner() {
                            let is_const = field_pair
                                .as_str()
                                .trim_start()
                                .starts_with("const");
                            let mut kv = field_pair.into_inner();
                            if let (Some(id), Some(type_name_pair)) =
                                (kv.next(), kv.next())
                            {
                                let field_type = parse_type_name(type_name_pair);
                                let mut default_value = None;
                                if let Some(expr_pair) = kv.next() {
                                    default_value = Some(
                                        crate::parser::expressions::parse_expression(
                                            expr_pair,
                                        ),
                                    );
                                }
                                fields.insert(
                                    id.as_str().to_string(),
                                    TypeFieldDef {
                                        typ: field_type,
                                        is_const,
                                        default_value,
                                    },
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Statement::TypeDecl {
                name,
                extends,
                fields,
                decay_after_ms,
                scoped_branch,
            }
        }
        Rule::decay_handler_stmt => {
            let mut inner = pair.into_inner();
            let type_name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut body = Vec::new();
            for stmt_pair in inner {
                if stmt_pair.as_rule() == Rule::statement {
                    if let Some(actual_stmt) = stmt_pair.into_inner().next() {
                        body.push(parse_statement(actual_stmt));
                    }
                }
            }
            Statement::DecayHandler { type_name, body }
        }
        Rule::field_update_stmt => {
            let mut inner = pair.into_inner();
            let target_expr = parse_expression(inner.next().unwrap());
            let value = parse_expression(inner.next().unwrap());
            if let Expression::FieldAccess { target, field } = target_expr {
                Statement::FieldUpdate {
                    target: *target,
                    field,
                    value,
                }
            } else if let Expression::IndexAccess { target, index } = target_expr {
                if let Expression::Literal(s) = *index {
                    Statement::FieldUpdate {
                        target: *target,
                        field: s,
                        value,
                    }
                } else {
                    Statement::Expression(Expression::BinaryOp {
                        left: Box::new(Expression::IndexAccess {
                            target,
                            index: Box::new(*index),
                        }),
                        op: BinaryOperator::Eq,
                        right: Box::new(value),
                    })
                }
            } else if let Expression::Identifier(name) = target_expr {
                Statement::Assignment {
                    target: name,
                    mutable: false,
                    var_type: None,
                    expr: value,
                }
            } else {
                Statement::Expression(Expression::BinaryOp {
                    left: Box::new(target_expr),
                    op: BinaryOperator::Eq,
                    right: Box::new(value),
                })
            }
        }
        Rule::interface_decl => {
            let mut inner = pair.into_inner().peekable();
            let name = inner.next().unwrap().as_str().to_string();
            let mut extends = Vec::new();
            let mut methods = Vec::new();

            if let Some(p) = inner.peek() {
                if p.as_rule() == Rule::generic_param_list {
                    inner.next(); // Consume generic_param_list
                }
            }

            let parse_method = |m_pair: Pair<Rule>| -> causm_core::InterfaceMethod {
                let mut m_inner = m_pair.into_inner();
                let m_name = m_inner.next().unwrap().as_str().to_string();

                let mut params = Vec::new();
                let mut return_type = None;
                let mut taking_ms = None;
                let mut default_body = None;
                let mut state_constraint = None;

                for opt in m_inner {
                    match opt.as_rule() {
                        Rule::param_decl_list => {
                            for p_pair in opt.into_inner() {
                                let mut p_inner = p_pair.into_inner();
                                let mode_str = p_inner.next().unwrap().as_str();
                                let mode = match mode_str {
                                    "consume" => causm_core::ParamMode::Consume,
                                    "clone" => causm_core::ParamMode::Clone,
                                    "decay" => causm_core::ParamMode::Decay,
                                    "peek" => causm_core::ParamMode::Peek,
                                    "lease" => causm_core::ParamMode::Lease,
                                    _ => causm_core::ParamMode::Peek,
                                };
                                let p_name =
                                    p_inner.next().unwrap().as_str().to_string();
                                let typ = p_inner.next().map(parse_type_name);
                                params.push(causm_core::ParamDecl {
                                    mode,
                                    name: p_name,
                                    typ,
                                });
                            }
                        }
                        Rule::return_annotation => {
                            let type_name_pair = opt.into_inner().next().unwrap();
                            return_type = Some(parse_type_name(type_name_pair));
                        }
                        Rule::duration_limit => {
                            taking_ms = opt
                                .into_inner()
                                .next()
                                .and_then(|p| p.as_str().parse::<u64>().ok());
                        }
                        Rule::state_constraint => {
                            let mut sc_inner = opt.into_inner();
                            let var_name =
                                sc_inner.next().unwrap().as_str().to_string();
                            let state_name =
                                sc_inner.next().unwrap().as_str().to_string();
                            state_constraint = Some((var_name, state_name));
                        }
                        Rule::statement_block => {
                            let body = opt
                                .into_inner()
                                .filter_map(|stmt_pair| {
                                    stmt_pair.into_inner().next()
                                })
                                .map(parse_statement)
                                .collect();
                            default_body = Some(body);
                        }
                        _ => {}
                    }
                }

                causm_core::InterfaceMethod {
                    name: m_name,
                    params,
                    return_type,
                    taking_ms,
                    default_body,
                    state_constraint,
                }
            };

            for next_pair in inner {
                match next_pair.as_rule() {
                    Rule::interface_sum => {
                        for term in next_pair.into_inner() {
                            match term.as_rule() {
                                Rule::interface_term => {
                                    let term_inner = term.into_inner();
                                    for item in term_inner {
                                        if item.as_rule() == Rule::identifier {
                                            let id_str = item.as_str();
                                            if id_str != "interface" {
                                                extends.push(id_str.to_string());
                                            }
                                        } else if item.as_rule()
                                            == Rule::interface_item
                                        {
                                            if let Some(inner_item) =
                                                item.into_inner().next()
                                            {
                                                if inner_item.as_rule()
                                                    == Rule::interface_method
                                                {
                                                    methods.push(parse_method(
                                                        inner_item,
                                                    ));
                                                }
                                            }
                                        } else if item.as_rule()
                                            == Rule::interface_method
                                        {
                                            methods.push(parse_method(item));
                                        }
                                    }
                                }
                                Rule::identifier => {
                                    extends.push(term.as_str().to_string());
                                }
                                _ => {}
                            }
                        }
                    }
                    Rule::interface_item => {
                        if let Some(inner_item) = next_pair.into_inner().next() {
                            if inner_item.as_rule() == Rule::interface_method {
                                methods.push(parse_method(inner_item));
                            }
                        }
                    }
                    Rule::interface_method => {
                        methods.push(parse_method(next_pair));
                    }
                    _ => {}
                }
            }
            Statement::InterfaceDecl {
                name,
                extends,
                methods,
            }
        }
        _ => unreachable!(),
    }
}
