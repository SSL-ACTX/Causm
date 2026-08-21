use crate::parser::Rule;
use causm_core::{FStringPart, *};
use pest::iterators::Pair;
use std::collections::HashMap;

pub(crate) fn parse_expression(pair: pest::iterators::Pair<Rule>) -> Expression {
    match pair.as_rule() {
        Rule::pipeline_expr => {
            let mut inner = pair.into_inner();
            let mut left = parse_expression(inner.next().unwrap());
            for stage in inner {
                let stage_expr = parse_expression(stage);
                left = match stage_expr {
                    Expression::Identifier(fn_name) => Expression::Call {
                        routine: fn_name,
                        args: vec![left],
                    },
                    Expression::Call { routine, mut args } => {
                        args.insert(0, left);
                        Expression::Call { routine, args }
                    }
                    Expression::MethodCall {
                        target,
                        method,
                        mut args,
                        resolved_routine,
                        resolved_budget,
                    } => {
                        args.insert(0, left);
                        Expression::MethodCall {
                            target,
                            method,
                            args,
                            resolved_routine,
                            resolved_budget,
                        }
                    }
                    _ => left,
                };
            }
            left
        }
        Rule::expression
        | Rule::relational_expr
        | Rule::additive_expr
        | Rule::multiplicative_expr
        | Rule::power_expr => {
            let mut inner = pair.into_inner();
            let first = inner.next().map(parse_expression);
            if first.is_none() {
                return Expression::Literal("void".into());
            }
            let mut left = first.unwrap();
            while let Some(op_pair) = inner.next() {
                let op = match op_pair.as_str() {
                    "+" => causm_core::BinaryOperator::Add,
                    "-" => causm_core::BinaryOperator::Sub,
                    "*" => causm_core::BinaryOperator::Mul,
                    "/" => causm_core::BinaryOperator::Div,
                    "%" => causm_core::BinaryOperator::Rem,
                    "^" => causm_core::BinaryOperator::Pow,
                    "==" => causm_core::BinaryOperator::Eq,
                    "!=" => causm_core::BinaryOperator::Neq,
                    "<" => causm_core::BinaryOperator::Lt,
                    ">" => causm_core::BinaryOperator::Gt,
                    "<=" => causm_core::BinaryOperator::Le,
                    ">=" => causm_core::BinaryOperator::Ge,
                    _ => causm_core::BinaryOperator::Eq,
                };
                if let Some(right) = inner.next() {
                    let right_expr = parse_expression(right);
                    left = Expression::BinaryOp {
                        left: Box::new(left),
                        op,
                        right: Box::new(right_expr),
                    };
                }
            }
            left
        }
        Rule::unary_expr => parse_expression(pair.into_inner().next().unwrap()),
        Rule::neg_expr => {
            let expr = parse_expression(pair.into_inner().next().unwrap());
            Expression::UnaryOp {
                op: causm_core::UnaryOperator::Neg,
                expr: Box::new(expr),
            }
        }
        Rule::not_expr => {
            let expr = parse_expression(pair.into_inner().next().unwrap());
            Expression::UnaryOp {
                op: causm_core::UnaryOperator::Not,
                expr: Box::new(expr),
            }
        }
        Rule::primary_expr => {
            let mut inner = pair.into_inner();
            let mut expr = parse_expression(inner.next().unwrap());
            for access_pair in inner {
                match access_pair.as_rule() {
                    Rule::try_tail => {
                        expr = Expression::TryUnwrap(Box::new(expr));
                    }
                    Rule::index_access => {
                        let inner_pair = access_pair.into_inner().next().unwrap();
                        if inner_pair.as_rule() == Rule::slice_range {
                            let raw_str = inner_pair.as_str();
                            let inclusive = raw_str.contains("..=");
                            let mut s_inner = inner_pair.into_inner();
                            let mut start = None;
                            let mut end = None;

                            let first = s_inner.next();
                            if let Some(p) = first {
                                if raw_str.starts_with("..") {
                                    end = Some(Box::new(parse_expression(p)));
                                } else {
                                    start = Some(Box::new(parse_expression(p)));
                                    if let Some(p2) = s_inner.next() {
                                        end = Some(Box::new(parse_expression(p2)));
                                    }
                                }
                            }
                            expr = Expression::ArraySlice {
                                target: Box::new(expr),
                                start,
                                end,
                                inclusive,
                            };
                        } else {
                            let index = parse_expression(inner_pair);
                            expr = Expression::IndexAccess {
                                target: Box::new(expr),
                                index: Box::new(index),
                            };
                        }
                    }
                    Rule::method_call_tail => {
                        let mut call_inner = access_pair.into_inner();
                        let method = call_inner
                            .next()
                            .map(|p| p.as_str().to_string())
                            .unwrap_or_default();
                        let mut args = Vec::new();
                        if let Some(arg_list_pair) = call_inner.next() {
                            for arg in arg_list_pair.into_inner() {
                                args.push(parse_expression(arg));
                            }
                        }
                        expr = Expression::MethodCall {
                            target: Box::new(expr),
                            method,
                            args,
                            resolved_routine: std::cell::RefCell::new(None),
                            resolved_budget: std::cell::RefCell::new(None),
                        };
                    }
                    Rule::type_assertion_tail => {
                        let type_name_pair =
                            access_pair.into_inner().next().unwrap();
                        let cast_type =
                            crate::parser::statements::utils::parse_type_name(
                                type_name_pair,
                            );
                        expr = Expression::TypeAssertion {
                            target: Box::new(expr),
                            cast_type,
                        };
                    }
                    Rule::type_cast_tail => {
                        let mut inner = access_pair.into_inner();
                        let type_name_pair = if inner.len() == 2 {
                            let _ = inner.next();
                            inner.next().unwrap()
                        } else {
                            inner.next().unwrap()
                        };
                        let target_type =
                            crate::parser::statements::utils::parse_type_name(
                                type_name_pair,
                            );
                        expr = Expression::TypeCast {
                            expr: Box::new(expr),
                            target_type,
                        };
                    }
                    Rule::field_access_tail => {
                        let field = access_pair
                            .into_inner()
                            .next()
                            .map(|p| p.as_str().to_string())
                            .unwrap_or_default();
                        expr = Expression::FieldAccess {
                            target: Box::new(expr),
                            field,
                        };
                    }
                    _ => {}
                }
            }
            expr
        }
        Rule::base_expr => parse_expression(pair.into_inner().next().unwrap()),
        Rule::defer_expr => {
            let mut inner = pair.into_inner();
            let capability = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut params = std::collections::HashMap::new();
            let mut deadline_ms = 0;

            let mut next = inner.next();
            if let Some(p) = &next {
                if p.as_rule() == Rule::param_list {
                    for param in p.clone().into_inner() {
                        let mut param_inner = param.into_inner();
                        if let (Some(key), Some(value_pair)) =
                            (param_inner.next(), param_inner.next())
                        {
                            let key_str = key.as_str().trim_matches('"').to_string();
                            let val_expr = parse_expression(value_pair.clone());
                            let val_str = match val_expr {
                                Expression::Literal(s) => s,
                                Expression::Identifier(id) => id,
                                Expression::Integer(i) => i.to_string(),
                                Expression::Float(bits) => {
                                    f64::from_bits(bits).to_string()
                                }
                                Expression::Boolean(b) => b.to_string(),
                                _ => {
                                    value_pair.as_str().trim_matches('"').to_string()
                                }
                            };
                            params.insert(key_str, val_str);
                        }
                    }
                    next = inner.next();
                }
            }

            if let Some(p) = next {
                deadline_ms =
                    crate::parser::statements::utils::parse_duration_limit(p);
            }

            Expression::Deferred {
                capability,
                params,
                deadline_ms,
            }
        }
        Rule::call_expr | Rule::direct_call_expr => {
            let mut inner = pair.into_inner();
            let routine = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut args = Vec::new();
            if let Some(expr_list) = inner.next() {
                for e in expr_list.into_inner() {
                    args.push(parse_expression(e));
                }
            }
            if routine == "clone" && args.len() == 1 {
                if let Expression::Identifier(ref name) = args[0] {
                    return Expression::CloneOp(name.clone());
                }
            } else if routine == "chan_recv" && args.len() == 1 {
                if let Expression::Identifier(ref name) = args[0] {
                    return Expression::ChannelReceive(name.clone());
                }
            }
            Expression::Call { routine, args }
        }
        Rule::byte_string => parse_byte_string(pair.as_str()),
        Rule::hex_byte_string => parse_hex_byte_string(pair.as_str()),
        Rule::duration_literal => {
            let str_val = pair.as_str();
            let val = if str_val.ends_with("ns") {
                str_val.trim_end_matches("ns").parse::<i64>().unwrap_or(0)
                    / 1_000_000
            } else if str_val.ends_with("us") {
                str_val.trim_end_matches("us").parse::<i64>().unwrap_or(0) / 1000
            } else if str_val.ends_with("ms") {
                str_val.trim_end_matches("ms").parse::<i64>().unwrap_or(0)
            } else if str_val.ends_with('s') {
                str_val.trim_end_matches('s').parse::<i64>().unwrap_or(0) * 1000
            } else {
                0
            };
            Expression::Integer(val)
        }
        Rule::hex_integer_literal => {
            let s = pair.as_str();
            let val = u64::from_str_radix(&s[2..], 16)
                .map(|v| v as i64)
                .unwrap_or(0);
            Expression::Integer(val)
        }
        Rule::integer_literal => {
            let s = pair.as_str();
            let val = if s.starts_with("0x") || s.starts_with("0X") {
                u64::from_str_radix(&s[2..], 16)
                    .map(|v| v as i64)
                    .unwrap_or(0)
            } else {
                s.parse::<i64>().unwrap_or(0)
            };
            Expression::Integer(val)
        }
        Rule::float_literal => {
            let val = pair.as_str().parse::<f64>().unwrap_or(0.0);
            Expression::Float(val.to_bits())
        }
        Rule::bool_literal => Expression::Boolean(pair.as_str() == "true"),
        Rule::string_literal => Expression::Literal(unescape_string(pair.as_str())),
        Rule::fstring_literal => {
            let parts: Vec<FStringPart> = pair
                .into_inner()
                .map(|part| match part.as_rule() {
                    Rule::fstring_text => {
                        FStringPart::Text(unescape_raw_text(part.as_str()))
                    }
                    Rule::fstring_interp => {
                        let inner_expr = part.into_inner().next().unwrap();
                        FStringPart::Expr(parse_expression(inner_expr))
                    }
                    Rule::fstring_part => {
                        let inner = part.into_inner().next().unwrap();
                        match inner.as_rule() {
                            Rule::fstring_text => {
                                FStringPart::Text(unescape_raw_text(inner.as_str()))
                            }
                            Rule::fstring_interp => {
                                let expr_pair = inner.into_inner().next().unwrap();
                                FStringPart::Expr(parse_expression(expr_pair))
                            }
                            _ => {
                                FStringPart::Text(unescape_raw_text(inner.as_str()))
                            }
                        }
                    }
                    _ => FStringPart::Text(unescape_raw_text(part.as_str())),
                })
                .collect();
            Expression::FString(parts)
        }
        Rule::identifier_expr | Rule::identifier => {
            Expression::Identifier(pair.as_str().to_string())
        }
        Rule::path_expr => {
            let mut inner = pair.into_inner();
            let enum_name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let variant_name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let mut args = Vec::new();
            if let Some(list_pair) = inner.next() {
                for expr_pair in list_pair.into_inner() {
                    args.push(parse_expression(expr_pair));
                }
            }
            Expression::EnumVariant {
                enum_name,
                variant_name,
                args,
            }
        }
        Rule::clone_op => Expression::CloneOp(
            pair.into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default(),
        ),
        Rule::str_bytes_expr => {
            let inner = pair.into_inner().next().unwrap();
            Expression::StrBytes(Box::new(parse_expression(inner)))
        }
        Rule::arena_expr => {
            let kind_pair = pair.into_inner().next().unwrap();
            let kind = match kind_pair.as_str() {
                "remaining" => ArenaIntrospect::Remaining,
                "used_bytes" => ArenaIntrospect::UsedBytes,
                "capacity" => ArenaIntrospect::Capacity,
                _ => ArenaIntrospect::Remaining,
            };
            Expression::ArenaIntrospect(kind)
        }
        Rule::to_str_expr => {
            let inner = pair.into_inner().next().unwrap();
            Expression::ToStr(Box::new(parse_expression(inner)))
        }
        Rule::len_expr => {
            let inner = pair.into_inner().next().unwrap();
            Expression::Len(Box::new(parse_expression(inner)))
        }
        Rule::syscall_expr => {
            let mut inner = pair.into_inner();
            let target_pair = inner.next().unwrap();
            let target = match target_pair.as_rule() {
                Rule::string_literal => {
                    SyscallTarget::Symbol(target_pair.as_str().replace('"', ""))
                }
                _ => SyscallTarget::Number(
                    target_pair.as_str().parse::<i64>().unwrap_or(0),
                ),
            };
            let mut args = Vec::new();
            let mut duration_ms = None;
            for p in inner {
                match p.as_rule() {
                    Rule::expression_list => {
                        for e in p.into_inner() {
                            args.push(parse_expression(e));
                        }
                    }
                    Rule::expression => {
                        args.push(parse_expression(p));
                    }
                    Rule::duration_limit => {
                        let str_val = p.as_str();
                        let digits: String =
                            str_val.chars().filter(|c| c.is_ascii_digit()).collect();
                        duration_ms = digits.parse::<u64>().ok();
                    }
                    _ => {}
                }
            }
            Expression::Syscall {
                target,
                args,
                duration_ms,
            }
        }
        Rule::ref_op => {
            let inner_expr = parse_expression(pair.into_inner().next().unwrap());
            Expression::RefOp(Box::new(inner_expr))
        }
        Rule::if_expr => {
            let mut inner = pair.into_inner();
            let cond = parse_expression(inner.next().unwrap());
            let then_e = parse_expression(inner.next().unwrap());
            let else_e = parse_expression(inner.next().unwrap());
            Expression::If {
                condition: Box::new(cond),
                then_branch: Box::new(then_e),
                else_branch: Box::new(else_e),
            }
        }
        Rule::match_entropy_expr => {
            let stmt =
                crate::parser::statements::entropic::parse_entropic_stmt(pair);
            if let Statement::MatchEntropy { target, .. } = stmt {
                target
            } else {
                Expression::Null
            }
        }
        Rule::match_expr => {
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
                let body = parse_expression(body_pair);
                arms.push(MatchExprArm {
                    pattern,
                    guard,
                    body,
                });
            }
            Expression::Match {
                target: Box::new(target),
                arms,
            }
        }
        Rule::await_expr => {
            let inner_expr = parse_expression(pair.into_inner().next().unwrap());
            if let Expression::Identifier(ref id) = inner_expr {
                Expression::Identifier(id.clone())
            } else {
                inner_expr
            }
        }
        Rule::chan_recv_expr => Expression::ChannelReceive(
            pair.into_inner()
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default(),
        ),
        Rule::struct_lit | Rule::topology_lit => {
            let rule = pair.as_rule();
            let mut inner = pair.into_inner();

            let (type_name, params_pair) = (None, inner.next());

            let mut fields = HashMap::new();
            if let Some(params) = params_pair {
                for p in params.into_inner() {
                    let mut p_inner = p.into_inner();
                    if let (Some(k), Some(v)) = (p_inner.next(), p_inner.next()) {
                        fields.insert(
                            k.as_str().replace("\"", ""),
                            parse_expression(v),
                        );
                    }
                }
            }
            if rule == Rule::struct_lit {
                Expression::StructLit(std::cell::RefCell::new(type_name), fields)
            } else {
                Expression::TopologyLit(fields)
            }
        }
        Rule::array_lit => {
            let inner = pair.into_inner().next().unwrap();
            match inner.as_rule() {
                Rule::array_repeat => {
                    let mut r_inner = inner.into_inner();
                    let val = parse_expression(r_inner.next().unwrap());
                    let count = parse_expression(r_inner.next().unwrap());
                    Expression::ArrayRepeat {
                        value: Box::new(val),
                        count: Box::new(count),
                    }
                }
                Rule::array_elements => {
                    let mut elements = Vec::new();
                    for expr_pair in inner.into_inner() {
                        elements.push(parse_expression(expr_pair));
                    }
                    Expression::ArrayLiteral(elements)
                }
                _ => Expression::ArrayLiteral(Vec::new()),
            }
        }
        Rule::array_repeat => {
            let mut r_inner = pair.into_inner();
            let val = parse_expression(r_inner.next().unwrap());
            let count = parse_expression(r_inner.next().unwrap());
            Expression::ArrayRepeat {
                value: Box::new(val),
                count: Box::new(count),
            }
        }
        Rule::array_elements => {
            let mut elements = Vec::new();
            for expr_pair in pair.into_inner() {
                elements.push(parse_expression(expr_pair));
            }
            Expression::ArrayLiteral(elements)
        }
        Rule::field_access => {
            let mut inner = pair.into_inner();
            let parent = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let field = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            Expression::FieldAccess {
                target: Box::new(Expression::Identifier(parent)),
                field,
            }
        }
        Rule::null => Expression::Null,
        _ => Expression::Literal(pair.as_str().to_string()),
    }
}

pub(crate) fn unescape_string(raw: &str) -> String {
    let inner = if (raw.starts_with('"') && raw.ends_with('"'))
        || (raw.starts_with('\'') && raw.ends_with('\''))
    {
        if raw.len() >= 2 {
            &raw[1..raw.len() - 1]
        } else {
            raw
        }
    } else {
        raw
    };

    unescape_raw_text(inner)
}

pub(crate) fn unescape_raw_text(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('\'') => out.push('\''),
                Some('0') => out.push('\0'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

pub(crate) fn parse_byte_string(raw: &str) -> Expression {
    let inner = if raw.starts_with("b\"") && raw.ends_with('"') && raw.len() >= 3 {
        &raw[2..raw.len() - 1]
    } else {
        raw
    };
    let unescaped = unescape_raw_text(inner);
    let elements = unescaped
        .bytes()
        .map(|b| Expression::Integer(b as i64))
        .collect();
    Expression::ArrayLiteral(elements)
}

pub(crate) fn parse_hex_byte_string(raw: &str) -> Expression {
    let inner = if raw.starts_with("hex\"") && raw.ends_with('"') && raw.len() >= 5 {
        &raw[4..raw.len() - 1]
    } else {
        raw
    };
    let clean: String = inner.chars().filter(|c| !c.is_whitespace()).collect();
    let mut elements = Vec::new();
    let chars: Vec<char> = clean.chars().collect();
    let mut i = 0;
    while i + 1 < chars.len() {
        let hex_chunk: String = chars[i..i + 2].iter().collect();
        if let Ok(byte_val) = u8::from_str_radix(&hex_chunk, 16) {
            elements.push(Expression::Integer(byte_val as i64));
        }
        i += 2;
    }
    Expression::ArrayLiteral(elements)
}

pub fn parse_pattern(pair: Pair<Rule>) -> Pattern {
    match pair.as_rule() {
        Rule::match_pattern => parse_pattern(pair.into_inner().next().unwrap()),
        Rule::wildcard_pattern => Pattern::Wildcard,
        Rule::ident_pattern => Pattern::Identifier(pair.as_str().trim().to_string()),
        Rule::literal_pattern => {
            let inner = pair.into_inner().next().unwrap();
            let expr = parse_expression(inner);
            Pattern::Literal(expr)
        }
        Rule::enum_pattern => {
            let mut inner: Vec<_> = pair.into_inner().collect();
            if inner.is_empty() {
                return Pattern::Wildcard;
            }
            if inner.len() == 1 {
                let first = inner.remove(0).as_str().to_string();
                Pattern::EnumVariant {
                    enum_name: None,
                    variant_name: first,
                    args: Vec::new(),
                }
            } else if inner.len() == 2
                && inner[1].as_rule() == Rule::pattern_arg_list
            {
                let variant_name = inner.remove(0).as_str().to_string();
                let args_pair = inner.remove(0);
                let args = args_pair.into_inner().map(parse_pattern).collect();
                Pattern::EnumVariant {
                    enum_name: None,
                    variant_name,
                    args,
                }
            } else if inner.len() == 2 && inner[1].as_rule() == Rule::identifier {
                let enum_name = inner.remove(0).as_str().to_string();
                let variant_name = inner.remove(0).as_str().to_string();
                Pattern::EnumVariant {
                    enum_name: Some(enum_name),
                    variant_name,
                    args: Vec::new(),
                }
            } else if inner.len() == 3 {
                let enum_name = inner.remove(0).as_str().to_string();
                let variant_name = inner.remove(0).as_str().to_string();
                let args_pair = inner.remove(0);
                let args = args_pair.into_inner().map(parse_pattern).collect();
                Pattern::EnumVariant {
                    enum_name: Some(enum_name),
                    variant_name,
                    args,
                }
            } else {
                let first = inner.remove(0).as_str().to_string();
                Pattern::EnumVariant {
                    enum_name: None,
                    variant_name: first,
                    args: Vec::new(),
                }
            }
        }
        _ => Pattern::Wildcard,
    }
}
