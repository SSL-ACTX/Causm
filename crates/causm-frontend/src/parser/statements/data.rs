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
            let mut inner = pair.into_inner();
            let name = inner
                .next()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();

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
        _ => unreachable!(),
    }
}
