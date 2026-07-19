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

            let mut current_struct = struct_name.clone();
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
        Expression::TypeAssertion { target, cast_type } => {
            let target_type = infer_expression_type(analyzer, target)?;
            match target_type {
                Type::Custom(_) | Type::Unknown => {}
                _ => {
                    return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                        "type assertion target must be a custom struct or interface"
                            .into(),
                    )));
                }
            }
            Ok(Type::from_typename(cast_type))
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
        Expression::MethodCall {
            target,
            method,
            args,
            resolved_routine,
            resolved_budget,
        } => {
            analyze_expression_nonconsuming(analyzer, target)?;
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
                let interface_method = methods
                    .iter()
                    .find(|m| &m.name == method)
                    .ok_or_else(|| {
                        analyzer.annotate(SemanticErrorKind::TypeMismatch(format!(
                            "unknown method {} on interface {}",
                            method, struct_name
                        )))
                    })?
                    .clone();

                if args.len() + 1 != interface_method.params.len() {
                    return Err(analyzer.annotate(
                        SemanticErrorKind::ArgumentCountMismatch(format!(
                            "method {} expects {} arguments (excluding self), got {}",
                            method,
                            interface_method.params.len() - 1,
                            args.len()
                        )),
                    ));
                }

                let first_param = &interface_method.params[0];
                let self_mode = first_param.mode.clone();
                *resolved_routine.borrow_mut() = Some("<dynamic>".to_string());
                *resolved_budget.borrow_mut() = interface_method.taking_ms;

                if let Some((ref param_name, ref expected_state)) =
                    interface_method.state_constraint
                {
                    if param_name == "self" {
                        if let Expression::Identifier(ref name) = &**target {
                            let state = analyzer
                                .branch_contexts
                                .get(&analyzer.current_branch)
                                .unwrap();
                            let actual_state = if state.consumed.contains(name) {
                                "Consumed"
                            } else if state.decayed.contains(name) {
                                "Decayed"
                            } else {
                                "Valid"
                            };
                            if actual_state != expected_state {
                                return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                                    format!(
                                        "State constraint violated: receiver '{}' is in state '{}', but interface method '{}' expects state '{}'",
                                        name, actual_state, method, expected_state
                                    )
                                )));
                            }
                        }
                    }
                }

                for (i, arg) in args.iter().enumerate() {
                    let param_decl = &interface_method.params[i + 1];
                    let param_type = param_decl
                        .typ
                        .as_ref()
                        .map(Type::from_typename)
                        .unwrap_or(Type::Unknown);
                    let arg_type = infer_expression_type(analyzer, arg)?;
                    if !analyzer.types_compatible(&param_type, &arg_type) {
                        return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                            format!(
                                "interface method {} param {} type mismatch: expected {:?}, got {:?}",
                                method, param_decl.name, param_type, arg_type
                            ),
                        )));
                    }
                    if let Expression::StructLit(ref type_name, _) = arg {
                        if type_name.borrow().is_none() {
                            if let Type::Custom(ref name) = param_type {
                                *type_name.borrow_mut() = Some(name.clone());
                            }
                        }
                    }
                    analyze_expression_nonconsuming(analyzer, arg)?;
                    if let ParamMode::Consume = param_decl.mode {
                        if let Expression::Identifier(ref name) = arg {
                            analyzer.mark_consumed(name)?;
                        }
                    } else if let ParamMode::Clone = param_decl.mode {
                        if let Expression::Identifier(name) = arg {
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
                }

                if let ParamMode::Consume = self_mode {
                    if let Expression::Identifier(name) = &**target {
                        analyzer.mark_consumed(name)?;
                    }
                } else if let ParamMode::Clone = self_mode {
                    if let Expression::Identifier(name) = &**target {
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

                let cost = interface_method.taking_ms.unwrap_or(0);
                let branch = analyzer
                    .branch_contexts
                    .get_mut(&analyzer.current_branch)
                    .unwrap();
                branch.accumulated_cost += cost;

                return Ok(());
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

            let mut current_struct = struct_name.clone();
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

            let (resolved_name, info) = resolved.ok_or_else(|| {
                analyzer.annotate(SemanticErrorKind::EntropyMismatch(format!(
                    "unknown method {} on type {}",
                    method, struct_name
                )))
            })?;
            *resolved_routine.borrow_mut() = Some(resolved_name);

            if let Some((ref param_name, ref expected_state)) = info.state_constraint
            {
                if param_name == "self" {
                    if let Expression::Identifier(ref name) = &**target {
                        let state = analyzer
                            .branch_contexts
                            .get(&analyzer.current_branch)
                            .unwrap();
                        let actual_state = if state.consumed.contains(name) {
                            "Consumed"
                        } else if state.decayed.contains(name) {
                            "Decayed"
                        } else {
                            "Valid"
                        };
                        if actual_state != expected_state {
                            return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                                format!(
                                    "State constraint violated: receiver '{}' is in state '{}', but method '{}' expects state '{}'",
                                    name, actual_state, method, expected_state
                                )
                            )));
                        }
                    }
                }
            }

            if args.len() + 1 != info.params.len() {
                return Err(analyzer.annotate(
                    SemanticErrorKind::ArgumentCountMismatch(format!(
                        "method {} expects {} arguments (excluding self), got {}",
                        method,
                        info.params.len() - 1,
                        args.len()
                    )),
                ));
            }

            let (self_mode, _self_name, self_type) = &info.params[0];
            if !analyzer.types_compatible(self_type, &target_type) {
                return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                    format!(
                        "method {} self type mismatch: expected {:?}, got {:?}",
                        method, self_type, target_type
                    ),
                )));
            }

            match self_mode {
                ParamMode::Consume => {
                    if let Expression::Identifier(name) = &**target {
                        analyzer.mark_consumed(name)?;
                    }
                }
                ParamMode::Clone => {
                    if let Expression::Identifier(name) = &**target {
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
                _ => {}
            }

            for (arg_expr, (mode, _param_name, expected_type)) in
                args.iter().zip(info.params.iter().skip(1))
            {
                let arg_type = infer_expression_type(analyzer, arg_expr)?;

                if !analyzer.types_compatible(expected_type, &arg_type) {
                    return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                        format!(
                            "method {} arg type mismatch: expected {:?}, got {:?}",
                            method, expected_type, arg_type
                        ),
                    )));
                }

                if let Expression::StructLit(ref type_name, _) = arg_expr {
                    if type_name.borrow().is_none() {
                        if let Type::Custom(ref name) = expected_type {
                            *type_name.borrow_mut() = Some(name.clone());
                        }
                    }
                }

                analyze_expression_nonconsuming(analyzer, arg_expr)?;

                match mode {
                    ParamMode::Consume => {
                        if let Expression::Identifier(name) = arg_expr {
                            analyzer.mark_consumed(name)?;
                        }
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
                    _ => {}
                }
            }

            Ok(())
        }
        Expression::Call { routine, args } => {
            let info = analyzer.routines.get(routine).cloned().ok_or_else(|| {
                analyzer.annotate(SemanticErrorKind::EntropyMismatch(format!(
                    "unknown routine {}",
                    routine
                )))
            })?;

            if args.len() != info.params.len() {
                return Err(analyzer.annotate(
                    SemanticErrorKind::ArgumentCountMismatch(format!(
                        "routine {} expects {} args, got {}",
                        routine,
                        info.params.len(),
                        args.len()
                    )),
                ));
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

                if let Expression::StructLit(ref type_name, _) = arg_expr {
                    if type_name.borrow().is_none() {
                        if let Type::Custom(ref name) = expected_type {
                            *type_name.borrow_mut() = Some(name.clone());
                        }
                    }
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
        Expression::FieldAccess { target, field } => {
            if let Expression::Identifier(name) = &**target {
                let field_path = format!("{}.{}", name, field);
                let state = analyzer
                    .branch_contexts
                    .get(&analyzer.current_branch)
                    .unwrap();
                if state.consumed.contains(&field_path) {
                    return Err(analyzer
                        .annotate(SemanticErrorKind::UseAfterConsume(field_path)));
                }
                if analyzer.inspection_depth == 0 {
                    analyzer.mark_decayed(name)?;
                    analyzer.mark_consumed(&field_path)?;
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
            analyze_expression_nonconsuming(analyzer, left)?;
            analyze_expression_nonconsuming(analyzer, right)?;
            Ok(())
        }
        Expression::UnaryOp { expr, .. } => {
            analyze_expression_nonconsuming(analyzer, expr)
        }
        Expression::TypeAssertion { target, cast_type } => {
            let target_type = infer_expression_type(analyzer, target)?;
            let cast_type_resolved = Type::from_typename(cast_type);
            match (&target_type, &cast_type_resolved) {
                (Type::Unknown, _) => {}
                (Type::Custom(ref source_name), Type::Custom(ref dest_name)) => {
                    let ok = source_name == dest_name
                        || analyzer.implements_interface(dest_name, source_name)
                        || analyzer.implements_interface(source_name, dest_name);
                    if !ok {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::TypeMismatch(format!(
                                "cannot assert type {} to {}",
                                source_name, dest_name
                            )),
                        ));
                    }
                }
                _ => {
                    return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                        "type assertions require custom struct or interface types"
                            .into(),
                    )));
                }
            }
            analyze_expression(analyzer, target)?;
            Ok(())
        }
    }
}

pub(crate) fn analyze_expression_nonconsuming(
    analyzer: &mut EntropicAnalyzer,
    expr: &Expression,
) -> Result<(), SemanticError> {
    infer_expression_type(analyzer, expr)?;
    match expr {
        Expression::Call { .. } | Expression::MethodCall { .. } => {
            analyze_expression(analyzer, expr)
        }
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
        Expression::FieldAccess { target, field } => {
            if let Expression::Identifier(name) = &**target {
                let field_path = format!("{}.{}", name, field);
                let state = analyzer
                    .branch_contexts
                    .get(&analyzer.current_branch)
                    .unwrap();
                if state.consumed.contains(&field_path) {
                    return Err(analyzer
                        .annotate(SemanticErrorKind::UseAfterConsume(field_path)));
                }
            }
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
        Expression::TypeAssertion { target, .. } => {
            analyze_expression_nonconsuming(analyzer, target)?;
            Ok(())
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
        Expression::MethodCall {
            target,
            method,
            args,
            resolved_routine,
            resolved_budget,
        } => {
            let target_cost = estimate_expression_cost(analyzer, target);
            let arg_cost: u64 = args
                .iter()
                .map(|a| estimate_expression_cost(analyzer, a))
                .sum();
            let routine_opt = resolved_routine.borrow();
            let taking_ms = if let Some(ref routine) = *routine_opt {
                if routine == "<dynamic>" {
                    resolved_budget.borrow().unwrap_or(0)
                } else {
                    analyzer
                        .routines
                        .get(routine)
                        .map(|info| info.taking_ms)
                        .unwrap_or(0)
                }
            } else {
                let target_type =
                    infer_expression_type(analyzer, target).unwrap_or(Type::Unknown);
                if let Type::Custom(struct_name) = target_type {
                    if analyzer.interfaces.contains_key(&struct_name) {
                        let methods = &analyzer.interfaces[&struct_name];
                        methods
                            .iter()
                            .find(|m| &m.name == method)
                            .and_then(|m| m.taking_ms)
                            .unwrap_or(0)
                    } else {
                        let routine_name = format!("{}.{}", struct_name, method);
                        analyzer
                            .routines
                            .get(&routine_name)
                            .map(|info| info.taking_ms)
                            .unwrap_or(0)
                    }
                } else {
                    0
                }
            };
            target_cost + arg_cost + taking_ms
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
        Expression::TypeAssertion { target, .. } => {
            1 + estimate_expression_cost(analyzer, target)
        }
    }
}
