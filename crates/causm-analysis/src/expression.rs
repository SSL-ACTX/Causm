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
                        if current_cost > instantiated_at + decay_ms {
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
        Expression::StructLit(_, fields) => {
            println!(
                "DEBUG: Inferring StructLit with fields: {:?}",
                fields.keys().collect::<Vec<_>>()
            );
            let mut schema = std::collections::HashMap::new();
            for (k, v) in fields {
                schema.insert(k.clone(), infer_expression_type(analyzer, v)?);
            }
            Ok(Type::Struct(causm_core::types::StructType {
                fields: schema,
                decay_after_ms: None,
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
        Expression::ChannelReceive(_) => Ok(Type::Unknown),
        Expression::Deferred { .. } => Ok(Type::Unknown),
        Expression::Call { routine, .. } => {
            if let Some(info) = analyzer.routines.get(routine) {
                Ok(info.return_type.clone())
            } else {
                Ok(Type::Unknown)
            }
        }
        Expression::FieldAccess { target, field } => {
            let t = infer_expression_type(analyzer, target)?;
            let mut resolved_t = analyzer.resolve_type(&t);
            if let Type::ConstantAccess { inner_type, .. } = resolved_t {
                resolved_t = analyzer.resolve_type(&inner_type);
            }

            match resolved_t {
                Type::Unknown => Ok(Type::Unknown),
                Type::Struct(s) => {
                    if !s.fields.contains_key(field) {
                        println!(
                            "DEBUG: Struct fields are {:?}",
                            s.fields.keys().collect::<Vec<_>>()
                        );
                        println!("DEBUG: Target was {:?}", target);
                    }
                    s.fields.get(field).cloned().ok_or_else(|| {
                        analyzer.annotate(SemanticErrorKind::TypeMismatch(format!(
                            "field '{}' not found",
                            field
                        )))
                    })
                }
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
        Expression::CloneOp(name) => match analyzer.get_variable_type(name) {
            Some(typ) => Ok(typ),
            None => Err(analyzer
                .annotate(SemanticErrorKind::UndefinedVariable(name.to_string()))),
        },
        Expression::BinaryOp { left, op, right } => {
            let left_type = infer_expression_type(analyzer, left)?;
            let right_type = infer_expression_type(analyzer, right)?;
            match op {
                causm_core::BinaryOperator::Add => {
                    if left_type == Type::String || right_type == Type::String {
                        Ok(Type::String)
                    } else if left_type == Type::Integer
                        && right_type == Type::Integer
                    {
                        Ok(Type::Integer)
                    } else if left_type.is_numeric() && right_type.is_numeric() {
                        Ok(Type::Float)
                    } else if left_type == Type::Unknown
                        || right_type == Type::Unknown
                    {
                        Ok(Type::Unknown)
                    } else {
                        Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!(
                                "cannot apply '{:?}' to {:?} and {:?}",
                                op, left_type, right_type
                            ),
                        )))
                    }
                }
                causm_core::BinaryOperator::Sub
                | causm_core::BinaryOperator::Mul
                | causm_core::BinaryOperator::Div
                | causm_core::BinaryOperator::Rem
                | causm_core::BinaryOperator::Pow => {
                    if left_type == Type::Integer && right_type == Type::Integer {
                        Ok(Type::Integer)
                    } else if left_type.is_numeric() && right_type.is_numeric() {
                        Ok(Type::Float)
                    } else if left_type == Type::Unknown
                        || right_type == Type::Unknown
                    {
                        Ok(Type::Unknown)
                    } else {
                        Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!(
                                "cannot apply '{:?}' to {:?} and {:?}",
                                op, left_type, right_type
                            ),
                        )))
                    }
                }
                causm_core::BinaryOperator::Eq | causm_core::BinaryOperator::Neq => {
                    if left_type == Type::Unknown
                        || right_type == Type::Unknown
                        || left_type == right_type
                    {
                        Ok(Type::Bool)
                    } else {
                        Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!(
                                "cannot compare {:?} with {:?}",
                                left_type, right_type
                            ),
                        )))
                    }
                }
                causm_core::BinaryOperator::Lt
                | causm_core::BinaryOperator::Gt
                | causm_core::BinaryOperator::Le
                | causm_core::BinaryOperator::Ge => {
                    if left_type == Type::Unknown
                        || right_type == Type::Unknown
                        || (left_type.is_numeric() && right_type.is_numeric())
                    {
                        Ok(Type::Bool)
                    } else {
                        Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!(
                                "cannot order compare {:?} and {:?}",
                                left_type, right_type
                            ),
                        )))
                    }
                }
            }
        }
        Expression::UnaryOp { op, expr } => {
            let t = infer_expression_type(analyzer, expr)?;
            match op {
                causm_core::UnaryOperator::Neg => {
                    if t.is_numeric() {
                        Ok(t)
                    } else {
                        Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!("cannot negate {:?}", t),
                        )))
                    }
                }
                causm_core::UnaryOperator::Not => {
                    if t == Type::Bool {
                        Ok(Type::Bool)
                    } else {
                        Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!("cannot apply NOT to {:?}", t),
                        )))
                    }
                }
            }
        }
    }
}

pub(crate) fn analyze_expression(
    analyzer: &mut EntropicAnalyzer,
    expr: &Expression,
) -> Result<(), SemanticError> {
    infer_expression_type(analyzer, expr)?;
    match expr {
        Expression::Null => Ok(()),
        Expression::Call { routine, args } => {
            let info = analyzer.routines.get(routine).cloned().ok_or_else(|| {
                analyzer.annotate(SemanticErrorKind::EntropyMismatch(format!(
                    "unknown routine {}",
                    routine
                )))
            })?;

            if args.len() != info.params.len() {
                return Err(analyzer.annotate(SemanticErrorKind::EntropyMismatch(
                    format!(
                        "routine {} expects {} args, got {}",
                        routine,
                        info.params.len(),
                        args.len()
                    ),
                )));
            }

            for (arg_expr, (mode, _param_name, expected_type)) in
                args.iter().zip(info.params.iter())
            {
                let arg_type = infer_expression_type(analyzer, arg_expr)?;

                if !analyzer.types_compatible(expected_type, &arg_type) {
                    return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                        format!(
                            "routine {} arg type mismatch: expected {:?}, got {:?}",
                            routine, expected_type, arg_type
                        ),
                    )));
                }

                analyze_expression_nonconsuming(analyzer, arg_expr)?;

                match mode {
                    ParamMode::Consume => {
                        if let Expression::Identifier(name) = arg_expr {
                            analyzer.mark_consumed(name)?;
                        }
                        // non-identifiers are treated as value literals and do not consume existing variables
                    }
                    ParamMode::Clone => {
                        if let Expression::Identifier(name) = arg_expr {
                            let state = analyzer
                                .branch_contexts
                                .get(&analyzer.current_branch)
                                .unwrap();
                            if state.consumed.contains(name) {
                                return Err(analyzer.annotate(
                                    SemanticErrorKind::UseAfterConsume(name.clone()),
                                ));
                            }
                        }
                    }
                    ParamMode::Decay => {
                        if let Expression::Identifier(name) = arg_expr {
                            analyzer.mark_consumed(name)?;
                        }
                    }
                    ParamMode::Peek => {}
                }
            }
            Ok(())
        }
        Expression::Identifier(name) => analyzer.mark_consumed(name),
        Expression::FieldAccess { target, .. } => {
            if let Expression::Identifier(name) = &**target {
                if analyzer.inspection_depth == 0 {
                    analyzer.mark_decayed(name)?;
                }
                Ok(())
            } else {
                analyze_expression(analyzer, target)
            }
        }
        Expression::CloneOp(name) => {
            let state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .unwrap();
            if state.consumed.contains(name) {
                return Err(analyzer
                    .annotate(SemanticErrorKind::UseAfterConsume(name.clone())));
            }
            Ok(())
        }
        Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
            for inner_expr in fields.values() {
                analyze_expression(analyzer, inner_expr)?;
            }
            Ok(())
        }
        Expression::IndexAccess { target, index } => {
            if let Expression::Identifier(name) = &**target {
                analyzer.mark_decayed(name)?;
            } else {
                analyze_expression(analyzer, target)?;
            }
            analyze_expression_nonconsuming(analyzer, index)?;
            Ok(())
        }
        Expression::Deferred { capability, .. } => {
            if !analyzer.capability_stack.is_empty()
                && !analyzer.is_capability_allowed(capability)
            {
                return Err(analyzer.annotate(
                    SemanticErrorKind::MissingCapability(capability.clone()),
                ));
            }
            Ok(())
        }
        Expression::ChannelReceive(id) => {
            if !analyzer.capability_stack.is_empty() {
                let key = format!("Chan.Inbound[id={}]", id);
                if !analyzer.is_capability_allowed("Chan.Inbound")
                    && !analyzer.is_capability_allowed(&key)
                {
                    return Err(analyzer.annotate(
                        SemanticErrorKind::MissingCapability(format!(
                            "Chan.Inbound(id={})",
                            id
                        )),
                    ));
                }
            }
            Ok(())
        }
        Expression::Literal(_)
        | Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Boolean(_)
        | Expression::ArrayLiteral(_) => Ok(()),
        Expression::BinaryOp { left, right, .. } => {
            analyze_expression(analyzer, left)?;
            analyze_expression(analyzer, right)?;
            Ok(())
        }
        Expression::UnaryOp { expr, .. } => analyze_expression(analyzer, expr),
    }
}

pub(crate) fn analyze_expression_nonconsuming(
    analyzer: &mut EntropicAnalyzer,
    expr: &Expression,
) -> Result<(), SemanticError> {
    infer_expression_type(analyzer, expr)?;
    match expr {
        Expression::Call { .. } => analyze_expression(analyzer, expr),
        Expression::Identifier(name) => {
            let state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .unwrap();
            if !analyzer.in_entropy_match && state.consumed.contains(name) {
                return Err(analyzer
                    .annotate(SemanticErrorKind::UseAfterConsume(name.clone())));
            }
            Ok(())
        }
        Expression::FieldAccess { target, .. } => {
            analyze_expression_nonconsuming(analyzer, target)
        }
        Expression::CloneOp(name) => {
            let state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .unwrap();
            if state.consumed.contains(name) {
                return Err(analyzer
                    .annotate(SemanticErrorKind::UseAfterConsume(name.clone())));
            }
            Ok(())
        }
        Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
            for inner_expr in fields.values() {
                analyze_expression_nonconsuming(analyzer, inner_expr)?;
            }
            Ok(())
        }
        Expression::IndexAccess { target, index } => {
            analyze_expression_nonconsuming(analyzer, target)?;
            analyze_expression_nonconsuming(analyzer, index)?;
            Ok(())
        }

        Expression::ArrayLiteral(elements) => {
            for inner_expr in elements {
                analyze_expression_nonconsuming(analyzer, inner_expr)?;
            }
            Ok(())
        }
        Expression::Deferred { capability, .. } => {
            if !analyzer.capability_stack.is_empty()
                && !analyzer.is_capability_allowed(capability)
            {
                return Err(analyzer.annotate(
                    SemanticErrorKind::MissingCapability(capability.clone()),
                ));
            }
            Ok(())
        }
        Expression::ChannelReceive(id) => {
            if !analyzer.capability_stack.is_empty() {
                let key = format!("Chan.Inbound[id={}]", id);
                if !analyzer.is_capability_allowed("Chan.Inbound")
                    && !analyzer.is_capability_allowed(&key)
                {
                    return Err(analyzer.annotate(
                        SemanticErrorKind::MissingCapability(format!(
                            "Chan.Inbound(id={})",
                            id
                        )),
                    ));
                }
            }
            Ok(())
        }
        Expression::Literal(_)
        | Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Boolean(_)
        | Expression::Null => Ok(()),
        Expression::BinaryOp { left, right, .. } => {
            analyze_expression_nonconsuming(analyzer, left)?;
            analyze_expression_nonconsuming(analyzer, right)?;
            Ok(())
        }
        Expression::UnaryOp { expr, .. } => {
            analyze_expression_nonconsuming(analyzer, expr)
        }
    }
}

pub fn estimate_expression_cost(
    analyzer: &EntropicAnalyzer,
    expr: &Expression,
) -> u64 {
    match expr {
        Expression::Call { routine, args } => {
            let arg_cost: u64 = args
                .iter()
                .map(|a| estimate_expression_cost(analyzer, a))
                .sum();
            let taking_ms = analyzer
                .routines
                .get(routine)
                .map(|info| info.taking_ms)
                .unwrap_or(0);
            arg_cost + taking_ms
        }
        Expression::BinaryOp { left, right, .. } => {
            1 + estimate_expression_cost(analyzer, left)
                + estimate_expression_cost(analyzer, right)
        }
        Expression::UnaryOp { expr, .. } => {
            1 + estimate_expression_cost(analyzer, expr)
        }
        Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
            1 + fields
                .values()
                .map(|v| estimate_expression_cost(analyzer, v))
                .sum::<u64>()
        }
        Expression::IndexAccess { target, index } => {
            1 + estimate_expression_cost(analyzer, target)
                + estimate_expression_cost(analyzer, index)
        }
        Expression::ArrayLiteral(elements) => {
            1 + elements
                .iter()
                .map(|e| estimate_expression_cost(analyzer, e))
                .sum::<u64>()
        }
        Expression::FieldAccess { target, .. } => {
            let target_type =
                infer_expression_type(analyzer, target).unwrap_or(Type::Unknown);
            let base_cost = match target_type {
                Type::ConstantAccess { access_time_ms, .. } => access_time_ms,
                _ => 1,
            };
            base_cost + estimate_expression_cost(analyzer, target)
        }
        Expression::CloneOp(_)
        | Expression::ChannelReceive(_)
        | Expression::Identifier(_)
        | Expression::Literal(_)
        | Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Boolean(_)
        | Expression::Null
        | Expression::Deferred { .. } => 1,
    }
}
