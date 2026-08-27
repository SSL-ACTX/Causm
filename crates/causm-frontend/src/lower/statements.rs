use super::context::LoweringContext;
use super::expressions::{lower_expression, lower_pattern_test};
use causm_core::{
    BinaryOperator, DecayedPattern, Expression, PatternValue, SpannedStatement,
    Statement,
};
use causm_ir::{Instruction, IrRoutine, IrSelectCase, Reg};
use std::collections::HashMap;

pub fn lower_spanned(ctx: &mut LoweringContext, spanned: &SpannedStatement) {
    let old_span = ctx.current_span.clone();
    ctx.current_span = Some(spanned.span.clone());
    lower_statement(ctx, &spanned.stmt);
    ctx.current_span = old_span;
}

pub fn lower_statement(ctx: &mut LoweringContext, stmt: &Statement) {
    match stmt {
        Statement::RoutineDef {
            name,
            params,
            return_type,
            taking_ms,
            state_constraint,
            required_capabilities,
            body,
        } => {
            let mut sub_ctx = LoweringContext::new();
            sub_ctx.type_decls = ctx.type_decls.clone();
            sub_ctx.type_decay_limits = ctx.type_decay_limits.clone();
            sub_ctx.routines = ctx.routines.clone();

            for (i, param) in params.iter().enumerate() {
                let p_reg = Reg(i as u32);
                sub_ctx.symbols.insert(param.name.clone(), p_reg);
            }
            sub_ctx.next_reg = params.len() as u32;

            if let Some((ref param_name, ref expected_state)) = state_constraint {
                if let Some(&reg) = sub_ctx.symbols.get(param_name) {
                    sub_ctx.push(Instruction::AssertState {
                        src: reg,
                        state: expected_state.clone(),
                    });
                }
            }

            for (i, s) in body.iter().enumerate() {
                if i == body.len() - 1 {
                    if let Statement::Expression(ref expr) = s.stmt {
                        let ret_reg = lower_expression(&mut sub_ctx, expr);
                        sub_ctx.push(Instruction::Return { src: Some(ret_reg) });
                        continue;
                    } else if let Statement::If {
                        ref binding,
                        ref condition,
                        ref then_branch,
                        ref else_branch,
                        reconcile: _,
                        ..
                    } = s.stmt
                    {
                        if binding.is_none() && else_branch.is_some() {
                            let else_branch_stmts = else_branch.as_ref().unwrap();
                            let cond_reg = lower_expression(&mut sub_ctx, condition);
                            let ret_reg = sub_ctx.alloc_reg();

                            let jump_to_else_idx = sub_ctx.instructions.len();
                            sub_ctx.push(Instruction::JumpIfNot {
                                cond: cond_reg,
                                target: 0,
                            });

                            fn lower_branch_tail(
                                ctx: &mut LoweringContext,
                                stmts: &[SpannedStatement],
                                ret_reg: Reg,
                            ) {
                                for (i, s) in stmts.iter().enumerate() {
                                    if i == stmts.len() - 1 {
                                        match &s.stmt {
                                            Statement::Expression(e) => {
                                                let res = lower_expression(ctx, e);
                                                ctx.push(Instruction::Move {
                                                    dest: ret_reg,
                                                    src: res,
                                                });
                                            }
                                            Statement::If {
                                                condition,
                                                then_branch,
                                                else_branch,
                                                ..
                                            } => {
                                                let cond_reg =
                                                    lower_expression(ctx, condition);
                                                let jump_to_else =
                                                    ctx.instructions.len();
                                                ctx.push(Instruction::JumpIfNot {
                                                    cond: cond_reg,
                                                    target: 0,
                                                });
                                                lower_branch_tail(
                                                    ctx,
                                                    then_branch,
                                                    ret_reg,
                                                );
                                                let jump_to_end =
                                                    ctx.instructions.len();
                                                ctx.push(Instruction::Jump {
                                                    target: 0,
                                                });
                                                let else_start =
                                                    ctx.instructions.len();
                                                if let Instruction::JumpIfNot {
                                                    ref mut target,
                                                    ..
                                                } = ctx.instructions[jump_to_else]
                                                {
                                                    *target = else_start;
                                                }
                                                if let Some(eb) = else_branch {
                                                    lower_branch_tail(
                                                        ctx, eb, ret_reg,
                                                    );
                                                }
                                                let end_pc = ctx.instructions.len();
                                                if let Instruction::Jump {
                                                    ref mut target,
                                                    ..
                                                } = ctx.instructions[jump_to_end]
                                                {
                                                    *target = end_pc;
                                                }
                                            }
                                            _ => lower_spanned(ctx, s),
                                        }
                                    } else {
                                        lower_spanned(ctx, s);
                                    }
                                }
                            }

                            lower_branch_tail(&mut sub_ctx, then_branch, ret_reg);

                            let jump_to_end_idx = sub_ctx.instructions.len();
                            sub_ctx.push(Instruction::Jump { target: 0 });

                            let else_start_idx = sub_ctx.instructions.len();
                            if let Instruction::JumpIfNot {
                                ref mut target, ..
                            } = sub_ctx.instructions[jump_to_else_idx]
                            {
                                *target = else_start_idx;
                            }

                            lower_branch_tail(
                                &mut sub_ctx,
                                else_branch_stmts,
                                ret_reg,
                            );

                            let end_idx = sub_ctx.instructions.len();
                            if let Instruction::Jump { ref mut target, .. } =
                                sub_ctx.instructions[jump_to_end_idx]
                            {
                                *target = end_idx;
                            }

                            sub_ctx.push(Instruction::Return { src: Some(ret_reg) });
                            continue;
                        }
                    } else if let Statement::Match {
                        ref target,
                        ref arms,
                    } = s.stmt
                    {
                        let all_exprs = arms.iter().all(|a| {
                            a.body.len() == 1
                                && matches!(a.body[0].stmt, Statement::Expression(_))
                        });
                        if all_exprs {
                            let expr_arms = arms
                                .iter()
                                .map(|a| {
                                    if let Statement::Expression(ref e) =
                                        a.body[0].stmt
                                    {
                                        causm_core::MatchExprArm {
                                            pattern: a.pattern.clone(),
                                            guard: a.guard.clone(),
                                            body: e.clone(),
                                        }
                                    } else {
                                        unreachable!()
                                    }
                                })
                                .collect();
                            let match_expr = Expression::Match {
                                target: Box::new(target.clone()),
                                arms: expr_arms,
                            };
                            let ret_reg =
                                lower_expression(&mut sub_ctx, &match_expr);
                            sub_ctx.push(Instruction::Return { src: Some(ret_reg) });
                            continue;
                        }
                    }
                }
                lower_spanned(&mut sub_ctx, s);
            }

            let routine = IrRoutine {
                params: params
                    .iter()
                    .map(|p| {
                        (
                            p.mode.clone(),
                            p.name.clone(),
                            p.typ
                                .as_ref()
                                .map(causm_core::types::Type::from_typename)
                                .unwrap_or(causm_core::types::Type::Unknown),
                        )
                    })
                    .collect(),
                return_type: return_type
                    .as_ref()
                    .map(causm_core::types::Type::from_typename)
                    .unwrap_or(causm_core::types::Type::Unknown),
                taking_ms: taking_ms.or_else(|| {
                    let cost = causm_core::Statement::RoutineDef {
                        name: name.clone(),
                        params: params.clone(),
                        return_type: return_type.clone(),
                        taking_ms: None,
                        state_constraint: state_constraint.clone(),
                        required_capabilities: required_capabilities.clone(),
                        body: body.clone(),
                    }
                    .estimate_cost(|b| {
                        b.iter()
                            .map(|s| {
                                s.stmt.estimate_cost(|inner_b| inner_b.len() as u64)
                            })
                            .sum::<u64>()
                    });
                    Some(cost.max(1))
                }),
                foreign_binding: None,
                instructions: sub_ctx.instructions,
                spans: sub_ctx.spans,
            };
            let base_name = if let Some(angle_idx) = name.find('<') {
                if let Some(dot_idx) = name.find('.') {
                    let struct_part = &name[..angle_idx];
                    let method_part = &name[dot_idx..];
                    format!("{}{}", struct_part, method_part)
                } else {
                    name.clone()
                }
            } else {
                name.clone()
            };

            ctx.routines.insert(name.clone(), routine.clone());
            if base_name != *name {
                ctx.routines.insert(base_name, routine);
            }
        }
        Statement::Yield(expr_opt) | Statement::Return(expr_opt) => {
            if let Some(expr) = expr_opt {
                let src = lower_expression(ctx, expr);
                ctx.push(Instruction::Return { src: Some(src) });
            } else {
                ctx.push(Instruction::Return { src: None });
            }
        }
        Statement::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            let spec_idx = ctx.instructions.len();
            ctx.push(Instruction::Speculate {
                max_ms: *max_ms,
                fallback_target: 0,
            });

            for s in body {
                lower_spanned(ctx, s);
            }

            let end_spec_idx = ctx.instructions.len();
            ctx.push(Instruction::EndSpeculate {
                max_ms: *max_ms,
                fallback_target: 0,
            });

            let jump_over_fallback_idx = ctx.instructions.len();
            ctx.push(Instruction::Jump { target: 0 });

            let fallback_start_idx = ctx.instructions.len();
            if let Instruction::Speculate {
                ref mut fallback_target,
                ..
            } = ctx.instructions[spec_idx]
            {
                *fallback_target = fallback_start_idx;
            }
            if let Instruction::EndSpeculate {
                ref mut fallback_target,
                ..
            } = ctx.instructions[end_spec_idx]
            {
                *fallback_target = fallback_start_idx;
            }

            if let Some(fb) = fallback {
                for s in fb {
                    lower_spanned(ctx, s);
                }
            }

            let end_idx = ctx.instructions.len();
            if let Instruction::Jump { ref mut target, .. } =
                ctx.instructions[jump_over_fallback_idx]
            {
                *target = end_idx;
            }
        }
        Statement::Collapse => {
            ctx.push(Instruction::Collapse);
        }
        Statement::SpeculationMode(mode) => {
            ctx.push(Instruction::SpeculationMode { mode: *mode });
        }
        Statement::Select {
            max_ms,
            cases,
            timeout,
            ..
        } => {
            let select_idx = ctx.instructions.len();
            ctx.push(Instruction::Select {
                max_ms: *max_ms,
                cases: Vec::new(),
                timeout_target: None,
            });

            let mut ir_cases = Vec::new();
            let mut case_jumps = Vec::new();

            for case in cases {
                let chan_id = match &case.source {
                    Expression::ChannelReceive(id) => id.clone(),
                    _ => "".to_string(),
                };
                let dest = ctx.get_reg(&case.binding);
                let target = ctx.instructions.len();

                for s in &case.body {
                    lower_spanned(ctx, s);
                }
                case_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });

                ir_cases.push(IrSelectCase {
                    chan_id,
                    dest,
                    target,
                });
            }

            if let Instruction::Select { ref mut cases, .. } =
                ctx.instructions[select_idx]
            {
                *cases = ir_cases;
            }

            if let Some(t) = timeout {
                let timeout_start = ctx.instructions.len();
                if let Instruction::Select {
                    ref mut timeout_target,
                    ..
                } = ctx.instructions[select_idx]
                {
                    *timeout_target = Some(timeout_start);
                }
                for s in t {
                    lower_spanned(ctx, s);
                }
            }

            let end_idx = ctx.instructions.len();
            for jump_idx in case_jumps {
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::RelativisticBlock { time, body } => {
            if let causm_core::TimeCoordinate::Periodic(interval_ms) = time {
                ctx.push(Instruction::FreezeBaseWatermark);
                for s in body {
                    lower_spanned(ctx, s);
                }
                ctx.push(Instruction::ResetBaseWatermark);
                ctx.push(Instruction::EndPeriodicEpoch {
                    interval_ms: *interval_ms,
                });
            } else {
                let target = match time {
                    causm_core::TimeCoordinate::Branch(b) => b.clone(),
                    _ => "main".to_string(),
                };

                let rb_instr_idx = ctx.instructions.len();
                ctx.push(Instruction::RelativisticBlock {
                    target: target.clone(),
                    block_pc: 0,
                    block_len: 0,
                });

                let jump_over_idx = ctx.instructions.len();
                ctx.push(Instruction::Jump { target: 0 });

                let start_pc = ctx.instructions.len();
                for s in body {
                    lower_spanned(ctx, s);
                }
                let len = ctx.instructions.len() - start_pc;

                if let Instruction::RelativisticBlock {
                    ref mut block_pc,
                    ref mut block_len,
                    ..
                } = ctx.instructions[rb_instr_idx]
                {
                    *block_pc = start_pc;
                    *block_len = len;
                }

                let end_idx = ctx.instructions.len();
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_over_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::MatchEntropy {
            target,
            valid_branch,
            decayed_branch,
            pending_branch,
            consumed_branch,
        } => {
            let target_reg = lower_expression(ctx, target);
            let match_idx = ctx.instructions.len();
            ctx.push(Instruction::MatchEntropy {
                target: target_reg,
                valid_target: None,
                decayed_target: None,
                pending_target: None,
                consumed_target: None,
            });

            let mut branch_jumps = Vec::new();
            let mut mismatch_jumps = Vec::new();

            if let Some((pattern, guard, body)) = valid_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut valid_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *valid_target = Some(start);
                }

                match pattern {
                    DecayedPattern::Binding(binding) => {
                        if !binding.is_empty() {
                            let dest = ctx.get_reg(binding);
                            ctx.push(Instruction::Move {
                                dest,
                                src: target_reg,
                            });
                        }
                    }
                    DecayedPattern::Fields(fields) => {
                        let mut sorted_fields: Vec<_> = fields.iter().collect();
                        sorted_fields.sort_by_key(|(k, _)| *k);
                        for (field_name, val) in sorted_fields {
                            if let PatternValue::Expr(expr) = val {
                                let idx_reg = ctx.alloc_reg();
                                ctx.push(Instruction::LoadString {
                                    dest: idx_reg,
                                    value: field_name.clone(),
                                });
                                let field_reg = ctx.alloc_reg();
                                ctx.push(Instruction::IndexAccess {
                                    dest: field_reg,
                                    target: target_reg,
                                    index: idx_reg,
                                });
                                let val_reg = lower_expression(ctx, expr);
                                let cmp_reg = ctx.alloc_reg();
                                ctx.push(Instruction::BinaryOp {
                                    dest: cmp_reg,
                                    op: BinaryOperator::Eq,
                                    left: field_reg,
                                    right: val_reg,
                                });
                                mismatch_jumps.push(ctx.instructions.len());
                                ctx.push(Instruction::JumpIfNot {
                                    cond: cmp_reg,
                                    target: 0,
                                });
                            }
                        }
                    }
                }

                if let Some(ref guard_expr) = guard {
                    let guard_reg = lower_expression(ctx, guard_expr);
                    mismatch_jumps.push(ctx.instructions.len());
                    ctx.push(Instruction::JumpIfNot {
                        cond: guard_reg,
                        target: 0,
                    });
                }

                for s in body {
                    lower_spanned(ctx, s);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some((pattern, guard, body)) = decayed_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut decayed_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *decayed_target = Some(start);
                }

                match pattern {
                    DecayedPattern::Binding(binding) => {
                        if !binding.is_empty() {
                            let dest = ctx.get_reg(binding);
                            ctx.push(Instruction::Move {
                                dest,
                                src: target_reg,
                            });
                        }
                    }
                    DecayedPattern::Fields(fields) => {
                        let mut sorted_fields: Vec<_> = fields.iter().collect();
                        sorted_fields.sort_by_key(|(k, _)| *k);
                        for (field_name, val) in sorted_fields {
                            if let PatternValue::Expr(expr) = val {
                                let idx_reg = ctx.alloc_reg();
                                ctx.push(Instruction::LoadString {
                                    dest: idx_reg,
                                    value: field_name.clone(),
                                });
                                let field_reg = ctx.alloc_reg();
                                ctx.push(Instruction::IndexAccess {
                                    dest: field_reg,
                                    target: target_reg,
                                    index: idx_reg,
                                });
                                let val_reg = lower_expression(ctx, expr);
                                let cmp_reg = ctx.alloc_reg();
                                ctx.push(Instruction::BinaryOp {
                                    dest: cmp_reg,
                                    op: BinaryOperator::Eq,
                                    left: field_reg,
                                    right: val_reg,
                                });
                                mismatch_jumps.push(ctx.instructions.len());
                                ctx.push(Instruction::JumpIfNot {
                                    cond: cmp_reg,
                                    target: 0,
                                });
                            }
                        }
                    }
                }

                if let Some(ref guard_expr) = guard {
                    let guard_reg = lower_expression(ctx, guard_expr);
                    mismatch_jumps.push(ctx.instructions.len());
                    ctx.push(Instruction::JumpIfNot {
                        cond: guard_reg,
                        target: 0,
                    });
                }

                for s in body {
                    lower_spanned(ctx, s);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some((pattern, guard, body)) = pending_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut pending_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *pending_target = Some(start);
                }

                match pattern {
                    DecayedPattern::Binding(binding) => {
                        if !binding.is_empty() {
                            let dest = ctx.get_reg(binding);
                            ctx.push(Instruction::Move {
                                dest,
                                src: target_reg,
                            });
                        }
                    }
                    DecayedPattern::Fields(fields) => {
                        let mut sorted_fields: Vec<_> = fields.iter().collect();
                        sorted_fields.sort_by_key(|(k, _)| *k);
                        for (field_name, val) in sorted_fields {
                            if let PatternValue::Expr(expr) = val {
                                let field_reg = ctx.alloc_reg();
                                ctx.push(Instruction::FieldAccess {
                                    dest: field_reg,
                                    target: target_reg,
                                    field: field_name.clone(),
                                });
                                let val_reg = lower_expression(ctx, expr);
                                let cmp_reg = ctx.alloc_reg();
                                ctx.push(Instruction::BinaryOp {
                                    dest: cmp_reg,
                                    op: BinaryOperator::Eq,
                                    left: field_reg,
                                    right: val_reg,
                                });
                                mismatch_jumps.push(ctx.instructions.len());
                                ctx.push(Instruction::JumpIfNot {
                                    cond: cmp_reg,
                                    target: 0,
                                });
                            }
                        }
                    }
                }

                if let Some(ref guard_expr) = guard {
                    let guard_reg = lower_expression(ctx, guard_expr);
                    mismatch_jumps.push(ctx.instructions.len());
                    ctx.push(Instruction::JumpIfNot {
                        cond: guard_reg,
                        target: 0,
                    });
                }

                for s in body {
                    lower_spanned(ctx, s);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            if let Some((guard, body)) = consumed_branch {
                let start = ctx.instructions.len();
                if let Instruction::MatchEntropy {
                    ref mut consumed_target,
                    ..
                } = ctx.instructions[match_idx]
                {
                    *consumed_target = Some(start);
                }

                if let Some(ref guard_expr) = guard {
                    let guard_reg = lower_expression(ctx, guard_expr);
                    mismatch_jumps.push(ctx.instructions.len());
                    ctx.push(Instruction::JumpIfNot {
                        cond: guard_reg,
                        target: 0,
                    });
                }

                for s in body {
                    lower_spanned(ctx, s);
                }
                branch_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });
            }

            let end_idx = ctx.instructions.len();
            for jump_idx in branch_jumps {
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_idx]
                {
                    *target = end_idx;
                }
            }
            for jump_idx in mismatch_jumps {
                if let Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::EnumDecl { .. } => {}
        Statement::DestructureAssignment { fields, expr, .. } => {
            let src = lower_expression(ctx, expr);
            for (source_field, target_var) in fields {
                let dest = ctx.get_reg(target_var);
                ctx.push(Instruction::FieldAccess {
                    dest,
                    target: src,
                    field: source_field.clone(),
                });
            }
        }
        Statement::Using {
            binding,
            resource,
            body,
        } => {
            let src = lower_expression(ctx, resource);
            let dest = ctx.get_reg(binding);
            ctx.push(Instruction::Move { dest, src });

            for s in body {
                lower_spanned(ctx, s);
            }

            if let Some(src_type) = ctx.reg_types.get(&src.0).cloned() {
                if let Some(spec) = ctx.auto_drop_specs.get(&src_type).cloned() {
                    ctx.push(Instruction::AutoDrop { target: dest, spec });
                }
            } else if let Some(spec) =
                ctx.auto_drop_specs.get(&format!("_reg_{}", src.0)).cloned()
            {
                ctx.push(Instruction::AutoDrop { target: dest, spec });
            } else {
                for (type_name, spec) in ctx.auto_drop_specs.clone() {
                    if binding.to_lowercase().contains(&type_name.to_lowercase())
                        || type_name.to_lowercase().contains(&binding.to_lowercase())
                    {
                        ctx.push(Instruction::AutoDrop {
                            target: dest,
                            spec: spec.clone(),
                        });
                        break;
                    }
                }
            }
            ctx.push(Instruction::Consume { src: dest });
        }
        Statement::Assignment {
            target,
            expr,
            lifetime,
            ..
        } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.get_reg(target);
            ctx.push(Instruction::Move { dest, src });
            if let Some(causm_core::LifetimeAnnotation::Decayed(ms)) = lifetime {
                ctx.push(Instruction::Lease {
                    target_reg: dest,
                    source_reg: dest,
                    duration_ms: *ms,
                });
            }

            match expr {
                Expression::Identifier(_) => {
                    ctx.push(Instruction::Consume { src });
                }
                Expression::IndexAccess { target, index } => {
                    if let Expression::Identifier(name) = &**target {
                        let target_reg = ctx.get_reg(name);
                        let index_reg = lower_expression(ctx, index);
                        ctx.push(Instruction::ConsumeFieldDynamic {
                            target: target_reg,
                            index: index_reg,
                        });
                    }
                }
                Expression::FieldAccess { target, field } => {
                    if let Expression::Identifier(name) = &**target {
                        let target_reg = ctx.get_reg(name);
                        ctx.push(Instruction::ConsumeField {
                            src: target_reg,
                            field: field.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
        Statement::Print(args) => {
            let src = if args.is_empty() {
                let dest = ctx.alloc_reg();
                ctx.push(Instruction::LoadString {
                    dest,
                    value: String::new(),
                });
                dest
            } else {
                let mut acc = lower_expression(ctx, &args[0]);
                for arg in &args[1..] {
                    // insert a space separator
                    let space = ctx.alloc_reg();
                    ctx.push(Instruction::LoadString {
                        dest: space,
                        value: " ".to_string(),
                    });
                    let with_space = ctx.alloc_reg();
                    ctx.push(Instruction::BinaryOp {
                        dest: with_space,
                        op: causm_core::BinaryOperator::Add,
                        left: acc,
                        right: space,
                    });
                    let rhs = lower_expression(ctx, arg);
                    let joined = ctx.alloc_reg();
                    ctx.push(Instruction::BinaryOp {
                        dest: joined,
                        op: causm_core::BinaryOperator::Add,
                        left: with_space,
                        right: rhs,
                    });
                    acc = joined;
                }
                acc
            };
            ctx.push(Instruction::Print { src });
        }
        Statement::Debug(expr) => {
            let src = lower_expression(ctx, expr);
            ctx.push(Instruction::Debug { src });
        }
        Statement::Isolate(block) => {
            let name = block.name.clone().unwrap_or_else(|| "<anon>".to_string());
            ctx.push(Instruction::Isolate {
                name,
                manifest: block.manifest.clone(),
            });
            for s in &block.body {
                lower_spanned(ctx, s);
            }
            // Drop elaboration for auto_drop structs
            let symbols_snapshot: Vec<(String, Reg)> =
                ctx.symbols.iter().map(|(k, v)| (k.clone(), *v)).collect();
            for (var_name, reg) in &symbols_snapshot {
                if let Some(t) = ctx.reg_types.get(&reg.0).cloned() {
                    if let Some(spec) = ctx.auto_drop_specs.get(&t).cloned() {
                        ctx.push(Instruction::AutoDrop { target: *reg, spec });
                        continue;
                    }
                }
                for (type_name, spec) in ctx.auto_drop_specs.clone() {
                    if var_name.to_lowercase().contains(&type_name.to_lowercase())
                        || type_name
                            .to_lowercase()
                            .contains(&var_name.to_lowercase())
                    {
                        ctx.push(Instruction::AutoDrop {
                            target: *reg,
                            spec: spec.clone(),
                        });
                        break;
                    }
                }
            }
            ctx.push(Instruction::EndIsolate);
        }
        Statement::Capability(cap) => {
            ctx.push(Instruction::Capability { cap: cap.clone() });
        }
        Statement::For {
            item_name,
            mode,
            source,
            body,
            pacing_ms,
            max_ms,
        } => {
            let source_reg = ctx.get_reg(source);
            let start_pc = ctx.instructions.len();
            let cond_reg = ctx.alloc_reg();
            let item_reg = ctx.get_reg(item_name);

            ctx.push(Instruction::For {
                dest_cond: cond_reg,
                item_reg,
                item_name: item_name.clone(),
                mode: mode.clone(),
                source: source_reg,
                pacing_ms: *pacing_ms,
                max_ms: *max_ms,
            });

            let jump_to_end_idx = ctx.instructions.len();
            ctx.push(Instruction::JumpIfNot {
                cond: cond_reg,
                target: 0,
            });

            for s in body {
                lower_spanned(ctx, s);
            }

            ctx.push(Instruction::EndFor);
            ctx.push(Instruction::Jump { target: start_pc });

            let end_pc = ctx.instructions.len();
            if let Instruction::JumpIfNot { ref mut target, .. } =
                ctx.instructions[jump_to_end_idx]
            {
                *target = end_pc;
            }
        }
        Statement::SplitMap {
            item_name,
            mode,
            source,
            body,
            reconcile,
        } => {
            let source_reg = ctx.get_reg(source);
            let item_reg = ctx.get_reg(item_name);
            let _ = ctx.get_reg("splitmap_results");
            ctx.push(Instruction::SplitMap {
                item_reg,
                item_name: item_name.clone(),
                mode: mode.clone(),
                source: source_reg,
                reconcile: reconcile.clone(),
            });
            let _ = ctx.get_reg(item_name);
            for s in body {
                lower_spanned(ctx, s);
            }
            ctx.push(Instruction::EndSplitMap);
        }
        Statement::Split { parent, branches } => {
            ctx.push(Instruction::Split {
                parent: parent.clone(),
                branches: branches.clone(),
            });
        }
        Statement::Merge {
            branches,
            target,
            resolutions,
            ..
        } => {
            ctx.push(Instruction::Merge {
                branches: branches.clone(),
                target: target.clone(),
                resolution: resolutions.clone(),
            });
        }
        Statement::Anchor(name) => {
            ctx.push(Instruction::Anchor { name: name.clone() });
        }
        Statement::Rewind(name) => {
            ctx.push(Instruction::Rewind {
                target: "self".to_string(),
                anchor: name.clone(),
            });
        }
        Statement::ForeignBlock {
            lib_name,
            abi,
            routines,
        } => {
            for r in routines {
                if let Statement::RoutineDef {
                    name,
                    params,
                    return_type,
                    taking_ms,
                    ..
                } = &r.stmt
                {
                    let bare_symbol =
                        name.rsplit('.').next().unwrap_or(name).to_string();
                    let routine = IrRoutine {
                        params: params
                            .iter()
                            .map(|p| {
                                (
                                    p.mode.clone(),
                                    p.name.clone(),
                                    p.typ
                                        .as_ref()
                                        .map(causm_core::types::Type::from_typename)
                                        .unwrap_or(causm_core::types::Type::Unknown),
                                )
                            })
                            .collect(),
                        return_type: return_type
                            .as_ref()
                            .map(causm_core::types::Type::from_typename)
                            .unwrap_or(causm_core::types::Type::Unknown),
                        taking_ms: *taking_ms,
                        foreign_binding: Some(causm_ir::ForeignBinding {
                            lib_name: lib_name.clone(),
                            abi: abi.clone(),
                            symbol: bare_symbol,
                        }),
                        instructions: Vec::new(),
                        spans: Vec::new(),
                    };
                    ctx.routines.insert(name.clone(), routine);
                } else {
                    lower_spanned(ctx, r);
                }
            }
        }
        Statement::Entangle { variables } => {
            let regs = variables.iter().map(|v| ctx.get_reg(v)).collect();
            ctx.push(Instruction::Entangle { regs });
        }
        Statement::Await(name) => {
            let target = ctx.get_reg(name);
            ctx.push(Instruction::Await { target });
        }
        Statement::Commit(body) => {
            for s in body {
                lower_spanned(ctx, s);
            }
            ctx.push(Instruction::Commit { vars: Vec::new() });
        }
        Statement::Slice { milliseconds } => {
            ctx.push(Instruction::Slice { ms: *milliseconds });
        }
        Statement::Break => {
            ctx.push(Instruction::Break);
        }
        Statement::LoopTick { body } => {
            ctx.push(Instruction::LoopTick);
            for s in body {
                lower_spanned(ctx, s);
            }
            ctx.push(Instruction::EndLoopTick);
        }
        Statement::Lease {
            binding,
            source,
            duration_ms,
            body,
            reconcile: _,
        } => {
            let source_reg = ctx.get_reg(source);
            let old_symbol = ctx.symbols.get(binding).cloned();
            let target_reg = ctx.get_reg(binding);

            ctx.push(Instruction::Lease {
                target_reg,
                source_reg,
                duration_ms: *duration_ms,
            });

            for s in body {
                lower_spanned(ctx, s);
            }

            ctx.push(Instruction::EndLease {
                source_reg,
                duration_ms: *duration_ms,
            });

            if let Some(old) = old_symbol {
                ctx.symbols.insert(binding.clone(), old);
            } else {
                ctx.symbols.remove(binding);
            }
        }
        Statement::Loop { max_ms, body } => {
            let start_pc = ctx.instructions.len();
            ctx.push(Instruction::Loop { max_ms: *max_ms });
            for s in body {
                lower_spanned(ctx, s);
            }
            ctx.push(Instruction::EndLoop { max_ms: *max_ms });
            ctx.push(Instruction::Jump { target: start_pc });
        }
        Statement::While {
            condition,
            is_valid_check,
            max_ms,
            body,
        } => {
            let while_limit = *max_ms;
            ctx.push(Instruction::While {
                max_ms: while_limit,
            });
            let start_pc = ctx.instructions.len();

            if *is_valid_check {
                let cond_reg = lower_expression(ctx, condition);
                let match_entropy_idx = ctx.instructions.len();
                ctx.push(Instruction::MatchEntropy {
                    target: cond_reg,
                    valid_target: None,
                    decayed_target: None,
                    pending_target: None,
                    consumed_target: None,
                });

                for s in body {
                    lower_spanned(ctx, s);
                }

                ctx.push(Instruction::Jump { target: start_pc });

                let end_while_idx = ctx.instructions.len();
                ctx.push(Instruction::EndWhile {
                    max_ms: while_limit,
                });

                if let Instruction::MatchEntropy {
                    ref mut valid_target,
                    ref mut decayed_target,
                    ref mut pending_target,
                    ref mut consumed_target,
                    ..
                } = ctx.instructions[match_entropy_idx]
                {
                    *valid_target = Some(match_entropy_idx + 1);
                    *decayed_target = Some(end_while_idx);
                    *pending_target = Some(end_while_idx);
                    *consumed_target = Some(end_while_idx);
                }
            } else {
                let cond_reg = lower_expression(ctx, condition);
                let jump_to_end_idx = ctx.instructions.len();
                ctx.push(Instruction::JumpIfNot {
                    cond: cond_reg,
                    target: 0,
                });

                for s in body {
                    lower_spanned(ctx, s);
                }

                ctx.push(Instruction::Jump { target: start_pc });

                let end_while_idx = ctx.instructions.len();
                ctx.push(Instruction::EndWhile {
                    max_ms: while_limit,
                });

                if let Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_to_end_idx]
                {
                    *target = end_while_idx;
                }
            }
        }
        Statement::ForStep {
            item_name,
            source,
            step_ms,
            body,
        } => {
            let source_reg = match source {
                Expression::Identifier(ref name) => ctx.get_reg(name),
                _ => lower_expression(ctx, source),
            };
            let start_pc = ctx.instructions.len();
            let cond_reg = ctx.alloc_reg();
            let item_reg = ctx.get_reg(item_name);

            ctx.push(Instruction::ForStep {
                dest_cond: cond_reg,
                item_reg,
                item_name: item_name.clone(),
                source: source_reg,
                step_ms: *step_ms,
            });

            let jump_to_end_idx = ctx.instructions.len();
            ctx.push(Instruction::JumpIfNot {
                cond: cond_reg,
                target: 0,
            });

            for s in body {
                lower_spanned(ctx, s);
            }

            ctx.push(Instruction::EndForStep);
            ctx.push(Instruction::Jump { target: start_pc });

            let end_pc = ctx.instructions.len();
            if let Instruction::JumpIfNot { ref mut target, .. } =
                ctx.instructions[jump_to_end_idx]
            {
                *target = end_pc;
            }
        }
        Statement::If {
            binding,
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_reg;
            let mut old_symbol = None;

            if let Some(binding_name) = binding {
                if let Expression::TypeAssertion { target, cast_type } = condition {
                    let target_reg = lower_expression(ctx, target);
                    let dest_reg = ctx.alloc_reg();
                    let success_reg = ctx.alloc_reg();

                    let type_name_str = match cast_type {
                        causm_core::TypeName::Custom(ref s) => s.clone(),
                        causm_core::TypeName::Builtin(b) => format!("{:?}", b),
                        _ => format!("{:?}", cast_type),
                    };

                    ctx.push(Instruction::TryTypeAssert {
                        dest: dest_reg,
                        src: target_reg,
                        type_name: type_name_str,
                        success: success_reg,
                    });

                    let old = ctx.symbols.insert(binding_name.clone(), dest_reg);
                    old_symbol = Some((binding_name.clone(), old));
                    cond_reg = success_reg;
                } else {
                    panic!("Expected TypeAssertion expression inside If statement with binding");
                }
            } else {
                cond_reg = lower_expression(ctx, condition);
            }

            let jump_to_else_idx = ctx.instructions.len();
            ctx.push(Instruction::JumpIfNot {
                cond: cond_reg,
                target: 0,
            });

            for s in then_branch {
                lower_spanned(ctx, s);
            }

            if let Some((binding_name, old)) = old_symbol {
                if let Some(o) = old {
                    ctx.symbols.insert(binding_name, o);
                } else {
                    ctx.symbols.remove(&binding_name);
                }
            }

            if let Some(eb) = else_branch {
                let jump_to_end_idx = ctx.instructions.len();
                ctx.push(Instruction::Jump { target: 0 });

                let else_start_idx = ctx.instructions.len();
                if let Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_to_else_idx]
                {
                    *target = else_start_idx;
                }

                for s in eb {
                    lower_spanned(ctx, s);
                }

                let end_idx = ctx.instructions.len();
                if let Instruction::Jump { ref mut target, .. } =
                    ctx.instructions[jump_to_end_idx]
                {
                    *target = end_idx;
                }
            } else {
                let end_idx = ctx.instructions.len();
                if let Instruction::JumpIfNot { ref mut target, .. } =
                    ctx.instructions[jump_to_else_idx]
                {
                    *target = end_idx;
                }
            }
        }
        Statement::Match { target, arms } => {
            let target_reg = lower_expression(ctx, target);
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
                    ctx.push(Instruction::JumpIfNot {
                        cond: guard_reg,
                        target: 0,
                    });
                }

                for s in &arm.body {
                    lower_spanned(ctx, s);
                }

                for (name, old) in bound_symbols.into_iter().rev() {
                    if let Some(o) = old {
                        ctx.symbols.insert(name, o);
                    } else {
                        ctx.symbols.remove(&name);
                    }
                }

                exit_jumps.push(ctx.instructions.len());
                ctx.push(Instruction::Jump { target: 0 });

                let next_arm_target = ctx.instructions.len();
                for fail_idx in fail_jumps {
                    if let Instruction::JumpIfNot { ref mut target, .. } =
                        ctx.instructions[fail_idx]
                    {
                        *target = next_arm_target;
                    }
                }
            }

            let end_target = ctx.instructions.len();
            for exit_idx in exit_jumps {
                if let Instruction::Jump { ref mut target } =
                    ctx.instructions[exit_idx]
                {
                    *target = end_target;
                }
            }
        }
        Statement::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
            ..
        } => {
            let target_reg = lower_expression(ctx, expr);
            let mut fail_jumps = Vec::new();
            let mut bound_symbols = Vec::new();

            lower_pattern_test(
                ctx,
                target_reg,
                pattern,
                &mut fail_jumps,
                &mut bound_symbols,
            );

            for s in then_branch {
                lower_spanned(ctx, s);
            }

            for (name, old) in bound_symbols.into_iter().rev() {
                if let Some(o) = old {
                    ctx.symbols.insert(name, o);
                } else {
                    ctx.symbols.remove(&name);
                }
            }

            if let Some(eb) = else_branch {
                let jump_to_end_idx = ctx.instructions.len();
                ctx.push(Instruction::Jump { target: 0 });

                let else_start_idx = ctx.instructions.len();
                for fail_idx in fail_jumps {
                    if let Instruction::JumpIfNot { ref mut target, .. } =
                        ctx.instructions[fail_idx]
                    {
                        *target = else_start_idx;
                    }
                }

                for s in eb {
                    lower_spanned(ctx, s);
                }

                let end_pc = ctx.instructions.len();
                if let Instruction::Jump { ref mut target } =
                    ctx.instructions[jump_to_end_idx]
                {
                    *target = end_pc;
                }
            } else {
                let end_pc = ctx.instructions.len();
                for fail_idx in fail_jumps {
                    if let Instruction::JumpIfNot { ref mut target, .. } =
                        ctx.instructions[fail_idx]
                    {
                        *target = end_pc;
                    }
                }
            }
        }
        Statement::FieldUpdate {
            target,
            field,
            value,
        } => match target {
            Expression::Identifier(name) => {
                let target_reg = ctx.get_reg(name);
                let src_reg = lower_expression(ctx, value);
                if field.is_empty() {
                    ctx.push(Instruction::Move {
                        dest: target_reg,
                        src: src_reg,
                    });
                } else {
                    ctx.push(Instruction::FieldUpdate {
                        target: target_reg,
                        field: field.clone(),
                        src: src_reg,
                    });
                }
            }
            Expression::IndexAccess {
                target: inner_target,
                index,
            } => {
                let graph_reg = lower_expression(ctx, inner_target);
                let index_reg = lower_expression(ctx, index);
                let src_reg = lower_expression(ctx, value);
                ctx.push(Instruction::IndexFieldUpdate {
                    target: graph_reg,
                    index: index_reg,
                    field: field.clone(),
                    src: src_reg,
                });
            }
            _ => {
                let target_reg = lower_expression(ctx, target);
                let src_reg = lower_expression(ctx, value);
                ctx.push(Instruction::FieldUpdate {
                    target: target_reg,
                    field: field.clone(),
                    src: src_reg,
                });
            }
        },
        Statement::Expression(expr) => {
            lower_expression(ctx, expr);
        }
        Statement::AssertTime {
            operator, limit_ms, ..
        } => {
            ctx.push(Instruction::AssertTime {
                op: *operator,
                limit_ms: *limit_ms,
            });
        }
        Statement::TypeDecl {
            name,
            extends,
            fields,
            decay_after_ms,
            auto_drop,
            scoped_branch: _,
        } => {
            let mut resolved_fields = HashMap::new();
            if let Some(ref base_name) = extends {
                ctx.struct_extends.insert(name.clone(), base_name.clone());
                if let Some(base_fields) = ctx.type_decls.get(base_name) {
                    resolved_fields = base_fields.clone();
                }
            }
            for (k, v) in fields {
                resolved_fields.insert(k.clone(), v.clone());
            }
            if let Some(limit) = decay_after_ms {
                ctx.type_decay_limits.insert(name.clone(), *limit);
            }
            if let Some(ref spec) = auto_drop {
                ctx.auto_drop_specs.insert(name.clone(), spec.clone());
            }
            ctx.type_decls.insert(name.clone(), resolved_fields);
        }
        Statement::InterfaceDecl {
            name,
            extends,
            methods,
        } => {
            let mut resolved_methods = Vec::new();
            for base in extends {
                if let Some(base_methods) = ctx.interfaces.get(base) {
                    resolved_methods.extend(base_methods.clone());
                }
            }
            resolved_methods.extend(methods.clone());
            ctx.interfaces.insert(name.clone(), resolved_methods);
        }
        Statement::DecayHandler { type_name, body } => {
            let mut sub_ctx = LoweringContext::new();
            sub_ctx.symbols = ctx.symbols.clone();
            sub_ctx.next_reg = ctx.next_reg;
            sub_ctx.type_decls = ctx.type_decls.clone();
            sub_ctx.type_decay_limits = ctx.type_decay_limits.clone();
            sub_ctx.struct_extends = ctx.struct_extends.clone();
            for s in body {
                lower_spanned(&mut sub_ctx, s);
            }
            ctx.decay_handlers
                .insert(type_name.clone(), sub_ctx.instructions);
        }
        Statement::DirectiveBlock { directives, body } => {
            use causm_core::BlockDirective;
            let mut new_entropy_mode = None;
            for dir in directives {
                match dir {
                    BlockDirective::Chaos => {
                        new_entropy_mode = Some(causm_core::EntropyMode::Chaos)
                    }
                    BlockDirective::Deterministic => {
                        new_entropy_mode =
                            Some(causm_core::EntropyMode::Deterministic)
                    }
                    _ => {}
                }
            }

            if let Some(mode) = new_entropy_mode {
                ctx.push(causm_ir::Instruction::SetEntropyMode { mode });
                ctx.entropy_modes.push(mode);
            }

            for s in body {
                lower_spanned(ctx, s);
            }

            if new_entropy_mode.is_some() {
                ctx.entropy_modes.pop();
                let prev_mode = ctx
                    .entropy_modes
                    .last()
                    .copied()
                    .unwrap_or(causm_core::EntropyMode::Deterministic);
                ctx.push(causm_ir::Instruction::SetEntropyMode { mode: prev_mode });
            }
        }
        Statement::StateDecl { target, expr, .. } => {
            let src = lower_expression(ctx, expr);
            let dest = ctx.get_reg(target);
            ctx.push(causm_ir::Instruction::Move { dest, src });
        }
        Statement::PolicyStmt { target, policy } => {
            ctx.push(causm_ir::Instruction::SetSaturationPolicy {
                target: *target,
                policy: *policy,
            });
        }
        Statement::LoopOn { target, body } => {
            let start_pc = ctx.instructions.len();
            let _target_reg = lower_expression(ctx, target);
            for s in body {
                lower_spanned(ctx, s);
            }
            ctx.push(causm_ir::Instruction::ResetBaseWatermark);
            ctx.push(causm_ir::Instruction::Jump { target: start_pc });
        }
        _ => {}
    }
}
