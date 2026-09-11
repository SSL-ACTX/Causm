//! Lowers causm_core::Program (AST) into canonical causm_frontend::hir::HirProgram
//!
//! Desugars:
//! 1. `Expression::FString` -> Concatenation chain of `BinaryOp::Add` with `ToStr`
//! 2. `Expression::Turbofish` -> Unwrapped inner expression
//! 3. `Statement::Using` -> Let binding + inner body + `AutoDrop` + `Consume`
//! 4. `Statement::MacroDef` -> Stripped (macros are already expanded)

use super::def::*;
use causm_core::{
    BinaryOperator, Expression, FStringPart, Program, SpannedStatement, Statement,
};
use std::collections::HashMap;

pub fn lower_ast_to_hir(program: &Program) -> HirProgram {
    let mut timelines = Vec::with_capacity(program.timelines.len());
    for tl in &program.timelines {
        timelines.push(HirTimelineBlock {
            time: tl.time.clone(),
            no_z3: tl.no_z3,
            entropy_mode: tl.entropy_mode,
            statements: lower_spanned_stmts(&tl.statements),
        });
    }
    HirProgram { timelines }
}

pub fn lower_spanned_stmts(stmts: &[SpannedStatement]) -> Vec<HirSpannedStatement> {
    let mut out = Vec::with_capacity(stmts.len());
    for s in stmts {
        lower_spanned_stmt(s, &mut out);
    }
    out
}

fn lower_spanned_stmt(spanned: &SpannedStatement, out: &mut Vec<HirSpannedStatement>) {
    let span = spanned.span.clone();
    let attrs = spanned.attributes.clone();

    match &spanned.stmt {
        Statement::MacroDef { .. } => {
            // Eliminated at HIR stage
        }
        Statement::Using {
            binding,
            resource,
            body,
        } => {
            // Desugar `using r = expr { ... }` into:
            // 1. `let r = expr;`
            // 2. `body`
            // 3. `autodrop r;`
            // 4. `consume r;`
            let res_expr = lower_expr(resource);
            let mut assign = HirSpannedStatement::new(
                HirStatement::Assignment {
                    target: binding.clone(),
                    mutable: false,
                    var_type: None,
                    lifetime: None,
                    expr: res_expr,
                },
                span.clone(),
            );
            assign.attributes = attrs.clone();
            out.push(assign);

            for inner in body {
                lower_spanned_stmt(inner, out);
            }

            out.push(HirSpannedStatement::new(
                HirStatement::AutoDrop {
                    target: binding.clone(),
                },
                span.clone(),
            ));
            out.push(HirSpannedStatement::new(
                HirStatement::Consume {
                    target: binding.clone(),
                },
                span,
            ));
        }
        Statement::Assignment {
            target,
            mutable,
            var_type,
            lifetime,
            expr,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Assignment {
                    target: target.clone(),
                    mutable: *mutable,
                    var_type: var_type.clone(),
                    lifetime: lifetime.clone(),
                    expr: lower_expr(expr),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::DestructureAssignment {
            fields,
            mutable,
            expr,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::DestructureAssignment {
                    fields: fields.clone(),
                    mutable: *mutable,
                    expr: lower_expr(expr),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Expression(e) => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Expression(lower_expr(e)),
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Return(opt_e) => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Return(opt_e.as_ref().map(lower_expr)),
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Yield(opt_e) => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Yield(opt_e.as_ref().map(lower_expr)),
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Print(exprs) => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Print(exprs.iter().map(lower_expr).collect()),
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Debug(e) => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Debug(lower_expr(e)),
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::AssertTime {
            operator,
            limit_ms,
            fallback,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::AssertTime {
                    operator: *operator,
                    limit_ms: *limit_ms,
                    fallback: fallback.as_ref().map(|b| lower_spanned_stmts(b)),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Break => {
            let mut s = HirSpannedStatement::new(HirStatement::Break, span);
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Collapse => {
            let mut s = HirSpannedStatement::new(HirStatement::Collapse, span);
            s.attributes = attrs;
            out.push(s);
        }
        Statement::If {
            binding,
            condition,
            then_branch,
            else_branch,
            reconcile,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::If {
                    binding: binding.clone(),
                    condition: lower_expr(condition),
                    then_branch: lower_spanned_stmts(then_branch),
                    else_branch: else_branch.as_ref().map(|b| lower_spanned_stmts(b)),
                    reconcile: reconcile.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
            reconcile,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::IfLet {
                    pattern: pattern.clone(),
                    expr: lower_expr(expr),
                    then_branch: lower_spanned_stmts(then_branch),
                    else_branch: else_branch.as_ref().map(|b| lower_spanned_stmts(b)),
                    reconcile: reconcile.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Match { target, arms } => {
            let hir_arms = arms
                .iter()
                .map(|a| HirMatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.as_ref().map(lower_expr),
                    body: lower_spanned_stmts(&a.body),
                })
                .collect();
            let mut s = HirSpannedStatement::new(
                HirStatement::Match {
                    target: lower_expr(target),
                    arms: hir_arms,
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::MatchEntropy {
            target,
            valid_branch,
            decayed_branch,
            pending_branch,
            consumed_branch,
        } => {
            let map_branch = |b: &Option<(causm_core::DecayedPattern, Option<Expression>, Vec<SpannedStatement>)>| {
                b.as_ref().map(|(pat, opt_e, stmts)| {
                    (pat.clone(), opt_e.as_ref().map(lower_expr), lower_spanned_stmts(stmts))
                })
            };
            let map_consumed = |b: &Option<(Option<Expression>, Vec<SpannedStatement>)>| {
                b.as_ref().map(|(opt_e, stmts)| {
                    (opt_e.as_ref().map(lower_expr), lower_spanned_stmts(stmts))
                })
            };
            let mut s = HirSpannedStatement::new(
                HirStatement::MatchEntropy {
                    target: lower_expr(target),
                    valid_branch: map_branch(valid_branch),
                    decayed_branch: map_branch(decayed_branch),
                    pending_branch: map_branch(pending_branch),
                    consumed_branch: map_consumed(consumed_branch),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::While {
            condition,
            is_valid_check,
            max_ms,
            body,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::While {
                    condition: lower_expr(condition),
                    is_valid_check: *is_valid_check,
                    max_ms: *max_ms,
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Loop { max_ms, body } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Loop {
                    max_ms: *max_ms,
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::LoopTick { body } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::LoopTick {
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::LoopOn { target, body } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::LoopOn {
                    target: lower_expr(target),
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::For {
            item_name,
            mode,
            source,
            body,
            pacing_ms,
            max_ms,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::For {
                    item_name: item_name.clone(),
                    mode: mode.clone(),
                    source: source.clone(),
                    body: lower_spanned_stmts(body),
                    pacing_ms: *pacing_ms,
                    max_ms: *max_ms,
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::ForStep {
            item_name,
            source,
            step_ms,
            body,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::ForStep {
                    item_name: item_name.clone(),
                    source: lower_expr(source),
                    step_ms: *step_ms,
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::RoutineDef {
            name,
            params,
            return_type,
            taking_ms,
            state_constraint,
            required_capabilities,
            body,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::RoutineDef {
                    name: name.clone(),
                    params: params.clone(),
                    return_type: return_type.clone(),
                    taking_ms: *taking_ms,
                    state_constraint: state_constraint.clone(),
                    required_capabilities: required_capabilities.clone(),
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::TypeDecl {
            name,
            extends,
            fields,
            decay_after_ms,
            auto_drop,
            scoped_branch,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::TypeDecl {
                    name: name.clone(),
                    extends: extends.clone(),
                    fields: fields.clone(),
                    decay_after_ms: *decay_after_ms,
                    auto_drop: auto_drop.clone(),
                    scoped_branch: scoped_branch.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::EnumDecl { name, variants } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::EnumDecl {
                    name: name.clone(),
                    variants: variants.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::InterfaceDecl {
            name,
            extends,
            methods,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::InterfaceDecl {
                    name: name.clone(),
                    extends: extends.clone(),
                    methods: methods.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::DecayHandler { type_name, body } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::DecayHandler {
                    type_name: type_name.clone(),
                    body: lower_spanned_stmts(body),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Commit(inner) => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Commit(lower_spanned_stmts(inner)),
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Anchor(a) => {
            let mut s = HirSpannedStatement::new(HirStatement::Anchor(a.clone()), span);
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Rewind(r) => {
            let mut s = HirSpannedStatement::new(HirStatement::Rewind(r.clone()), span);
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Lease {
            binding,
            source,
            duration_ms,
            body,
            reconcile,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Lease {
                    binding: binding.clone(),
                    source: source.clone(),
                    duration_ms: *duration_ms,
                    body: lower_spanned_stmts(body),
                    reconcile: reconcile.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Select {
            max_ms,
            cases,
            timeout,
            reconcile,
        } => {
            let hir_cases = cases
                .iter()
                .map(|c| HirSelectCase {
                    binding: c.binding.clone(),
                    source: lower_expr(&c.source),
                    body: lower_spanned_stmts(&c.body),
                })
                .collect();
            let hir_timeout = timeout.as_ref().map(|b| lower_spanned_stmts(b));

            let mut s = HirSpannedStatement::new(
                HirStatement::Select {
                    max_ms: *max_ms,
                    cases: hir_cases,
                    timeout: hir_timeout,
                    reconcile: reconcile.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Entangle { variables } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Entangle {
                    variables: variables.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Speculate {
                    max_ms: *max_ms,
                    body: lower_spanned_stmts(body),
                    fallback: fallback.as_ref().map(|b| lower_spanned_stmts(b)),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::SpeculationMode(m) => {
            let mut s = HirSpannedStatement::new(HirStatement::SpeculationMode(*m), span);
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Slice { milliseconds } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Slice {
                    milliseconds: *milliseconds,
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Split { parent, branches } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Split {
                    parent: parent.clone(),
                    branches: branches.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Merge {
            branches,
            target,
            resolutions,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Merge {
                    branches: branches.clone(),
                    target: target.clone(),
                    resolutions: resolutions.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::SplitMap {
            item_name,
            mode,
            source,
            body,
            reconcile,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::SplitMap {
                    item_name: item_name.clone(),
                    mode: mode.clone(),
                    source: source.clone(),
                    body: lower_spanned_stmts(body),
                    reconcile: reconcile.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Send {
            value_id,
            target_branch,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::Send {
                    value_id: value_id.clone(),
                    target_branch: target_branch.clone(),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::FieldUpdate {
            target,
            field,
            value,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::FieldUpdate {
                    target: lower_expr(target),
                    field: field.clone(),
                    value: lower_expr(value),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::StateDecl {
            target,
            var_type,
            expr,
        } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::StateDecl {
                    target: target.clone(),
                    var_type: var_type.clone(),
                    expr: lower_expr(expr),
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::PolicyStmt { target, policy } => {
            let mut s = HirSpannedStatement::new(
                HirStatement::PolicyStmt {
                    target: *target,
                    policy: *policy,
                },
                span,
            );
            s.attributes = attrs;
            out.push(s);
        }
        Statement::Isolate(iso) => {
            for inner in &iso.body {
                lower_spanned_stmt(inner, out);
            }
        }
        Statement::Import { .. }
        | Statement::FromImport { .. }
        | Statement::ForeignBlock { .. }
        | Statement::RelativisticBlock { .. }
        | Statement::DirectiveBlock { .. }
        | Statement::Capability(_)
        | Statement::Await(_) => {
            // Evaluated/flattened in frontend preprocessing passes
        }
    }
}

pub fn lower_expr(expr: &Expression) -> HirExpression {
    match expr {
        Expression::Integer(v) => HirExpression::Integer(*v),
        Expression::Float(v) => HirExpression::Float(*v),
        Expression::Boolean(v) => HirExpression::Boolean(*v),
        Expression::Literal(s) => HirExpression::Literal(s.clone()),
        Expression::Identifier(s) => HirExpression::Identifier(s.clone()),
        Expression::Null => HirExpression::Null,
        Expression::BinaryOp { left, op, right } => HirExpression::BinaryOp {
            op: *op,
            left: Box::new(lower_expr(left)),
            right: Box::new(lower_expr(right)),
        },
        Expression::UnaryOp { op, expr } => HirExpression::UnaryOp {
            op: *op,
            expr: Box::new(lower_expr(expr)),
        },
        Expression::Call { routine, args, .. } => HirExpression::Call {
            routine: routine.clone(),
            args: args.iter().map(lower_expr).collect(),
        },
        Expression::MethodCall {
            target,
            method,
            args,
            ..
        } => HirExpression::MethodCall {
            target: Box::new(lower_expr(target)),
            method: method.clone(),
            args: args.iter().map(lower_expr).collect(),
        },
        Expression::GenericStaticCall {
            type_name,
            type_args,
            method,
            args,
        } => HirExpression::GenericStaticCall {
            type_name: type_name.clone(),
            type_args: type_args.clone(),
            method: method.clone(),
            args: args.iter().map(lower_expr).collect(),
        },
        Expression::FieldAccess { target, field } => HirExpression::FieldAccess {
            target: Box::new(lower_expr(target)),
            field: field.clone(),
        },
        Expression::IndexAccess { target, index } => HirExpression::IndexAccess {
            target: Box::new(lower_expr(target)),
            index: Box::new(lower_expr(index)),
        },
        Expression::ArrayLiteral(elems) => {
            HirExpression::ArrayLiteral(elems.iter().map(lower_expr).collect())
        }
        Expression::ArrayRepeat { value, count } => HirExpression::ArrayRepeat {
            value: Box::new(lower_expr(value)),
            count: Box::new(lower_expr(count)),
        },
        Expression::ArraySlice {
            target,
            start,
            end,
            inclusive,
        } => HirExpression::ArraySlice {
            target: Box::new(lower_expr(target)),
            start: start.as_ref().map(|e| Box::new(lower_expr(e))),
            end: end.as_ref().map(|e| Box::new(lower_expr(e))),
            inclusive: *inclusive,
        },
        Expression::Tuple(elems) => {
            HirExpression::Tuple(elems.iter().map(lower_expr).collect())
        }
        Expression::StructLit(tag, fields) => {
            let mut field_map = HashMap::with_capacity(fields.len());
            for (k, v) in fields {
                field_map.insert(k.clone(), lower_expr(v));
            }
            HirExpression::StructLit(tag.borrow().clone(), field_map)
        }
        Expression::TopologyLit(fields) => {
            let mut field_map = HashMap::with_capacity(fields.len());
            for (k, v) in fields {
                field_map.insert(k.clone(), lower_expr(v));
            }
            HirExpression::TopologyLit(field_map)
        }
        Expression::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => HirExpression::EnumVariant {
            enum_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            args: args.iter().map(lower_expr).collect(),
        },
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => HirExpression::If {
            condition: Box::new(lower_expr(condition)),
            then_branch: Box::new(lower_expr(then_branch)),
            else_branch: Box::new(lower_expr(else_branch)),
        },
        Expression::Match { target, arms } => {
            let hir_arms = arms
                .iter()
                .map(|a| HirExprMatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.as_ref().map(lower_expr),
                    body: Box::new(lower_expr(&a.body)),
                })
                .collect();
            HirExpression::Match {
                target: Box::new(lower_expr(target)),
                arms: hir_arms,
            }
        }
        Expression::ToStr(e) => HirExpression::ToStr(Box::new(lower_expr(e))),
        Expression::StrBytes(e) => HirExpression::StrBytes(Box::new(lower_expr(e))),
        Expression::Len(e) => HirExpression::Len(Box::new(lower_expr(e))),
        Expression::RefOp(e) => HirExpression::RefOp(Box::new(lower_expr(e))),
        Expression::CloneOp(name) => HirExpression::CloneOp(name.clone()),
        Expression::ChannelReceive(name) => HirExpression::ChannelReceive(name.clone()),
        Expression::TypeAssertion { target, cast_type } => HirExpression::TypeAssertion {
            target: Box::new(lower_expr(target)),
            cast_type: cast_type.clone(),
        },
        Expression::TypeCast { expr, target_type } => HirExpression::TypeCast {
            expr: Box::new(lower_expr(expr)),
            target_type: target_type.clone(),
        },
        Expression::TryUnwrap(e) => HirExpression::TryUnwrap(Box::new(lower_expr(e))),
        Expression::Syscall {
            target,
            args,
            duration_ms,
        } => HirExpression::Syscall {
            target: target.clone(),
            args: args.iter().map(lower_expr).collect(),
            duration_ms: *duration_ms,
        },
        Expression::ArenaIntrospect(kind) => HirExpression::ArenaIntrospect(*kind),
        Expression::CapabilityCheck(cap) => HirExpression::CapabilityCheck(cap.clone()),
        Expression::Turbofish { expr, .. } => {
            // Desugar turbofish to inner expr in HIR
            lower_expr(expr)
        }
        Expression::FString(parts) => {
            // Desugar f"..." into a canonical string concat chain
            if parts.is_empty() {
                return HirExpression::Literal(String::new());
            }
            let part_to_expr = |p: &FStringPart| -> HirExpression {
                match p {
                    FStringPart::Text(t) => HirExpression::Literal(t.clone()),
                    FStringPart::Expr(e) => HirExpression::ToStr(Box::new(lower_expr(e))),
                }
            };
            let mut acc = part_to_expr(&parts[0]);
            for part in &parts[1..] {
                acc = HirExpression::BinaryOp {
                    op: BinaryOperator::Add,
                    left: Box::new(acc),
                    right: Box::new(part_to_expr(part)),
                };
            }
            acc
        }
        Expression::Deferred { .. } => HirExpression::Null,
    }
}
