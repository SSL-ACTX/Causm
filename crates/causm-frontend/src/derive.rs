//! Automatic `@derive` code generation pass.
//!
//! Generates synthetic helper methods for types and enums tagged with `@derive(...)`:
//! - `@derive(Clone)`: Generates `TypeName.clone(peek self: TypeName) -> TypeName`
//! - `@derive(Debug)` / `@derive(ToString)`: Generates `TypeName.to_string(peek self: TypeName) -> string`
//! - `@derive(PartialEq)`: Generates `TypeName.equals(peek self: TypeName, peek other: TypeName) -> bool`

use causm_core::*;

pub fn expand_derives(program: &mut Program) {
    for timeline in &mut program.timelines {
        let mut generated_stmts = Vec::new();
        for spanned in &timeline.statements {
            for attr in &spanned.attributes {
                if let AttributeKind::Derive(traits) = &attr.kind {
                    match &spanned.stmt {
                        Statement::TypeDecl { name, fields, .. } => {
                            for t in traits {
                                match t.as_str() {
                                    "Clone" => {
                                        generated_stmts.push(generate_struct_clone(
                                            name,
                                            fields,
                                            spanned.span.clone(),
                                        ));
                                    }
                                    "Debug" | "ToString" => {
                                        generated_stmts.push(generate_struct_debug(
                                            name,
                                            fields,
                                            spanned.span.clone(),
                                        ));
                                    }
                                    "PartialEq" => {
                                        generated_stmts.push(
                                            generate_struct_equals(
                                                name,
                                                fields,
                                                spanned.span.clone(),
                                            ),
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Statement::EnumDecl { name, variants } => {
                            for t in traits {
                                match t.as_str() {
                                    "Clone" => {
                                        generated_stmts.push(generate_enum_clone(
                                            name,
                                            variants,
                                            spanned.span.clone(),
                                        ));
                                    }
                                    "Debug" | "ToString" => {
                                        generated_stmts.push(generate_enum_debug(
                                            name,
                                            variants,
                                            spanned.span.clone(),
                                        ));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        timeline.statements.extend(generated_stmts);
    }
}

fn generate_struct_clone(
    type_name: &str,
    _fields: &std::collections::HashMap<String, TypeFieldDef>,
    span: Span,
) -> SpannedStatement {
    let method_name = format!("{}.clone", type_name);
    let routine = Statement::RoutineDef {
        name: method_name,
        params: vec![ParamDecl {
            mode: ParamMode::Peek,
            name: "self".to_string(),
            typ: Some(TypeName::Custom(type_name.to_string())),
        }],
        return_type: Some(TypeName::Custom(type_name.to_string())),
        taking_ms: None,
        state_constraint: None,
        required_capabilities: Vec::new(),
        body: vec![SpannedStatement::new(
            Statement::Expression(Expression::CloneOp("self".to_string())),
            span.clone(),
        )],
    };
    SpannedStatement::new(routine, span)
}

fn generate_struct_debug(
    type_name: &str,
    fields: &std::collections::HashMap<String, TypeFieldDef>,
    span: Span,
) -> SpannedStatement {
    let method_name = format!("{}.to_string", type_name);
    let mut parts = Vec::new();
    parts.push(FStringPart::Text(format!("{} {{ ", type_name)));

    let mut sorted_keys: Vec<_> = fields.keys().collect();
    sorted_keys.sort();
    for (i, k) in sorted_keys.iter().enumerate() {
        if i > 0 {
            parts.push(FStringPart::Text(", ".to_string()));
        }
        parts.push(FStringPart::Text(format!("{}: ", k)));
        parts.push(FStringPart::Expr(Expression::FieldAccess {
            target: Box::new(Expression::Identifier("self".to_string())),
            field: (*k).clone(),
        }));
    }
    parts.push(FStringPart::Text(" }".to_string()));

    let routine = Statement::RoutineDef {
        name: method_name,
        params: vec![ParamDecl {
            mode: ParamMode::Peek,
            name: "self".to_string(),
            typ: Some(TypeName::Custom(type_name.to_string())),
        }],
        return_type: Some(TypeName::Builtin(BuiltinType::String)),
        taking_ms: None,
        state_constraint: None,
        required_capabilities: Vec::new(),
        body: vec![SpannedStatement::new(
            Statement::Expression(Expression::FString(parts)),
            span.clone(),
        )],
    };
    SpannedStatement::new(routine, span)
}

fn generate_struct_equals(
    type_name: &str,
    fields: &std::collections::HashMap<String, TypeFieldDef>,
    span: Span,
) -> SpannedStatement {
    let method_name = format!("{}.equals", type_name);
    let mut cond: Option<Expression> = None;

    let mut sorted_keys: Vec<_> = fields.keys().collect();
    sorted_keys.sort();
    for k in sorted_keys {
        let field_eq = Expression::BinaryOp {
            op: BinaryOperator::Eq,
            left: Box::new(Expression::FieldAccess {
                target: Box::new(Expression::Identifier("self".to_string())),
                field: k.clone(),
            }),
            right: Box::new(Expression::FieldAccess {
                target: Box::new(Expression::Identifier("other".to_string())),
                field: k.clone(),
            }),
        };
        cond = Some(match cond {
            None => field_eq,
            Some(prev) => Expression::BinaryOp {
                op: BinaryOperator::LogicalAnd,
                left: Box::new(prev),
                right: Box::new(field_eq),
            },
        });
    }

    let routine = Statement::RoutineDef {
        name: method_name,
        params: vec![
            ParamDecl {
                mode: ParamMode::Peek,
                name: "self".to_string(),
                typ: Some(TypeName::Custom(type_name.to_string())),
            },
            ParamDecl {
                mode: ParamMode::Peek,
                name: "other".to_string(),
                typ: Some(TypeName::Custom(type_name.to_string())),
            },
        ],
        return_type: Some(TypeName::Builtin(BuiltinType::Bool)),
        taking_ms: None,
        state_constraint: None,
        required_capabilities: Vec::new(),
        body: vec![SpannedStatement::new(
            Statement::Expression(cond.unwrap_or(Expression::Boolean(true))),
            span.clone(),
        )],
    };
    SpannedStatement::new(routine, span)
}

fn generate_enum_clone(
    type_name: &str,
    _variants: &[EnumVariantDef],
    span: Span,
) -> SpannedStatement {
    let method_name = format!("{}.clone", type_name);
    let routine = Statement::RoutineDef {
        name: method_name,
        params: vec![ParamDecl {
            mode: ParamMode::Peek,
            name: "self".to_string(),
            typ: Some(TypeName::Custom(type_name.to_string())),
        }],
        return_type: Some(TypeName::Custom(type_name.to_string())),
        taking_ms: None,
        state_constraint: None,
        required_capabilities: Vec::new(),
        body: vec![SpannedStatement::new(
            Statement::Expression(Expression::CloneOp("self".to_string())),
            span.clone(),
        )],
    };
    SpannedStatement::new(routine, span)
}

fn generate_enum_debug(
    type_name: &str,
    variants: &[EnumVariantDef],
    span: Span,
) -> SpannedStatement {
    let method_name = format!("{}.to_string", type_name);
    let mut arms = Vec::new();

    for vdef in variants {
        let pattern_args: Vec<_> = (0..vdef.payload_types.len())
            .map(|i| Pattern::Identifier(format!("arg_{}", i)))
            .collect();
        let pattern = Pattern::EnumVariant {
            enum_name: Some(type_name.to_string()),
            variant_name: vdef.name.clone(),
            args: pattern_args,
        };
        let mut parts = Vec::new();
        parts.push(FStringPart::Text(format!("{}::{}", type_name, vdef.name)));
        if !vdef.payload_types.is_empty() {
            parts.push(FStringPart::Text("(".to_string()));
            for i in 0..vdef.payload_types.len() {
                if i > 0 {
                    parts.push(FStringPart::Text(", ".to_string()));
                }
                parts.push(FStringPart::Expr(Expression::Identifier(format!(
                    "arg_{}",
                    i
                ))));
            }
            parts.push(FStringPart::Text(")".to_string()));
        }
        arms.push(MatchArm {
            pattern,
            guard: None,
            body: vec![SpannedStatement::new(
                Statement::Expression(Expression::FString(parts)),
                span.clone(),
            )],
        });
    }

    let routine = Statement::RoutineDef {
        name: method_name,
        params: vec![ParamDecl {
            mode: ParamMode::Peek,
            name: "self".to_string(),
            typ: Some(TypeName::Custom(type_name.to_string())),
        }],
        return_type: Some(TypeName::Builtin(BuiltinType::String)),
        taking_ms: None,
        state_constraint: None,
        required_capabilities: Vec::new(),
        body: vec![SpannedStatement::new(
            Statement::Match {
                target: Expression::Identifier("self".to_string()),
                arms,
            },
            span.clone(),
        )],
    };
    SpannedStatement::new(routine, span)
}
