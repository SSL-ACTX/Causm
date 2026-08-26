use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::types::Type;
use causm_core::*;

pub(crate) fn infer_expression_type(
    analyzer: &EntropicAnalyzer,
    expr: &Expression,
) -> Result<Type, SemanticError> {
    match expr {
        Expression::Null => Ok(Type::Unknown),
        Expression::Boolean(_) => Ok(Type::Bool),
        Expression::Integer(_) => Ok(Type::Integer),
        Expression::ArenaIntrospect(_) => Ok(Type::Integer),
        Expression::CapabilityCheck(_) => Ok(Type::Bool),
        Expression::Float(_) => Ok(Type::Float),
        Expression::Literal(_) => Ok(Type::String),
        Expression::Identifier(name) => match analyzer.get_variable_type(name) {
            Some(typ) => {
                let resolved_t = analyzer.resolve_type(&typ);
                if let Type::Struct(s) = resolved_t {
                    if let Some(decay_ms) = s.decay_after_ms {
                        let branch = analyzer
                            .branch_contexts
                            .get(&analyzer.current_branch)
                            .unwrap();
                        let instantiated_at =
                            branch.instantiated_at.get(name).cloned().unwrap_or(0);
                        let current_cost = branch.accumulated_cost;
                        if current_cost > instantiated_at + decay_ms
                            && !analyzer.in_entropy_match
                        {
                            return Err(analyzer.annotate(
                                SemanticErrorKind::UsedDecayedValue(
                                    name.clone(),
                                    decay_ms,
                                    instantiated_at,
                                    current_cost,
                                ),
                            ));
                        }
                    }
                }
                Ok(typ)
            }
            None => Err(analyzer
                .annotate(SemanticErrorKind::UndefinedVariable(name.to_string()))),
        },
        Expression::StructLit(type_name, fields) => {
            if let Some(ref name) = *type_name.borrow() {
                return Ok(Type::Custom(name.clone()));
            }
            let mut schema = std::collections::HashMap::new();
            for (k, v) in fields {
                schema.insert(k.clone(), infer_expression_type(analyzer, v)?);
            }
            Ok(Type::Struct(causm_core::types::StructType {
                fields: schema,
                decay_after_ms: None,
                auto_drop: None,
                scoped_branch: None,
            }))
        }
        Expression::TopologyLit(fields) => {
            let mut schema = std::collections::HashMap::new();
            for (k, v) in fields {
                schema.insert(k.clone(), infer_expression_type(analyzer, v)?);
            }
            Ok(Type::Topology(schema))
        }
        Expression::ArrayLiteral(elements) => {
            let elem_types: Vec<Type> = elements
                .iter()
                .map(|e| infer_expression_type(analyzer, e))
                .collect::<Result<_, _>>()?;
            if elem_types.is_empty() {
                Ok(Type::Array(Box::new(Type::Unknown)))
            } else {
                let first = elem_types[0].clone();
                if elem_types.iter().all(|t| t == &first) {
                    Ok(Type::Array(Box::new(first)))
                } else {
                    Ok(Type::Array(Box::new(Type::Unknown)))
                }
            }
        }
        Expression::ArrayRepeat { value, .. } => {
            let elem_type = infer_expression_type(analyzer, value)?;
            Ok(Type::Array(Box::new(elem_type)))
        }
        Expression::ArraySlice { target, .. } => {
            let target_type = infer_expression_type(analyzer, target)?;
            match target_type {
                Type::String => Ok(Type::String),
                Type::Array(inner) => Ok(Type::Array(inner)),
                _ => Ok(Type::Array(Box::new(Type::Unknown))),
            }
        }
        Expression::ChannelReceive(_) => Ok(Type::Unknown),
        Expression::Deferred { .. } => Ok(Type::Promise(Box::new(Type::Unknown))),
        Expression::Call { routine, .. } => {
            if let Some(info) = analyzer.routines.get(routine) {
                Ok(info.return_type.clone())
            } else {
                Ok(Type::Unknown)
            }
        }
        Expression::MethodCall {
            target,
            method,
            args: _,
            resolved_routine,
            resolved_budget,
        } => {
            if let Expression::Identifier(ref name) = **target {
                let is_enum_type = analyzer
                    .branch_contexts
                    .get(&analyzer.current_branch)
                    .map(|st| st.custom_types.contains_key(name))
                    .unwrap_or(false);
                if is_enum_type {
                    return Ok(Type::Custom(name.clone()));
                }
            }
            if let Some(ns) = get_static_target_path(target) {
                let static_routine_name = format!("{}.{}", ns, method);
                let is_local_var = analyzer
                    .branch_contexts
                    .get(&analyzer.current_branch)
                    .map(|st| st.types.contains_key(&ns))
                    .unwrap_or(false);
                if !is_local_var {
                    if let Some(info) = analyzer.routines.get(&static_routine_name) {
                        return Ok(info.return_type.clone());
                    }
                }
            }
            let target_type = infer_expression_type(analyzer, target)?;
            let struct_name = match &target_type {
                Type::Custom(name) => name.clone(),
                _ => {
                    return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                        "method call target must be a custom type instance".into(),
                    )));
                }
            };

            if analyzer.interfaces.contains_key(&struct_name) {
                let methods = &analyzer.interfaces[&struct_name];
                let m =
                    methods.iter().find(|m| &m.name == method).ok_or_else(|| {
                        analyzer.annotate(SemanticErrorKind::TypeMismatch(format!(
                            "unknown method {} on interface {}",
                            method, struct_name
                        )))
                    })?;
                *resolved_routine.borrow_mut() = Some("<dynamic>".to_string());
                *resolved_budget.borrow_mut() = m.taking_ms;
                let ret_t = m
                    .return_type
                    .as_ref()
                    .map(Type::from_typename)
                    .unwrap_or(Type::Unknown);
                return Ok(ret_t);
            }

            if method.starts_with('_') {
                let mut is_allowed = false;
                if let Some(ref cur_routine) = analyzer.current_routine {
                    if let Some(dot_idx) = cur_routine.find('.') {
                        let struct_prefix = &cur_routine[..dot_idx];
                        if struct_prefix == struct_name {
                            is_allowed = true;
                        }
                    }
                }
                if !is_allowed {
                    return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                        format!(
                            "Method '{}' is private to type '{}'",
                            method, struct_name
                        ),
                    )));
                }
            }

            let mut current_struct = struct_name
                .split('<')
                .next()
                .unwrap_or(&struct_name)
                .split("::")
                .next()
                .unwrap_or(&struct_name)
                .trim()
                .to_string();
            let mut resolved = None;
            loop {
                let routine_name = format!("{}.{}", current_struct, method);
                if let Some(info) = analyzer.routines.get(&routine_name) {
                    resolved = Some((routine_name, info.clone()));
                    break;
                }
                if let Some(parent) = analyzer.struct_extends.get(&current_struct) {
                    current_struct = parent.clone();
                } else {
                    break;
                }
            }

            if let Some((resolved_name, info)) = resolved {
                *resolved_routine.borrow_mut() = Some(resolved_name);
                Ok(info.return_type.clone())
            } else {
                Err(
                    analyzer.annotate(SemanticErrorKind::EntropyMismatch(format!(
                        "unknown method {} on type {}",
                        method, struct_name
                    ))),
                )
            }
        }
        Expression::FieldAccess { target, field } => {
            if let Expression::Identifier(ref name) = &**target {
                if analyzer.type_decls.contains_key(name) {
                    let fields_map = &analyzer.type_decls[name];
                    if let Some(field_def) = fields_map.get(field) {
                        if field_def.is_const {
                            return Ok(Type::from_typename(&field_def.typ));
                        }
                    }
                }
            }
            let t = infer_expression_type(analyzer, target)?;
            if let Type::Custom(ref struct_name) = t {
                if field.starts_with('_') {
                    let mut is_allowed = false;
                    if let Some(ref cur_routine) = analyzer.current_routine {
                        if let Some(dot_idx) = cur_routine.find('.') {
                            let struct_prefix = &cur_routine[..dot_idx];
                            if struct_prefix == struct_name {
                                is_allowed = true;
                            }
                        }
                    }
                    if !is_allowed {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::TypeMismatch(format!(
                                "Field '{}' is private to type '{}'",
                                field, struct_name
                            )),
                        ));
                    }
                }
            }
            let mut resolved_t = analyzer.resolve_type(&t);
            if let Type::ConstantAccess { inner_type, .. } = resolved_t {
                resolved_t = analyzer.resolve_type(&inner_type);
            }

            match resolved_t {
                Type::Unknown => Ok(Type::Unknown),
                Type::Struct(s) => s.fields.get(field).cloned().ok_or_else(|| {
                    analyzer.annotate(SemanticErrorKind::TypeMismatch(format!(
                        "field '{}' not found",
                        field
                    )))
                }),
                Type::Topology(fields) => {
                    fields.get(field).cloned().ok_or_else(|| {
                        analyzer.annotate(SemanticErrorKind::TypeMismatch(format!(
                            "field '{}' not found",
                            field
                        )))
                    })
                }
                _ => Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                    "field access on non-struct/topology".into(),
                ))),
            }
        }
        Expression::IndexAccess { .. } => Ok(Type::Unknown),
        Expression::RefOp(expr) => infer_expression_type(analyzer, expr),
        Expression::Syscall { .. } => Ok(Type::Integer),
        Expression::EnumVariant { enum_name, .. } => {
            Ok(Type::Custom(enum_name.clone()))
        }
        Expression::CloneOp(name) => match analyzer.get_variable_type(name) {
            Some(typ) => Ok(typ),
            None => Err(analyzer
                .annotate(SemanticErrorKind::UndefinedVariable(name.to_string()))),
        },
        Expression::StrBytes(_) => Ok(Type::Array(Box::new(Type::Unknown))),
        Expression::ToStr(_) => Ok(Type::String),
        Expression::Len(_) => Ok(Type::I32),
        Expression::BinaryOp { left, op, right } => {
            let left_type = infer_expression_type(analyzer, left)?;
            let right_type = infer_expression_type(analyzer, right)?;
            match op {
                BinaryOperator::Eq
                | BinaryOperator::Neq
                | BinaryOperator::Lt
                | BinaryOperator::Gt
                | BinaryOperator::Le
                | BinaryOperator::Ge
                | BinaryOperator::LogicalAnd
                | BinaryOperator::LogicalOr => Ok(Type::Bool),
                BinaryOperator::Add
                | BinaryOperator::Sub
                | BinaryOperator::Mul
                | BinaryOperator::Div
                | BinaryOperator::Rem
                | BinaryOperator::Pow => {
                    if (left_type == Type::String || right_type == Type::String)
                        && matches!(op, BinaryOperator::Add)
                    {
                        Ok(Type::String)
                    } else if left_type == Type::Float || right_type == Type::Float {
                        Ok(Type::Float)
                    } else {
                        Ok(Type::Integer)
                    }
                }
            }
        }
        Expression::UnaryOp { op, expr } => match op {
            UnaryOperator::Not => Ok(Type::Bool),
            UnaryOperator::Neg => infer_expression_type(analyzer, expr),
        },
        Expression::TypeAssertion { cast_type, .. } => {
            Ok(Type::from_typename(cast_type))
        }
        Expression::TypeCast { target_type, .. } => {
            Ok(Type::from_typename(target_type))
        }
        Expression::TryUnwrap(expr) => {
            let inner_ty = infer_expression_type(analyzer, expr)?;
            match inner_ty {
                Type::Optional(opt) => Ok(*opt),
                other => Ok(other),
            }
        }
        Expression::FString(_) => Ok(Type::String),
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond_t = infer_expression_type(analyzer, condition)?;
            if cond_t != Type::Bool && cond_t != Type::Unknown {
                return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                    format!("if condition must be bool, got {:?}", cond_t),
                )));
            }
            let then_t = infer_expression_type(analyzer, then_branch)?;
            let else_t = infer_expression_type(analyzer, else_branch)?;
            if then_t == else_t || else_t == Type::Unknown {
                Ok(then_t)
            } else if then_t == Type::Unknown {
                Ok(else_t)
            } else {
                Ok(then_t)
            }
        }
        Expression::Match { arms, .. } => {
            if let Some(first_arm) = arms.first() {
                let mut local_analyzer = analyzer.clone();
                crate::statements::control_flow::bind_pattern_variables(
                    &mut local_analyzer,
                    &first_arm.pattern,
                );
                infer_expression_type(&local_analyzer, &first_arm.body)
            } else {
                Ok(Type::Unknown)
            }
        }
        Expression::Turbofish { expr, .. } => infer_expression_type(analyzer, expr),
        Expression::GenericStaticCall {
            type_name, method, ..
        } => {
            let qualified = format!("{}.{}", type_name, method);
            if let Some(info) = analyzer.routines.get(&qualified) {
                return Ok(info.return_type.clone());
            }
            Ok(Type::Unknown)
        }
    }
}

pub(crate) fn get_static_target_path(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Identifier(id) => Some(id.clone()),
        Expression::FieldAccess { target, field } => {
            let parent = get_static_target_path(target)?;
            Some(format!("{}.{}", parent, field))
        }
        _ => None,
    }
}
