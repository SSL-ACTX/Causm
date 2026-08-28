use super::context::LoweringContext;
use causm_core::{BinaryOperator, Expression, FStringPart, Pattern, TypeFieldDef};
use causm_ir::Reg;
use std::collections::HashMap;

pub fn lower_expression(ctx: &mut LoweringContext, expr: &Expression) -> Reg {
    match expr {
        Expression::Integer(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadInt { dest, value: *v });
            dest
        }
        Expression::Float(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadFloat { dest, value: *v });
            dest
        }
        Expression::Boolean(v) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadBool { dest, value: *v });
            dest
        }
        Expression::Literal(s) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadString {
                dest,
                value: s.clone(),
            });
            dest
        }
        Expression::Null => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadNull { dest });
            dest
        }
        Expression::ArenaIntrospect(kind) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ArenaIntrospect { dest, kind: *kind });
            dest
        }
        Expression::CapabilityCheck(capability) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::CapabilityCheck {
                dest,
                capability: capability.clone(),
            });
            dest
        }
        Expression::Identifier(name) => ctx.get_reg(name),
        Expression::Tuple(elems) => {
            let elem_regs: Vec<causm_ir::Reg> =
                elems.iter().map(|e| lower_expression(ctx, e)).collect();
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::TupleLit {
                dest,
                elems: elem_regs,
            });
            dest
        }
        Expression::BinaryOp { left, op, right } => {
            let l = lower_expression(ctx, left);
            let r = lower_expression(ctx, right);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::BinaryOp {
                dest,
                op: *op,
                left: l,
                right: r,
            });
            dest
        }
        Expression::UnaryOp { op, expr } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::UnaryOp { dest, op: *op, src });
            dest
        }

        Expression::MethodCall {
            target,
            method,
            args,
            resolved_routine,
            resolved_budget,
        } => {
            let routine_name = resolved_routine
                .borrow()
                .clone()
                .or_else(|| {
                    let mut parts = Vec::new();
                    let mut curr: &Expression = target;
                    while let Expression::FieldAccess {
                        target: next_t,
                        field: f,
                    } = curr
                    {
                        parts.push(f.as_str());
                        curr = next_t;
                    }
                    if let Expression::Identifier(ref base) = curr {
                        parts.push(base.as_str());
                        parts.reverse();
                        let base_path = parts.join(".");
                        let full_static = format!("{}.{}", base_path, method);
                        if ctx.routines.contains_key(&full_static) {
                            Some(format!("<static>{}", full_static))
                        } else {
                            Some(format!("{}.{}", base_path, method))
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    panic!("MethodCall was not resolved during semantic analysis");
                });
            let is_static = routine_name.starts_with("<static>");
            let actual_routine_name = if is_static {
                routine_name.trim_start_matches("<static>").to_string()
            } else {
                routine_name.clone()
            };
            let mut arg_regs = Vec::new();
            if !is_static && routine_name != "<enum_constructor>" {
                arg_regs.push(lower_expression(ctx, target));
            }
            for arg in args {
                arg_regs.push(lower_expression(ctx, arg));
            }
            let dest = ctx.alloc_reg();
            if routine_name == "<enum_constructor>" {
                let mut payload_regs = Vec::new();
                for arg in args {
                    payload_regs.push(lower_expression(ctx, arg));
                }
                let src = if !payload_regs.is_empty() {
                    payload_regs[0]
                } else {
                    ctx.alloc_reg()
                };
                ctx.push(causm_ir::Instruction::Move { dest, src });
            } else if routine_name == "<dynamic>" {
                let budget = *resolved_budget.borrow();
                ctx.push(causm_ir::Instruction::DynamicCall {
                    method: method.clone(),
                    args: arg_regs,
                    dest,
                    budget,
                });
            } else {
                if let Some(r) = ctx.routines.get(&actual_routine_name) {
                    match &r.return_type {
                        causm_core::types::Type::Custom(t) => {
                            ctx.reg_types.insert(dest.0, t.clone());
                        }
                        causm_core::types::Type::Struct(s) => {
                            if let Some(ref spec) = s.auto_drop {
                                ctx.auto_drop_specs.insert(
                                    format!("_reg_{}", dest.0),
                                    spec.clone(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                ctx.push(causm_ir::Instruction::Call {
                    routine: actual_routine_name,
                    args: arg_regs,
                    dest,
                });
            }
            dest
        }

        Expression::Call { routine, args } => {
            let mut arg_regs = Vec::new();
            for arg in args {
                arg_regs.push(lower_expression(ctx, arg));
            }
            let dest = ctx.alloc_reg();
            if let Some(r) = ctx.routines.get(routine) {
                match &r.return_type {
                    causm_core::types::Type::Custom(t) => {
                        ctx.reg_types.insert(dest.0, t.clone());
                    }
                    causm_core::types::Type::Struct(s) => {
                        if let Some(ref spec) = s.auto_drop {
                            ctx.auto_drop_specs
                                .insert(format!("_reg_{}", dest.0), spec.clone());
                        }
                    }
                    _ => {}
                }
            }
            ctx.push(causm_ir::Instruction::Call {
                routine: routine.clone(),
                args: arg_regs,
                dest,
            });
            dest
        }
        Expression::CloneOp(name) => {
            let src = ctx.get_reg(name);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Clone { dest, src });
            dest
        }
        Expression::StrBytes(expr) => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::StrBytes { dest, src });
            dest
        }
        Expression::ToStr(expr) => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ToStr { dest, src });
            dest
        }
        Expression::Len(expr) => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ArrayLen { dest, src });
            dest
        }
        Expression::FieldAccess { target, field } => {
            let mut const_expr = None;
            if let Expression::Identifier(ref name) = &**target {
                if let Some(fields_map) = ctx.type_decls.get(name) {
                    if let Some(field_def) = fields_map.get(field) {
                        if field_def.is_const {
                            if let Some(ref val_expr) = field_def.default_value {
                                const_expr = Some(val_expr.clone());
                            }
                        }
                    }
                }
            }
            if let Some(expr) = const_expr {
                return lower_expression(ctx, &expr);
            }
            let t = lower_expression(ctx, target);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::FieldAccess {
                dest,
                target: t,
                field: field.clone(),
            });
            dest
        }
        Expression::IndexAccess { target, index } => {
            let t = lower_expression(ctx, target);
            let i = lower_expression(ctx, index);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::IndexAccess {
                dest,
                target: t,
                index: i,
            });
            dest
        }
        Expression::StructLit(type_name, fields) => {
            let mut field_regs = HashMap::new();
            let mut sorted_fields: Vec<(&String, &Expression)> =
                fields.iter().collect();
            sorted_fields.sort_by_key(|(name, _)| *name);
            for (name, expr) in sorted_fields {
                field_regs.insert(name.clone(), lower_expression(ctx, expr));
            }
            let type_name_opt = type_name.borrow().clone();
            let mut defaults_to_lower = Vec::new();
            if let Some(ref name) = type_name_opt {
                if let Some(fields_map) = ctx.type_decls.get(name) {
                    let mut sorted_fields_map: Vec<(&String, &TypeFieldDef)> =
                        fields_map.iter().collect();
                    sorted_fields_map.sort_by_key(|(name, _)| *name);
                    for (field_name, field_def) in sorted_fields_map {
                        if !field_def.is_const
                            && !field_regs.contains_key(field_name)
                        {
                            if let Some(ref default_expr) = field_def.default_value {
                                defaults_to_lower.push((
                                    field_name.clone(),
                                    default_expr.clone(),
                                ));
                            }
                        }
                    }
                }
            }
            for (field_name, expr) in defaults_to_lower {
                field_regs.insert(field_name, lower_expression(ctx, &expr));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::StructLit {
                dest,
                fields: field_regs,
                type_name: type_name_opt,
            });
            dest
        }
        Expression::TopologyLit(fields) => {
            let mut field_regs = HashMap::new();
            let mut sorted_fields: Vec<(&String, &Expression)> =
                fields.iter().collect();
            sorted_fields.sort_by_key(|(name, _)| *name);
            for (name, expr) in sorted_fields {
                field_regs.insert(name.clone(), lower_expression(ctx, expr));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::TopologyLit {
                dest,
                fields: field_regs,
            });
            dest
        }
        Expression::ArrayLiteral(elements) => {
            let mut elem_regs = Vec::new();
            for e in elements {
                elem_regs.push(lower_expression(ctx, e));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ArrayLit {
                dest,
                elements: elem_regs,
            });
            dest
        }
        Expression::ArrayRepeat { value, count } => {
            let val_reg = lower_expression(ctx, value);
            let count_reg = lower_expression(ctx, count);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ArrayRepeat {
                dest,
                value: val_reg,
                count: count_reg,
            });
            dest
        }
        Expression::ArraySlice {
            target,
            start,
            end,
            inclusive,
        } => {
            let target_reg = lower_expression(ctx, target);
            let start_reg = start.as_ref().map(|s| lower_expression(ctx, s));
            let end_reg = end.as_ref().map(|e| lower_expression(ctx, e));
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ArraySlice {
                dest,
                target: target_reg,
                start: start_reg,
                end: end_reg,
                inclusive: *inclusive,
            });
            dest
        }
        Expression::ChannelReceive(chan_id) => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::ChanRecv {
                dest,
                chan_id: chan_id.clone(),
            });
            dest
        }
        Expression::Syscall {
            target,
            args,
            duration_ms,
        } => {
            let mut arg_regs = Vec::new();
            for a in args {
                arg_regs.push(lower_expression(ctx, a));
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Syscall {
                dest,
                target: target.clone(),
                args: arg_regs,
                duration_ms: *duration_ms,
            });
            dest
        }
        Expression::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => {
            let tag_reg = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadString {
                dest: tag_reg,
                value: variant_name.clone(),
            });
            let mut fields_map = HashMap::new();
            fields_map.insert("tag".to_string(), tag_reg);
            for (idx, arg_expr) in args.iter().enumerate() {
                let arg_reg = lower_expression(ctx, arg_expr);
                fields_map.insert(format!("_{}", idx), arg_reg);
            }
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::StructLit {
                dest,
                fields: fields_map,
                type_name: Some(format!("{}::{}", enum_name, variant_name)),
            });
            dest
        }
        Expression::Deferred {
            capability,
            params,
            deadline_ms,
        } => {
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Defer {
                dest,
                cap: causm_core::Capability {
                    path: capability.clone(),
                    parameters: params.clone(),
                },
                deadline_ms: *deadline_ms,
            });
            dest
        }
        Expression::TypeAssertion { target, cast_type } => {
            let src = lower_expression(ctx, target);
            let dest = ctx.alloc_reg();
            let type_name = match cast_type {
                causm_core::TypeName::Custom(ref s) => s.clone(),
                _ => panic!("Type assertion target must be a custom type name"),
            };
            ctx.push(causm_ir::Instruction::TypeAssert {
                dest,
                src,
                type_name,
            });
            dest
        }
        Expression::TypeCast { expr, target_type } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::TypeCast {
                dest,
                src,
                target_type: target_type.clone(),
            });
            dest
        }
        Expression::TryUnwrap(expr) => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Move { dest, src });
            let is_null_reg = ctx.alloc_reg();
            let null_val_reg = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::LoadNull { dest: null_val_reg });
            ctx.push(causm_ir::Instruction::BinaryOp {
                dest: is_null_reg,
                op: causm_core::BinaryOperator::Eq,
                left: src,
                right: null_val_reg,
            });
            let jump_idx = ctx.instructions.len();
            ctx.push(causm_ir::Instruction::JumpIfNot {
                cond: is_null_reg,
                target: 0,
            });
            ctx.push(causm_ir::Instruction::Return {
                src: Some(null_val_reg),
            });
            let resume_target = ctx.instructions.len();
            if let Some(causm_ir::Instruction::JumpIfNot { target, .. }) =
                ctx.instructions.get_mut(jump_idx)
            {
                *target = resume_target;
            }
            dest
        }
        Expression::Turbofish { expr, .. } => lower_expression(ctx, expr),
        Expression::GenericStaticCall {
            type_name,
            method,
            args,
            ..
        } => {
            let routine = format!("{}.{}", type_name, method);
            let lowered_args: Vec<_> =
                args.iter().map(|a| lower_expression(ctx, a)).collect();
            let dest = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::Call {
                dest,
                routine,
                args: lowered_args,
            });
            dest
        }
        Expression::RefOp(expr) => lower_expression(ctx, expr),
        Expression::FString(parts) => {
            // Build a concat chain across all parts.
            // Empty fstring → empty string reg.
            if parts.is_empty() {
                let dest = ctx.alloc_reg();
                ctx.push(causm_ir::Instruction::LoadString {
                    dest,
                    value: String::new(),
                });
                return dest;
            }
            let lower_part = |ctx: &mut LoweringContext,
                              part: &FStringPart|
             -> Reg {
                match part {
                    FStringPart::Text(t) => {
                        let dest = ctx.alloc_reg();
                        ctx.push(causm_ir::Instruction::LoadString {
                            dest,
                            value: t.clone(),
                        });
                        dest
                    }
                    FStringPart::Expr(e) => {
                        let val = lower_expression(ctx, e);
                        let dest = ctx.alloc_reg();
                        ctx.push(causm_ir::Instruction::ToStr { dest, src: val });
                        dest
                    }
                }
            };
            let mut acc = lower_part(ctx, &parts[0]);
            for part in &parts[1..] {
                let rhs = lower_part(ctx, part);
                let dest = ctx.alloc_reg();
                ctx.push(causm_ir::Instruction::BinaryOp {
                    dest,
                    op: causm_core::BinaryOperator::Add,
                    left: acc,
                    right: rhs,
                });
                acc = dest;
            }
            acc
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond_reg = lower_expression(ctx, condition);
            let dest_reg = ctx.alloc_reg();

            if is_pure_scalar_expr(then_branch) && is_pure_scalar_expr(else_branch) {
                let true_reg = lower_expression(ctx, then_branch);
                let false_reg = lower_expression(ctx, else_branch);
                ctx.push(causm_ir::Instruction::ConditionalSelect {
                    dest: dest_reg,
                    cond: cond_reg,
                    true_val: true_reg,
                    false_val: false_reg,
                });
            } else {
                let jump_to_else_idx = ctx.instructions.len();
                ctx.push(causm_ir::Instruction::JumpIfNot {
                    cond: cond_reg,
                    target: 0,
                });

                let then_reg = lower_expression(ctx, then_branch);
                ctx.push(causm_ir::Instruction::Move {
                    dest: dest_reg,
                    src: then_reg,
                });

                let jump_to_end_idx = ctx.instructions.len();
                ctx.push(causm_ir::Instruction::Jump { target: 0 });

                let else_start_idx = ctx.instructions.len();
                if let causm_ir::Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_to_else_idx]
                {
                    *target = else_start_idx;
                }

                let else_reg = lower_expression(ctx, else_branch);
                ctx.push(causm_ir::Instruction::Move {
                    dest: dest_reg,
                    src: else_reg,
                });

                let end_idx = ctx.instructions.len();
                if let causm_ir::Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_to_end_idx]
                {
                    *target = end_idx;
                }
            }

            dest_reg
        }
        Expression::Match { target, arms } => {
            let target_reg = lower_expression(ctx, target);
            let result_reg = ctx.alloc_reg();
            let mut exit_jumps = Vec::new();

            for arm in arms {
                let mut fail_jumps = Vec::new();
                let mut bound_symbols = Vec::new();

                lower_pattern_test(
                    ctx,
                    target_reg,
                    &arm.pattern,
                    &mut fail_jumps,
                    &mut bound_symbols,
                );

                if let Some(ref guard) = arm.guard {
                    let guard_reg = lower_expression(ctx, guard);
                    fail_jumps.push(ctx.instructions.len());
                    ctx.push(causm_ir::Instruction::JumpIfNot {
                        cond: guard_reg,
                        target: 0,
                    });
                }

                let body_reg = lower_expression(ctx, &arm.body);
                ctx.push(causm_ir::Instruction::Move {
                    dest: result_reg,
                    src: body_reg,
                });

                for (name, old) in bound_symbols.into_iter().rev() {
                    if let Some(o) = old {
                        ctx.symbols.insert(name, o);
                    } else {
                        ctx.symbols.remove(&name);
                    }
                }

                exit_jumps.push(ctx.instructions.len());
                ctx.push(causm_ir::Instruction::Jump { target: 0 });

                let next_arm_target = ctx.instructions.len();
                for fail_idx in fail_jumps {
                    if let causm_ir::Instruction::JumpIfNot {
                        ref mut target, ..
                    } = ctx.instructions[fail_idx]
                    {
                        *target = next_arm_target;
                    }
                }
            }

            let end_target = ctx.instructions.len();
            for exit_idx in exit_jumps {
                if let causm_ir::Instruction::Jump { ref mut target } =
                    ctx.instructions[exit_idx]
                {
                    *target = end_target;
                }
            }
            result_reg
        }
    }
}

pub(crate) fn lower_pattern_test(
    ctx: &mut LoweringContext,
    target_reg: Reg,
    pattern: &Pattern,
    fail_jumps: &mut Vec<usize>,
    bound_symbols: &mut Vec<(String, Option<Reg>)>,
) {
    match pattern {
        Pattern::Wildcard => {}
        Pattern::Identifier(name) => {
            let old = ctx.symbols.insert(name.clone(), target_reg);
            bound_symbols.push((name.clone(), old));
        }
        Pattern::Tuple(subpatterns) => {
            for (idx, subpat) in subpatterns.iter().enumerate() {
                let elem_reg = ctx.alloc_reg();
                ctx.push(causm_ir::Instruction::TupleAccess {
                    dest: elem_reg,
                    tuple: target_reg,
                    index: idx,
                });
                lower_pattern_test(
                    ctx,
                    elem_reg,
                    subpat,
                    fail_jumps,
                    bound_symbols,
                );
            }
        }
        Pattern::Literal(lit_expr) => {
            let lit_reg = lower_expression(ctx, lit_expr);
            let cmp_reg = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::BinaryOp {
                op: BinaryOperator::Eq,
                dest: cmp_reg,
                left: target_reg,
                right: lit_reg,
            });
            fail_jumps.push(ctx.instructions.len());
            ctx.push(causm_ir::Instruction::JumpIfNot {
                cond: cmp_reg,
                target: 0,
            });
        }
        Pattern::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => {
            let dest_reg = ctx.alloc_reg();
            let success_reg = ctx.alloc_reg();
            ctx.push(causm_ir::Instruction::TryEnumVariant {
                dest: dest_reg,
                src: target_reg,
                enum_name: enum_name.clone(),
                variant_name: variant_name.clone(),
                success: success_reg,
            });
            fail_jumps.push(ctx.instructions.len());
            ctx.push(causm_ir::Instruction::JumpIfNot {
                cond: success_reg,
                target: 0,
            });
            for (idx, arg_pat) in args.iter().enumerate() {
                let field_reg = ctx.alloc_reg();
                ctx.push(causm_ir::Instruction::FieldAccess {
                    dest: field_reg,
                    target: dest_reg,
                    field: format!("_{}", idx),
                });
                lower_pattern_test(
                    ctx,
                    field_reg,
                    arg_pat,
                    fail_jumps,
                    bound_symbols,
                );
            }
        }
        Pattern::TypeAssert {
            binding,
            target_type,
        } => {
            let dest_reg = ctx.alloc_reg();
            let success_reg = ctx.alloc_reg();
            let type_name_str = match target_type {
                causm_core::TypeName::Custom(ref s) => s.clone(),
                causm_core::TypeName::Builtin(b) => format!("{:?}", b),
                _ => format!("{:?}", target_type),
            };
            ctx.push(causm_ir::Instruction::TryTypeAssert {
                dest: dest_reg,
                src: target_reg,
                type_name: type_name_str,
                success: success_reg,
            });
            fail_jumps.push(ctx.instructions.len());
            ctx.push(causm_ir::Instruction::JumpIfNot {
                cond: success_reg,
                target: 0,
            });
            let old = ctx.symbols.insert(binding.clone(), dest_reg);
            bound_symbols.push((binding.clone(), old));
        }
    }
}

fn is_pure_scalar_expr(expr: &Expression) -> bool {
    match expr {
        Expression::Integer(_)
        | Expression::Float(_)
        | Expression::Boolean(_)
        | Expression::Null => true,
        Expression::BinaryOp { left, right, .. } => {
            is_pure_scalar_expr(left) && is_pure_scalar_expr(right)
        }
        Expression::UnaryOp { expr, .. } => is_pure_scalar_expr(expr),
        _ => false,
    }
}
