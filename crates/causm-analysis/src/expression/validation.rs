use super::inference::infer_expression_type;
use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::types::Type;
use causm_core::*;

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
            if let Expression::Identifier(ref name) = **target {
                let is_enum_type = analyzer
                    .branch_contexts
                    .get(&analyzer.current_branch)
                    .map(|st| st.custom_types.contains_key(name))
                    .unwrap_or(false);
                if is_enum_type {
                    for arg in args {
                        analyze_expression_nonconsuming(analyzer, arg)?;
                    }
                    *resolved_routine.borrow_mut() =
                        Some("<enum_constructor>".to_string());
                    return Ok(());
                }
            }
            if let Some(ns) = super::inference::get_static_target_path(target) {
                let static_routine_name = format!("{}.{}", ns, method);
                let is_local_var = analyzer
                    .branch_contexts
                    .get(&analyzer.current_branch)
                    .map(|st| st.types.contains_key(&ns))
                    .unwrap_or(false);
                if !is_local_var
                    && analyzer.routines.contains_key(&static_routine_name)
                {
                    let info =
                        analyzer.routines.get(&static_routine_name).unwrap().clone();
                    if !analyzer.capability_stack.is_empty() {
                        for req in &info.requires {
                            let cap_key = if let Some(id) = req.parameters.get("id") {
                                format!("{}[id={}]", req.path, id)
                            } else {
                                req.path.clone()
                            };
                            if !analyzer.is_capability_allowed(&cap_key) {
                                return Err(analyzer.annotate(
                                    SemanticErrorKind::MissingCapability(req.path.clone()),
                                ));
                            }
                        }
                    }
                    if args.len() != info.params.len() {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::ArgumentCountMismatch(format!(
                                "routine {} expects {} args, got {}",
                                static_routine_name,
                                info.params.len(),
                                args.len()
                            )),
                        ));
                    }
                    for (arg_expr, (mode, _param_name, expected_type)) in
                        args.iter().zip(info.params.iter())
                    {
                        let arg_type = infer_expression_type(analyzer, arg_expr)?;
                        let is_ffi_ptr_pass = matches!(
                            (&expected_type, &arg_type),
                            (
                                Type::I64 | Type::I32 | Type::U64 | Type::Integer,
                                Type::Array(_) | Type::Struct(_) | Type::Custom(_)
                            )
                        );
                        if !is_ffi_ptr_pass
                            && !analyzer.types_compatible(expected_type, &arg_type)
                        {
                            return Err(analyzer.annotate(SemanticErrorKind::TypeMismatch(
                                format!(
                                    "routine {} arg type mismatch: expected {:?}, got {:?}",
                                    static_routine_name, expected_type, arg_type
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
                                            SemanticErrorKind::UseAfterConsume(
                                                name.clone(),
                                            ),
                                        ));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    *resolved_routine.borrow_mut() =
                        Some(format!("<static>{}", static_routine_name));
                    let cost = info.taking_ms;
                    let branch = analyzer
                        .branch_contexts
                        .get_mut(&analyzer.current_branch)
                        .unwrap();
                    branch.accumulated_cost += cost;
                    return Ok(());
                }
            }
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

            let (resolved_name, info) = resolved.ok_or_else(|| {
                analyzer.annotate(SemanticErrorKind::EntropyMismatch(format!(
                    "unknown method {} on type {}",
                    method, struct_name
                )))
            })?;
            *resolved_routine.borrow_mut() = Some(resolved_name);

            if !analyzer.capability_stack.is_empty() {
                for req in &info.requires {
                    let cap_key = if let Some(id) = req.parameters.get("id") {
                        format!("{}[id={}]", req.path, id)
                    } else {
                        req.path.clone()
                    };
                    if !analyzer.is_capability_allowed(&cap_key) {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::MissingCapability(req.path.clone()),
                        ));
                    }
                }
            }

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
            let self_type_normalized = match self_type {
                Type::Custom(name) => Type::Custom(
                    name.split('<').next().unwrap_or(name).trim().to_string(),
                ),
                other => other.clone(),
            };
            let target_type_normalized = match &target_type {
                Type::Custom(name) => Type::Custom(
                    name.split('<').next().unwrap_or(name).trim().to_string(),
                ),
                other => other.clone(),
            };
            if !analyzer
                .types_compatible(&self_type_normalized, &target_type_normalized)
            {
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
                let is_ffi_ptr_pass = matches!(
                    (&expected_type, &arg_type),
                    (
                        Type::I64 | Type::I32 | Type::U64 | Type::Integer,
                        Type::Array(_) | Type::Struct(_) | Type::Custom(_)
                    )
                );

                if !is_ffi_ptr_pass
                    && !analyzer.types_compatible(expected_type, &arg_type)
                {
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

            if !analyzer.capability_stack.is_empty() {
                for req in &info.requires {
                    let cap_key = if let Some(id) = req.parameters.get("id") {
                        format!("{}[id={}]", req.path, id)
                    } else {
                        req.path.clone()
                    };
                    if !analyzer.is_capability_allowed(&cap_key) {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::MissingCapability(req.path.clone()),
                        ));
                    }
                }
            }

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
                let is_ffi_ptr_pass = matches!(
                    (&expected_type, &arg_type),
                    (
                        Type::I64 | Type::I32 | Type::U64 | Type::Integer,
                        Type::Array(_) | Type::Struct(_) | Type::Custom(_)
                    )
                );
                if !is_ffi_ptr_pass
                    && !analyzer.types_compatible(expected_type, &arg_type)
                {
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
                    ParamMode::Peek | ParamMode::Lease => {}
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
                let is_borrowed_or_mutable =
                    state.yields.contains(name) || state.mutables.contains(name);
                if !is_borrowed_or_mutable && state.consumed.contains(&field_path) {
                    return Err(analyzer
                        .annotate(SemanticErrorKind::UseAfterConsume(field_path)));
                }
                if !is_borrowed_or_mutable && analyzer.inspection_depth == 0 {
                    analyzer.mark_decayed(name)?;
                    analyzer.mark_consumed(&field_path)?;
                }
                Ok(())
            } else {
                analyze_expression(analyzer, target)
            }
        }
        Expression::RefOp(expr) => analyze_expression_nonconsuming(analyzer, expr),
        Expression::Syscall { args, .. } => {
            if !analyzer.capability_stack.is_empty()
                && !analyzer.is_capability_allowed("System.Syscall")
            {
                return Err(analyzer.annotate(
                    SemanticErrorKind::MissingCapability(
                        "System.Syscall".to_string(),
                    ),
                ));
            }
            for a in args {
                analyze_expression_nonconsuming(analyzer, a)?;
            }
            Ok(())
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
        Expression::StrBytes(expr) => {
            analyze_expression_nonconsuming(analyzer, expr)
        }
        Expression::ToStr(expr) => analyze_expression_nonconsuming(analyzer, expr),
        Expression::Len(expr) => analyze_expression_nonconsuming(analyzer, expr),
        Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
            for inner_expr in fields.values() {
                analyze_expression(analyzer, inner_expr)?;
            }
            Ok(())
        }
        Expression::IndexAccess { target, index } => {
            analyze_expression_nonconsuming(analyzer, target)?;
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
        Expression::ArrayRepeat { value, count } => {
            analyze_expression_nonconsuming(analyzer, value)?;
            analyze_expression_nonconsuming(analyzer, count)?;
            Ok(())
        }
        Expression::ArraySlice {
            target, start, end, ..
        } => {
            analyze_expression_nonconsuming(analyzer, target)?;
            if let Some(s) = start {
                analyze_expression_nonconsuming(analyzer, s)?;
            }
            if let Some(e) = end {
                analyze_expression_nonconsuming(analyzer, e)?;
            }
            Ok(())
        }
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
        Expression::EnumVariant { args, .. } => {
            for a in args {
                analyze_expression(analyzer, a)?;
            }
            Ok(())
        }
        Expression::TypeCast { expr, .. } | Expression::TryUnwrap(expr) => {
            analyze_expression(analyzer, expr)?;
            Ok(())
        }
        Expression::FString(parts) => {
            for part in parts {
                if let causm_core::FStringPart::Expr(e) = part {
                    analyze_expression_nonconsuming(analyzer, e)?;
                }
            }
            Ok(())
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => {
            analyze_expression_nonconsuming(analyzer, condition)?;
            let original_state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .cloned()
                .unwrap_or_default();

            let mut then_contexts = analyzer.branch_contexts.clone();
            then_contexts
                .insert(analyzer.current_branch.clone(), original_state.clone());
            let previous_contexts =
                std::mem::replace(&mut analyzer.branch_contexts, then_contexts);

            analyze_expression(analyzer, then_branch)?;
            let then_end_state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .cloned()
                .unwrap_or_default();

            let mut else_contexts = previous_contexts.clone();
            else_contexts.insert(analyzer.current_branch.clone(), original_state);
            analyzer.branch_contexts = else_contexts;

            analyze_expression(analyzer, else_branch)?;
            let else_end_state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .cloned()
                .unwrap_or_default();

            analyzer.branch_contexts = previous_contexts;
            let merged =
                analyzer.merge_states(then_end_state, else_end_state, &None)?;
            analyzer
                .branch_contexts
                .insert(analyzer.current_branch.clone(), merged);
            Ok(())
        }
        Expression::Match { target, arms } => {
            analyze_expression_nonconsuming(analyzer, target)?;
            let original_state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .cloned()
                .unwrap_or_default();
            for arm in arms {
                let mut arm_contexts = analyzer.branch_contexts.clone();
                arm_contexts
                    .insert(analyzer.current_branch.clone(), original_state.clone());
                let previous =
                    std::mem::replace(&mut analyzer.branch_contexts, arm_contexts);
                crate::statements::control_flow::bind_pattern_variables(
                    analyzer,
                    &arm.pattern,
                );
                if let Some(ref g) = arm.guard {
                    analyze_expression(analyzer, g)?;
                }
                analyze_expression(analyzer, &arm.body)?;
                analyzer.branch_contexts = previous;
            }
            Ok(())
        }
        Expression::ArenaIntrospect(_) | Expression::HasCapability(_) => Ok(()),
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
        Expression::RefOp(expr) => analyze_expression_nonconsuming(analyzer, expr),
        Expression::Syscall { args, .. } => {
            if !analyzer.capability_stack.is_empty()
                && !analyzer.is_capability_allowed("System.Syscall")
            {
                return Err(analyzer.annotate(
                    SemanticErrorKind::MissingCapability(
                        "System.Syscall".to_string(),
                    ),
                ));
            }
            for a in args {
                analyze_expression_nonconsuming(analyzer, a)?;
            }
            Ok(())
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
        Expression::StrBytes(expr) => {
            analyze_expression_nonconsuming(analyzer, expr)
        }
        Expression::ToStr(expr) => analyze_expression_nonconsuming(analyzer, expr),
        Expression::Len(expr) => analyze_expression_nonconsuming(analyzer, expr),
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
        Expression::ArrayRepeat { value, count } => {
            analyze_expression_nonconsuming(analyzer, value)?;
            analyze_expression_nonconsuming(analyzer, count)?;
            Ok(())
        }
        Expression::ArraySlice {
            target, start, end, ..
        } => {
            analyze_expression_nonconsuming(analyzer, target)?;
            if let Some(s) = start {
                analyze_expression_nonconsuming(analyzer, s)?;
            }
            if let Some(e) = end {
                analyze_expression_nonconsuming(analyzer, e)?;
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
        | Expression::ArenaIntrospect(_)
        | Expression::HasCapability(_)
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
        Expression::EnumVariant { args, .. } => {
            for arg in args {
                analyze_expression_nonconsuming(analyzer, arg)?;
            }
            Ok(())
        }
        Expression::TypeCast { expr, .. } | Expression::TryUnwrap(expr) => {
            analyze_expression_nonconsuming(analyzer, expr)?;
            Ok(())
        }
        Expression::FString(parts) => {
            for part in parts {
                if let causm_core::FStringPart::Expr(e) = part {
                    analyze_expression_nonconsuming(analyzer, e)?;
                }
            }
            Ok(())
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => {
            analyze_expression_nonconsuming(analyzer, condition)?;
            analyze_expression_nonconsuming(analyzer, then_branch)?;
            analyze_expression_nonconsuming(analyzer, else_branch)?;
            Ok(())
        }
        Expression::Match { target, arms } => {
            analyze_expression_nonconsuming(analyzer, target)?;
            let original_state = analyzer
                .branch_contexts
                .get(&analyzer.current_branch)
                .cloned()
                .unwrap_or_default();
            for arm in arms {
                let mut arm_contexts = analyzer.branch_contexts.clone();
                arm_contexts
                    .insert(analyzer.current_branch.clone(), original_state.clone());
                let previous =
                    std::mem::replace(&mut analyzer.branch_contexts, arm_contexts);
                crate::statements::control_flow::bind_pattern_variables(
                    analyzer,
                    &arm.pattern,
                );
                if let Some(ref g) = arm.guard {
                    analyze_expression_nonconsuming(analyzer, g)?;
                }
                analyze_expression_nonconsuming(analyzer, &arm.body)?;
                analyzer.branch_contexts = previous;
            }
            Ok(())
        }
    }
}
