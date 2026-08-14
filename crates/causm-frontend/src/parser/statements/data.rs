use crate::parser::expressions::parse_expression;
use crate::parser::statements::parse_statement;
use crate::parser::statements::utils::*;
use crate::parser::Rule;
use causm_core::types::AutoDropSpec;
use causm_core::*;
use pest::iterators::Pair;
use std::collections::HashMap;

pub fn parse_data_stmt(pair: Pair<Rule>) -> Statement {
    match pair.as_rule() {
        Rule::assignment_stmt => {
            let mut inner = pair.into_inner().peekable();
            let mut mutable = false;
            if let Some(first) = inner.peek() {
                if first.as_str() == "mut" {
                    mutable = true;
                    inner.next();
                }
            }

            let mut lifetime = None;
            if let Some(p) = inner.peek() {
                if p.as_rule() == Rule::lifetime_annotation {
                    let lt_pair = inner.next().unwrap();
                    let s = lt_pair.as_str();
                    if s == "@valid" {
                        lifetime = Some(LifetimeAnnotation::Valid);
                    } else if s.starts_with("@decay_rate(") {
                        let dur_str = lt_pair
                            .into_inner()
                            .next()
                            .map(|p| p.as_str())
                            .unwrap_or("0ms");
                        let ms = parse_duration_to_ms(dur_str);
                        lifetime = Some(LifetimeAnnotation::DecayRate(ms));
                    } else if s.starts_with("@decayed(") {
                        let dur_str = lt_pair
                            .into_inner()
                            .next()
                            .map(|p| p.as_str())
                            .unwrap_or("0ms");
                        let ms = parse_duration_to_ms(dur_str);
                        lifetime = Some(LifetimeAnnotation::Decayed(ms));
                    }
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
                .unwrap_or(Expression::Null);

            Statement::Assignment {
                target,
                mutable,
                var_type,
                lifetime,
                expr,
            }
        }
        Rule::enum_decl => {
            let mut inner = pair.into_inner();
            let mut name_pair = inner.next().unwrap();
            if name_pair.as_rule() == Rule::pub_opt {
                name_pair = inner.next().unwrap();
            }
            let name = name_pair.as_str().to_string();
            let mut variants = Vec::new();
            if let Some(list_pair) = inner.next() {
                for variant_pair in list_pair.into_inner() {
                    let mut var_inner = variant_pair.into_inner();
                    let v_name = var_inner
                        .next()
                        .map(|p| p.as_str().to_string())
                        .unwrap_or_default();
                    let mut payload_types = Vec::new();
                    if let Some(types_pair) = var_inner.next() {
                        for t_pair in types_pair.into_inner() {
                            payload_types.push(parse_type_name(t_pair));
                        }
                    }
                    variants.push(EnumVariantDef {
                        name: v_name,
                        payload_types,
                    });
                }
            }
            Statement::EnumDecl { name, variants }
        }
        Rule::type_decl => {
            let mut inner = pair.into_inner().peekable();
            let mut name_pair = inner.next().unwrap();
            if name_pair.as_rule() == Rule::pub_opt {
                name_pair = inner.next().unwrap();
            }
            let name = name_pair.as_str().to_string();

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
            let mut auto_drop = None;
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
                    Rule::auto_drop_opt => {
                        let mut ad_inner = current.into_inner();
                        let lib_name = ad_inner
                            .next()
                            .map(|p| p.as_str().replace('"', ""))
                            .unwrap_or_default();
                        let routine_name = ad_inner
                            .next()
                            .map(|p| p.as_str().replace('"', ""))
                            .unwrap_or_default();
                        let field_name = ad_inner
                            .next()
                            .map(|p| p.as_str().to_string())
                            .unwrap_or_default();
                        auto_drop = Some(AutoDropSpec {
                            lib_name,
                            routine_name,
                            field_name,
                        });
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
                auto_drop,
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
            let mut inner = pair.clone().into_inner();
            let target_pair = inner.next().unwrap();
            let value_pair = inner.next().unwrap();

            // Descend through expression wrapper rules to find the actual inner rule.
            // Grammar: expression -> relational_expr -> ... -> primary_expr -> base_expr -> match_entropy_expr
            fn unwrap_to_inner(
                p: pest::iterators::Pair<Rule>,
            ) -> pest::iterators::Pair<Rule> {
                if p.as_rule() == Rule::match_entropy_expr
                    || p.as_rule() == Rule::match_entropy_stmt
                {
                    return p;
                }
                for child in p.clone().into_inner() {
                    let res = unwrap_to_inner(child);
                    if res.as_rule() == Rule::match_entropy_expr
                        || res.as_rule() == Rule::match_entropy_stmt
                    {
                        return res;
                    }
                }
                p
            }

            let inner_val = unwrap_to_inner(value_pair.clone());
            if inner_val.as_rule() == Rule::match_entropy_expr {
                let lhs_name = target_pair.as_str().trim().to_string();
                let mut match_stmt =
                    crate::parser::statements::entropic::parse_entropic_stmt(
                        inner_val,
                    );
                if let Statement::MatchEntropy {
                    ref mut valid_branch,
                    ref mut decayed_branch,
                    ref mut pending_branch,
                    ref mut consumed_branch,
                    ..
                } = match_stmt
                {
                    let inject = |branch: &mut Option<(
                        DecayedPattern,
                        Option<Expression>,
                        Vec<SpannedStatement>,
                    )>| {
                        if let Some((_, _, ref mut body)) = branch {
                            if body.len() == 1 {
                                if let Statement::Expression(expr) =
                                    body[0].stmt.clone()
                                {
                                    *body = vec![SpannedStatement {
                                        stmt: Statement::Assignment {
                                            target: lhs_name.clone(),
                                            mutable: false,
                                            var_type: None,
                                            lifetime: None,
                                            expr,
                                        },
                                        span: body[0].span.clone(),
                                    }];
                                }
                            }
                        }
                    };
                    let inject_consumed = |branch: &mut Option<(
                        Option<Expression>,
                        Vec<SpannedStatement>,
                    )>| {
                        if let Some((_, ref mut body)) = branch {
                            if body.len() == 1 {
                                if let Statement::Expression(expr) =
                                    body[0].stmt.clone()
                                {
                                    *body = vec![SpannedStatement {
                                        stmt: Statement::Assignment {
                                            target: lhs_name.clone(),
                                            mutable: false,
                                            var_type: None,
                                            lifetime: None,
                                            expr,
                                        },
                                        span: body[0].span.clone(),
                                    }];
                                }
                            }
                        }
                    };
                    inject(valid_branch);
                    inject(decayed_branch);
                    inject(pending_branch);
                    inject_consumed(consumed_branch);
                }
                return match_stmt;
            }

            let target_expr = parse_expression(target_pair);
            let value = parse_expression(value_pair);
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
                    lifetime: None,
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
            let mut name_pair = inner.next().unwrap();
            if name_pair.as_rule() == Rule::pub_opt {
                name_pair = inner.next().unwrap();
            }
            let name = name_pair.as_str().to_string();
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
                                let first_pair = p_inner.next().unwrap();
                                let (mode, p_name) = match first_pair.as_str() {
                                    "consume" => (
                                        causm_core::ParamMode::Consume,
                                        p_inner.next().unwrap().as_str().to_string(),
                                    ),
                                    "clone" => (
                                        causm_core::ParamMode::Clone,
                                        p_inner.next().unwrap().as_str().to_string(),
                                    ),
                                    "decay" => (
                                        causm_core::ParamMode::Decay,
                                        p_inner.next().unwrap().as_str().to_string(),
                                    ),
                                    "peek" => (
                                        causm_core::ParamMode::Peek,
                                        p_inner.next().unwrap().as_str().to_string(),
                                    ),
                                    "lease" => (
                                        causm_core::ParamMode::Lease,
                                        p_inner.next().unwrap().as_str().to_string(),
                                    ),
                                    _ => (
                                        causm_core::ParamMode::Peek,
                                        first_pair.as_str().to_string(),
                                    ),
                                };
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
