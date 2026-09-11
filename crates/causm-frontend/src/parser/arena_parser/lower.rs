use crate::parser::arena_parser::{parse_type_name_str, ArenaParser};
use causm_core::arena::{AstArena, StmtId, StmtNode};

pub fn to_ast_statement(
    arena: &AstArena,
    id: StmtId,
) -> causm_core::SpannedStatement {
    let span = arena
        .stmt_spans
        .get(id.0 as usize)
        .cloned()
        .unwrap_or(causm_core::Span { start: 0, end: 0 });
    let stmt = match &arena.statements[id.0 as usize] {
        StmtNode::Expr(eid) => {
            let expr = crate::parser::pratt::to_ast_expression(arena, *eid);
            match expr {
                causm_core::Expression::Call { routine, mut args }
                    if routine == "await" && !args.is_empty() =>
                {
                    let target_name = match args.remove(0) {
                        causm_core::Expression::Identifier(name) => name,
                        other => format!("{:?}", other),
                    };
                    causm_core::Statement::Await(target_name)
                }
                other => causm_core::Statement::Expression(other),
            }
        }
        StmtNode::Let {
            target,
            is_mut,
            type_annotation,
            init,
            lifetime,
        } => {
            let expr = init
                .map(|eid| crate::parser::pratt::to_ast_expression(arena, eid))
                .unwrap_or(causm_core::Expression::Null);
            let var_type = type_annotation.map(|sym| {
                let s = causm_core::symbol::resolve(sym);
                parse_type_name_str(&s)
            });
            causm_core::Statement::Assignment {
                target: causm_core::symbol::resolve(*target),
                mutable: *is_mut,
                lifetime: lifetime.clone(),
                var_type,
                expr,
            }
        }
        StmtNode::Destructure { fields, expr } => {
            let mut pairs = Vec::new();
            let slice = &arena.symbol_pool[fields.as_range()];
            let mut i = 0;
            while i + 1 < slice.len() {
                let f_str = causm_core::symbol::resolve(slice[i]);
                let t_str = causm_core::symbol::resolve(slice[i + 1]);
                pairs.push((f_str, t_str));
                i += 2;
            }
            causm_core::Statement::DestructureAssignment {
                fields: pairs,
                expr: crate::parser::pratt::to_ast_expression(arena, *expr),
                mutable: false,
            }
        }
        StmtNode::Assign { target, value } => {
            let expr = crate::parser::pratt::to_ast_expression(arena, *value);
            if let causm_core::Expression::Match {
                target: ref match_tgt,
                ref arms,
            } = expr
            {
                if let causm_core::Expression::Call {
                    ref routine,
                    ref args,
                } = **match_tgt
                {
                    if routine == "entropy" && !args.is_empty() {
                        let lhs_name = causm_core::symbol::resolve(*target);
                        let mut valid_branch = None;
                        let mut decayed_branch = None;
                        let mut pending_branch = None;
                        let mut consumed_branch = None;
                        for arm in arms {
                            let (pat_name, binding) = match &arm.pattern {
                                causm_core::Pattern::EnumVariant {
                                    variant_name,
                                    args,
                                    ..
                                } => {
                                    let b = if let Some(
                                        causm_core::Pattern::Identifier(id),
                                    ) = args.first()
                                    {
                                        id.clone()
                                    } else {
                                        String::new()
                                    };
                                    (variant_name.as_str(), b)
                                }
                                causm_core::Pattern::Identifier(id) => {
                                    (id.as_str(), String::new())
                                }
                                _ => ("", String::new()),
                            };
                            let body_stmts =
                                vec![causm_core::SpannedStatement::new(
                                    causm_core::Statement::Assignment {
                                        target: lhs_name.clone(),
                                        mutable: false,
                                        var_type: None,
                                        lifetime: None,
                                        expr: arm.body.clone(),
                                    },
                                    span.clone(),
                                )];
                            match pat_name {
                                "Valid" => {
                                    valid_branch = Some((
                                        causm_core::DecayedPattern::Binding(binding),
                                        arm.guard.clone(),
                                        body_stmts,
                                    ));
                                }
                                "Decayed" => {
                                    decayed_branch = Some((
                                        causm_core::DecayedPattern::Binding(binding),
                                        arm.guard.clone(),
                                        body_stmts,
                                    ));
                                }
                                "Pending" => {
                                    pending_branch = Some((
                                        causm_core::DecayedPattern::Binding(binding),
                                        arm.guard.clone(),
                                        body_stmts,
                                    ));
                                }
                                "Consumed" => {
                                    consumed_branch =
                                        Some((arm.guard.clone(), body_stmts));
                                }
                                _ => {}
                            }
                        }
                        return causm_core::SpannedStatement::with_attributes(
                            causm_core::Statement::MatchEntropy {
                                target: args[0].clone(),
                                valid_branch,
                                decayed_branch,
                                pending_branch,
                                consumed_branch,
                            },
                            span,
                            arena
                                .stmt_attributes
                                .get(&id.0)
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                }
            }
            causm_core::Statement::Assignment {
                target: causm_core::symbol::resolve(*target),
                mutable: true,
                lifetime: None,
                var_type: None,
                expr,
            }
        }
        StmtNode::FieldUpdate {
            target,
            field,
            value,
        } => {
            let target_expr =
                crate::parser::pratt::to_ast_expression(arena, *target);
            let val_expr = crate::parser::pratt::to_ast_expression(arena, *value);
            let field_str = causm_core::symbol::resolve(*field);
            causm_core::Statement::FieldUpdate {
                target: target_expr,
                field: field_str,
                value: val_expr,
            }
        }
        StmtNode::Return(val) => {
            let expr =
                val.map(|eid| crate::parser::pratt::to_ast_expression(arena, eid));
            causm_core::Statement::Return(expr)
        }
        StmtNode::Yield(eid) => causm_core::Statement::Yield(Some(
            crate::parser::pratt::to_ast_expression(arena, *eid),
        )),
        StmtNode::RoutineDef {
            name,
            params,
            return_type,
            taking_ms,
            state_constraint,
            required_capabilities,
            body,
        } => {
            let r_name = causm_core::symbol::resolve(*name);
            let receiver_type = if let Some((type_name, _)) = r_name.split_once('.')
            {
                Some(causm_core::TypeName::Custom(type_name.to_string()))
            } else {
                None
            };
            let mut param_decls = Vec::new();
            let p_slice = &arena.symbol_pool[params.as_range()];
            for chunk in p_slice.chunks(3) {
                if chunk.len() == 3 {
                    let mode_str = causm_core::symbol::resolve(chunk[0]);
                    let mode = match mode_str.as_str() {
                        "consume" => causm_core::ParamMode::Consume,
                        "clone" => causm_core::ParamMode::Clone,
                        "decay" => causm_core::ParamMode::Decay,
                        "lease" => causm_core::ParamMode::Lease,
                        _ => causm_core::ParamMode::Peek,
                    };
                    let type_str = causm_core::symbol::resolve(chunk[2]);
                    let typ = parse_type_name_str(&type_str);
                    let param_name = causm_core::symbol::resolve(chunk[1]);
                    let final_typ = if param_name == "self" && type_str.is_empty() {
                        receiver_type.clone()
                    } else if type_str.is_empty() {
                        None
                    } else {
                        Some(typ)
                    };
                    param_decls.push(causm_core::ParamDecl {
                        mode,
                        name: param_name,
                        typ: final_typ,
                    });
                }
            }
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let ret_type = return_type.map(|rt| {
                causm_core::TypeName::from_str_name(&causm_core::symbol::resolve(rt))
            });
            let sc = state_constraint.map(|(v, s)| {
                (
                    causm_core::symbol::resolve(v),
                    causm_core::symbol::resolve(s),
                )
            });
            causm_core::Statement::RoutineDef {
                name: causm_core::symbol::resolve(*name),
                params: param_decls,
                return_type: ret_type,
                taking_ms: *taking_ms,
                state_constraint: sc,
                required_capabilities: required_capabilities
                    .iter()
                    .map(|&c| causm_core::Capability {
                        path: causm_core::symbol::resolve(c),
                        parameters: std::collections::HashMap::new(),
                    })
                    .collect(),
                body: body_stmts,
            }
        }
        StmtNode::Isolate { name, body } => {
            let mut manifest = causm_core::Manifest::default();
            let mut body_stmts = Vec::new();
            let mut in_manifest = true;
            for &sid in &arena.stmt_pool[body.as_range()] {
                let stmt_node = &arena.statements[sid.0 as usize];
                if in_manifest {
                    match stmt_node {
                        StmtNode::Capability(cap) => {
                            manifest.capabilities.push(cap.clone());
                            continue;
                        }
                        StmtNode::EnableResource {
                            resource,
                            amount,
                            unit,
                        } => {
                            let r_name = causm_core::symbol::resolve(*resource);
                            let u_str = unit.map(causm_core::symbol::resolve);
                            match r_name.as_str() {
                                "slice" => {
                                    manifest.slice_ms = Some(*amount);
                                }
                                "cpu" => {
                                    manifest.cpu_budget_ms = Some(*amount);
                                }
                                "memory" => {
                                    let mult = match u_str.as_deref() {
                                        Some("KB") => 1024,
                                        Some("MB") => 1024 * 1024,
                                        Some("GB") => 1024 * 1024 * 1024,
                                        _ => 1,
                                    };
                                    manifest.memory_budget_bytes =
                                        Some(*amount * mult);
                                }
                                _ => {
                                    manifest
                                        .resource_budgets
                                        .insert(r_name, *amount);
                                }
                            }
                            continue;
                        }
                        _ => {
                            in_manifest = false;
                        }
                    }
                }
                body_stmts.push(to_ast_statement(arena, sid));
            }
            causm_core::Statement::Isolate(causm_core::IsolateBlock {
                name: Some(causm_core::symbol::resolve(*name)),
                manifest,
                body: body_stmts,
            })
        }
        StmtNode::Capability(cap) => causm_core::Statement::Capability(cap.clone()),
        StmtNode::TimelineBlock {
            coord,
            directives,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            if !directives.is_empty() {
                causm_core::Statement::DirectiveBlock {
                    directives: directives.clone(),
                    body: body_stmts,
                }
            } else {
                causm_core::Statement::RelativisticBlock {
                    time: coord.clone(),
                    body: body_stmts,
                }
            }
        }
        StmtNode::Send { target, payload: _ } => causm_core::Statement::Send {
            value_id: "__send_value".to_string(),
            target_branch: causm_core::symbol::resolve(*target),
        },
        StmtNode::Split { parent, branches } => {
            let b_list = arena.symbol_pool[branches.as_range()]
                .iter()
                .map(|&b| causm_core::symbol::resolve(b))
                .collect();
            causm_core::Statement::Split {
                parent: causm_core::symbol::resolve(*parent),
                branches: b_list,
            }
        }
        StmtNode::Merge {
            branches,
            target,
            resolutions,
        } => {
            let b_list = arena.symbol_pool[branches.as_range()]
                .iter()
                .map(|&b| causm_core::symbol::resolve(b))
                .collect();
            let fallback = resolutions.fallback.map(|fb| {
                arena.stmt_pool[fb.as_range()]
                    .iter()
                    .map(|&s| to_ast_statement(arena, s))
                    .collect()
            });
            causm_core::Statement::Merge {
                branches: b_list,
                target: causm_core::symbol::resolve(*target),
                resolutions: causm_core::MergeResolution {
                    rules: resolutions.rules.clone(),
                    auto: resolutions.auto,
                    fallback,
                    taking_ms: resolutions.taking_ms,
                },
            }
        }
        StmtNode::Import { path, alias } => causm_core::Statement::Import {
            path: causm_core::symbol::resolve(*path),
            alias: alias.map(causm_core::symbol::resolve),
        },
        StmtNode::FromImport { path, symbols } => {
            let sym_list = arena.symbol_pool[symbols.as_range()]
                .iter()
                .map(|&s| {
                    let s_str = causm_core::symbol::resolve(s);
                    if let Some((name, alias)) = s_str.split_once(" as ") {
                        (name.trim().to_string(), Some(alias.trim().to_string()))
                    } else {
                        (s_str, None)
                    }
                })
                .collect();
            causm_core::Statement::FromImport {
                path: causm_core::symbol::resolve(*path),
                symbols: sym_list,
            }
        }
        StmtNode::TypeDecl {
            name,
            extends,
            fields,
            decay_after_ms,
            auto_drop,
        } => {
            let mut field_map = std::collections::HashMap::new();
            for f in &arena.field_assigns_pool[fields.as_range()] {
                let default_val = if f.expr.0 != u32::MAX {
                    Some(crate::parser::pratt::to_ast_expression(arena, f.expr))
                } else {
                    None
                };
                let type_str = f
                    .type_name
                    .map(causm_core::symbol::resolve)
                    .unwrap_or_else(|| "any".into());
                let typ = causm_core::TypeName::from_str_name(&type_str);
                field_map.insert(
                    causm_core::symbol::resolve(f.field),
                    causm_core::TypeFieldDef {
                        typ,
                        is_const: f.is_const,
                        default_value: default_val,
                    },
                );
            }
            causm_core::Statement::TypeDecl {
                name: causm_core::symbol::resolve(*name),
                extends: extends.map(causm_core::symbol::resolve),
                fields: field_map,
                decay_after_ms: *decay_after_ms,
                auto_drop: auto_drop.clone(),
                scoped_branch: None,
            }
        }
        StmtNode::InterfaceDecl {
            name,
            extends,
            methods,
        } => {
            let extends_vec: Vec<String> = arena.symbol_pool[extends.as_range()]
                .iter()
                .map(|&s| causm_core::symbol::resolve(s))
                .collect();
            let mut iface_methods = Vec::new();
            for &mid in &arena.stmt_pool[methods.as_range()] {
                let spanned_stmt = to_ast_statement(arena, mid);
                if let causm_core::Statement::RoutineDef {
                    name,
                    params,
                    return_type,
                    taking_ms,
                    state_constraint,
                    required_capabilities,
                    body,
                } = spanned_stmt.stmt
                {
                    let default_body =
                        if body.is_empty() { None } else { Some(body) };
                    iface_methods.push(causm_core::InterfaceMethod {
                        name,
                        params,
                        return_type,
                        taking_ms,
                        default_body,
                        state_constraint,
                        required_capabilities,
                    });
                }
            }
            causm_core::Statement::InterfaceDecl {
                name: causm_core::symbol::resolve(*name),
                extends: extends_vec,
                methods: iface_methods,
            }
        }
        StmtNode::EnumDecl { name, variants } => causm_core::Statement::EnumDecl {
            name: causm_core::symbol::resolve(*name),
            variants: variants.clone(),
        },
        StmtNode::MacroDef {
            name,
            params,
            body_template,
        } => causm_core::Statement::MacroDef {
            name: causm_core::symbol::resolve(*name),
            params: params.clone(),
            body_template: body_template.clone(),
        },
        StmtNode::If {
            cond,
            then_branch,
            else_branch,
            reconcile_auto,
        } => {
            let then_stmts = arena.stmt_pool[then_branch.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let else_stmts = else_branch.map(|eb| {
                arena.stmt_pool[eb.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            let reconcile = if *reconcile_auto {
                Some(causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };
            causm_core::Statement::If {
                binding: None,
                condition: crate::parser::pratt::to_ast_expression(arena, *cond),
                then_branch: then_stmts,
                else_branch: else_stmts,
                reconcile,
            }
        }
        StmtNode::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
            reconcile_auto,
        } => {
            let pat_str = causm_core::symbol::resolve(*pattern);
            let then_stmts = arena.stmt_pool[then_branch.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let else_stmts = else_branch.map(|eb| {
                arena.stmt_pool[eb.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            let reconcile = if *reconcile_auto {
                Some(causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };
            let ast_expr = crate::parser::pratt::to_ast_expression(arena, *expr);
            if let causm_core::Expression::TypeAssertion { .. } = &ast_expr {
                let pat = crate::parser::pratt::parse_pattern_from_str(&pat_str);
                if let causm_core::Pattern::Identifier(binding_id) = pat {
                    causm_core::Statement::If {
                        condition: ast_expr,
                        binding: Some(binding_id),
                        then_branch: then_stmts,
                        else_branch: else_stmts,
                        reconcile,
                    }
                } else {
                    causm_core::Statement::IfLet {
                        pattern: crate::parser::pratt::parse_pattern_from_str(
                            &pat_str,
                        ),
                        expr: ast_expr,
                        then_branch: then_stmts,
                        else_branch: else_stmts,
                        reconcile,
                    }
                }
            } else {
                causm_core::Statement::IfLet {
                    pattern: crate::parser::pratt::parse_pattern_from_str(&pat_str),
                    expr: ast_expr,
                    then_branch: then_stmts,
                    else_branch: else_stmts,
                    reconcile,
                }
            }
        }
        StmtNode::Loop {
            max_ms,
            step_ms,
            is_tick,
            body,
        } => {
            let mut body_stmts: Vec<_> = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            if *is_tick {
                causm_core::Statement::LoopTick { body: body_stmts }
            } else {
                if let Some(step) = step_ms {
                    body_stmts.push(causm_core::SpannedStatement {
                        stmt: causm_core::Statement::Slice {
                            milliseconds: *step,
                        },
                        span: span.clone(),
                        attributes: Vec::new(),
                    });
                }
                causm_core::Statement::Loop {
                    max_ms: max_ms.unwrap_or(0),
                    body: body_stmts,
                }
            }
        }
        StmtNode::LoopOn { target, body } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::LoopOn {
                target: crate::parser::pratt::to_ast_expression(arena, *target),
                body: body_stmts,
            }
        }
        StmtNode::While {
            cond,
            max_ms,
            step_ms,
            body,
        } => {
            let mut body_stmts: Vec<_> = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            if let Some(step) = step_ms {
                body_stmts.push(causm_core::SpannedStatement {
                    stmt: causm_core::Statement::Slice {
                        milliseconds: *step,
                    },
                    span: span.clone(),
                    attributes: Vec::new(),
                });
            }
            let raw_cond = crate::parser::pratt::to_ast_expression(arena, *cond);
            let (cond_expr, is_valid_check) = match raw_cond {
                causm_core::Expression::Call {
                    ref routine,
                    ref args,
                } if routine == "valid" && args.len() == 1 => {
                    (args[0].clone(), true)
                }
                other => (other, false),
            };
            causm_core::Statement::While {
                condition: cond_expr,
                is_valid_check,
                max_ms: max_ms.unwrap_or(0),
                body: body_stmts,
            }
        }
        StmtNode::Using {
            binding,
            resource,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::Using {
                binding: causm_core::symbol::resolve(*binding),
                resource: crate::parser::pratt::to_ast_expression(arena, *resource),
                body: body_stmts,
            }
        }
        StmtNode::Print(args) => {
            let arg_exprs = arena.expr_pool[args.as_range()]
                .iter()
                .map(|&eid| crate::parser::pratt::to_ast_expression(arena, eid))
                .collect();
            causm_core::Statement::Print(arg_exprs)
        }
        StmtNode::Break => causm_core::Statement::Break,
        StmtNode::Continue => {
            causm_core::Statement::Expression(causm_core::Expression::Null)
        }
        StmtNode::Collapse => causm_core::Statement::Collapse,
        StmtNode::Slice(eid) => {
            let ms = match &arena.expressions[eid.0 as usize] {
                causm_core::arena::ExprNode::Literal(
                    causm_core::arena::LiteralKind::Duration(d),
                ) => *d,
                causm_core::arena::ExprNode::Literal(
                    causm_core::arena::LiteralKind::Integer(i),
                ) => *i as u64,
                _ => 0,
            };
            causm_core::Statement::Slice { milliseconds: ms }
        }
        StmtNode::AssertTime {
            operator,
            limit_ms,
            fallback,
        } => {
            let fb = fallback.as_ref().map(|b| {
                arena.stmt_pool[b.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            causm_core::Statement::AssertTime {
                operator: *operator,
                limit_ms: *limit_ms,
                fallback: fb,
            }
        }
        StmtNode::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let fb = fallback.as_ref().map(|b| {
                arena.stmt_pool[b.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect()
            });
            causm_core::Statement::Speculate {
                max_ms: *max_ms,
                body: body_stmts,
                fallback: fb,
            }
        }
        StmtNode::Commit(body) => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::Commit(body_stmts)
        }
        StmtNode::ForeignBlock {
            lib_name,
            abi,
            routines,
        } => {
            let r_stmts = arena.stmt_pool[routines.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::ForeignBlock {
                lib_name: causm_core::symbol::resolve(*lib_name),
                abi: causm_core::symbol::resolve(*abi),
                routines: r_stmts,
            }
        }
        StmtNode::For {
            var_name,
            mode,
            iter_expr,
            pacing_ms,
            max_ms,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::For {
                item_name: causm_core::symbol::resolve(*var_name),
                mode: mode.clone(),
                source: match &arena.expressions[iter_expr.0 as usize] {
                    causm_core::arena::ExprNode::Identifier(s) => {
                        causm_core::symbol::resolve(*s)
                    }
                    _ => "__iter".into(),
                },
                pacing_ms: *pacing_ms,
                max_ms: *max_ms,
                body: body_stmts,
            }
        }
        StmtNode::Entangle(range) => {
            let vars = arena.symbol_pool[range.as_range()]
                .iter()
                .map(|sym| causm_core::symbol::resolve(*sym))
                .collect();
            causm_core::Statement::Entangle { variables: vars }
        }
        StmtNode::Debug(eid) => causm_core::Statement::Debug(
            crate::parser::pratt::to_ast_expression(arena, *eid),
        ),
        StmtNode::DecayHandler { type_name, body } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            causm_core::Statement::DecayHandler {
                type_name: causm_core::symbol::resolve(*type_name),
                body: body_stmts,
            }
        }
        StmtNode::ForStep {
            var_name,
            start_expr,
            end_expr,
            step_ms,
            body,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let source_expr = if start_expr.0 == end_expr.0 {
                crate::parser::pratt::to_ast_expression(arena, *start_expr)
            } else {
                match (
                    &arena.expressions[start_expr.0 as usize],
                    &arena.expressions[end_expr.0 as usize],
                ) {
                    (
                        causm_core::arena::ExprNode::Literal(
                            causm_core::arena::LiteralKind::Integer(s),
                        ),
                        causm_core::arena::ExprNode::Literal(
                            causm_core::arena::LiteralKind::Integer(e),
                        ),
                    ) => {
                        let elems: Vec<causm_core::Expression> =
                            (*s..*e).map(causm_core::Expression::Integer).collect();
                        causm_core::Expression::ArrayLiteral(elems)
                    }
                    (
                        causm_core::arena::ExprNode::Literal(
                            causm_core::arena::LiteralKind::Duration(s),
                        ),
                        causm_core::arena::ExprNode::Literal(
                            causm_core::arena::LiteralKind::Duration(e),
                        ),
                    ) => {
                        let elems: Vec<causm_core::Expression> = (*s..*e)
                            .map(|v| causm_core::Expression::Integer(v as i64))
                            .collect();
                        causm_core::Expression::ArrayLiteral(elems)
                    }
                    _ => crate::parser::pratt::to_ast_expression(arena, *start_expr),
                }
            };
            let final_step_ms = if *step_ms == 0 || *step_ms == u64::MAX {
                None
            } else {
                Some(*step_ms)
            };
            causm_core::Statement::ForStep {
                item_name: causm_core::symbol::resolve(*var_name),
                source: source_expr,
                step_ms: final_step_ms,
                body: body_stmts,
            }
        }
        StmtNode::Lease {
            binding,
            source,
            duration_ms,
            body,
            reconcile_auto,
        } => {
            let body_stmts = arena.stmt_pool[body.as_range()]
                .iter()
                .map(|&sid| to_ast_statement(arena, sid))
                .collect();
            let reconcile = if *reconcile_auto {
                Some(causm_core::MergeResolution {
                    rules: std::collections::HashMap::new(),
                    auto: true,
                    fallback: None,
                    taking_ms: None,
                })
            } else {
                None
            };
            causm_core::Statement::Lease {
                binding: causm_core::symbol::resolve(*binding),
                source: causm_core::symbol::resolve(*source),
                duration_ms: *duration_ms,
                body: body_stmts,
                reconcile,
            }
        }
        StmtNode::Anchor(name) => {
            causm_core::Statement::Anchor(causm_core::symbol::resolve(*name))
        }
        StmtNode::RewindTo(name) => {
            causm_core::Statement::Rewind(causm_core::symbol::resolve(*name))
        }
        StmtNode::State { name, value } => causm_core::Statement::StateDecl {
            target: causm_core::symbol::resolve(*name),
            var_type: None,
            expr: crate::parser::pratt::to_ast_expression(arena, *value),
        },
        StmtNode::Policy { target, kind } => {
            let t_str = causm_core::symbol::resolve(*target);
            let k_str = causm_core::symbol::resolve(*kind);
            let target_enum = match t_str.as_str() {
                "on_deadline_breach" => causm_core::PolicyTarget::OnDeadlineBreach,
                "on_overflow" => causm_core::PolicyTarget::OnOverflow,
                _ => causm_core::PolicyTarget::OnFull,
            };
            let kind_enum = match k_str.as_str() {
                "RingBuffer" => causm_core::SaturationPolicy::RingBuffer,
                "Throttle" => causm_core::SaturationPolicy::Throttle,
                "FailFast" => causm_core::SaturationPolicy::FailFast,
                _ => causm_core::SaturationPolicy::EvictDecayed,
            };
            causm_core::Statement::PolicyStmt {
                target: target_enum,
                policy: kind_enum,
            }
        }
        StmtNode::Select { max_ms, cases } => {
            let mut select_cases = Vec::new();
            for &cid in &arena.stmt_pool[cases.as_range()] {
                if let StmtNode::Assign { target, value } =
                    &arena.statements[cid.0 as usize]
                {
                    select_cases.push(causm_core::SelectCase {
                        binding: causm_core::symbol::resolve(*target),
                        source: crate::parser::pratt::to_ast_expression(
                            arena, *value,
                        ),
                        body: Vec::new(),
                    });
                }
            }
            causm_core::Statement::Select {
                max_ms: *max_ms,
                cases: select_cases,
                timeout: None,
                reconcile: None,
            }
        }
        StmtNode::Match { target, arms } => {
            let mut is_entropy = false;
            let mut valid_branch = None;
            let mut decayed_branch = None;
            let mut pending_branch = None;
            let mut consumed_branch = None;
            let mut std_arms = Vec::new();

            for arm in &arena.match_arms_pool[arms.as_range()] {
                let pat_str = causm_core::symbol::resolve(arm.pattern);
                let guard_expr = arm
                    .guard
                    .map(|gid| crate::parser::pratt::to_ast_expression(arena, gid));
                let body_stmts = arena.stmt_pool[arm.body.as_range()]
                    .iter()
                    .map(|&sid| to_ast_statement(arena, sid))
                    .collect();
                match pat_str.as_str() {
                    s if s.starts_with("Valid") => {
                        is_entropy = true;
                        if valid_branch.is_none() {
                            let binding = s
                                .split_once(':')
                                .map(|(_, b)| b.trim().to_string())
                                .unwrap_or_else(|| {
                                    if let Some((_, rest)) = s.split_once('(') {
                                        rest.trim_end_matches(')').trim().to_string()
                                    } else {
                                        String::new()
                                    }
                                });
                            valid_branch = Some((
                                causm_core::DecayedPattern::Binding(binding),
                                guard_expr,
                                body_stmts,
                            ));
                        }
                    }
                    s if s.starts_with("Decayed") => {
                        is_entropy = true;
                        let pattern = if let Some((_, payload)) = s.split_once(':') {
                            let mut fields = std::collections::HashMap::new();
                            let tokens: Vec<&str> =
                                payload.split_whitespace().collect();
                            let mut i = 0;
                            while i < tokens.len() {
                                let key = tokens[i];
                                if !matches!(
                                    key,
                                    "Valid" | "Decayed" | "Pending" | "Consumed"
                                ) {
                                    let state_val = if i + 1 < tokens.len()
                                        && matches!(
                                            tokens[i + 1],
                                            "Valid"
                                                | "Decayed"
                                                | "Pending"
                                                | "Consumed"
                                        ) {
                                        i += 1;
                                        tokens[i]
                                    } else {
                                        "Valid"
                                    };
                                    fields.insert(
                                        key.to_string(),
                                        causm_core::PatternValue::State(
                                            state_val.to_string(),
                                        ),
                                    );
                                }
                                i += 1;
                            }
                            if fields.is_empty() {
                                causm_core::DecayedPattern::Binding(String::new())
                            } else {
                                causm_core::DecayedPattern::Fields(fields)
                            }
                        } else {
                            causm_core::DecayedPattern::Binding(String::new())
                        };
                        decayed_branch = Some((pattern, guard_expr, body_stmts));
                    }
                    "Pending" => {
                        is_entropy = true;
                        pending_branch = Some((
                            causm_core::DecayedPattern::Binding(String::new()),
                            guard_expr,
                            body_stmts,
                        ));
                    }
                    "Consumed" => {
                        is_entropy = true;
                        consumed_branch = Some((guard_expr, body_stmts));
                    }
                    _ => {
                        std_arms.push(causm_core::MatchArm {
                            pattern: crate::parser::pratt::parse_pattern_from_str(
                                &pat_str,
                            ),
                            guard: guard_expr,
                            body: body_stmts,
                        });
                    }
                }
            }

            if is_entropy {
                let tgt_expr =
                    crate::parser::pratt::to_ast_expression(arena, *target);
                let unwrapped_tgt = match tgt_expr {
                    causm_core::Expression::Call { routine, mut args }
                        if routine == "entropy" && !args.is_empty() =>
                    {
                        args.remove(0)
                    }
                    other => other,
                };
                causm_core::Statement::MatchEntropy {
                    target: unwrapped_tgt,
                    valid_branch,
                    decayed_branch,
                    pending_branch,
                    consumed_branch,
                }
            } else {
                causm_core::Statement::Match {
                    target: crate::parser::pratt::to_ast_expression(arena, *target),
                    arms: std_arms,
                }
            }
        }
        _ => causm_core::Statement::Expression(causm_core::Expression::Null),
    };
    let attrs = arena
        .stmt_attributes
        .get(&id.0)
        .cloned()
        .unwrap_or_default();
    causm_core::SpannedStatement::with_attributes(stmt, span, attrs)
}

pub fn parse_arena_program_to_ast(
    source: &str,
) -> Result<causm_core::Program, String> {
    let mut parser = ArenaParser::new(source);
    let root = parser.parse_program()?;
    let mut timelines = Vec::new();
    let mut standalone = Vec::new();

    for &sid in &parser.arena.root_statements[root.as_range()] {
        if let StmtNode::TimelineBlock {
            coord,
            directives,
            body,
        } = &parser.arena.statements[sid.0 as usize]
        {
            let mut no_z3 = false;
            let mut entropy_mode = None;
            for dir in directives {
                match dir {
                    causm_core::BlockDirective::NoZ3 => no_z3 = true,
                    causm_core::BlockDirective::Chaos => {
                        entropy_mode = Some(causm_core::EntropyMode::Chaos);
                    }
                    causm_core::BlockDirective::Deterministic => {
                        entropy_mode = Some(causm_core::EntropyMode::Deterministic);
                    }
                }
            }
            let mut stmts = Vec::new();
            for &s in &parser.arena.stmt_pool[body.as_range()] {
                stmts.push(to_ast_statement(&parser.arena, s));
            }
            timelines.push(causm_core::TimelineBlock {
                time: coord.clone(),
                no_z3,
                entropy_mode,
                statements: stmts,
            });
        } else {
            standalone.push(to_ast_statement(&parser.arena, sid));
        }
    }

    if !standalone.is_empty() {
        timelines.insert(
            0,
            causm_core::TimelineBlock {
                time: causm_core::TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: standalone,
            },
        );
    }

    let mut prog = causm_core::Program { timelines };
    crate::macro_expand::expand_program(&mut prog);
    crate::derive::expand_derives(&mut prog);
    Ok(prog)
}

pub fn parse_arena_program_to_hir(
    source: &str,
) -> Result<causm_core::HirProgram, String> {
    let prog = parse_arena_program_to_ast(source)?;
    Ok(crate::hir::lower_ast_to_hir(&prog))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_arena_parser_parse_to_hir() {
        let src = r#"
            routine double(x: int) -> int {
                return x * 2
            }
            let val = double(21)
        "#;
        let hir = parse_arena_program_to_hir(src)
            .expect("should parse directly to hir successfully");
        assert!(!hir.timelines.is_empty());
        assert_eq!(hir.timelines[0].statements.len(), 2);
    }

    #[test]
    fn test_syntax_arena_parser_expressions_and_routines() {
        let src = r#"
            routine add(x: int, y: int) -> int {
                return x + y * 2
            }
            let a = 10
            let b = add(a, 5)
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse successfully without semicolons");
        assert!(!prog.timelines.is_empty());
        let stmts = &prog.timelines[0].statements;
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_syntax_arena_parser_struct_and_enum() {
        let src = r#"
            struct Point {
                x: int = 0,
                y: int = 0
            }
            enum Status {
                Active,
                Inactive
            }
            let p = Point;
        "#;
        let prog =
            parse_arena_program_to_ast(src).expect("should parse successfully");
        assert!(!prog.timelines.is_empty());
        let stmts = &prog.timelines[0].statements;
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn test_syntax_arena_parser_advanced_statements() {
        let src = r#"
            anchor checkpoint_1;
            rewind_to(checkpoint_1);
            lease res = source 500ms {
                print("in lease");
            }
            for i in 0..10 step 100ms {
                print(i);
            }
            let p = Point { x: 10, y: 20 };
            let rep = [0; 8];
            let tup = (1, 2, 3);
            let s = syscall(1, 42);
            let m = match x {
                1 => 10,
                _ => 20
            };
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse all advanced statements");
        assert!(!prog.timelines.is_empty());
        assert_eq!(prog.timelines[0].statements.len(), 9);
    }

    #[test]
    fn test_syntax_enum_variant_nested_call_argument_isolation() {
        let src = r#"
            let agent_opt = if (cond) {
                Option::Some(clone(extracted_id))
            } else {
                Option::None
            }
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse nested call inside enum variant without arg pool corruption");
        assert!(!prog.timelines.is_empty());
        if let causm_core::Statement::Assignment { expr, .. } =
            &prog.timelines[0].statements[0].stmt
        {
            if let causm_core::Expression::If { then_branch, .. } = expr {
                if let causm_core::Expression::EnumVariant { args, .. } =
                    &**then_branch
                {
                    assert_eq!(
                        args.len(),
                        1,
                        "Option::Some should only have 1 argument"
                    );
                } else {
                    panic!("expected EnumVariant then branch");
                }
            } else {
                panic!("expected If expression");
            }
        } else {
            panic!("expected Assignment statement");
        }
    }

    #[test]
    fn test_syntax_loop_step_and_tick_modifiers() {
        let src = r#"
            loop step 10ms max 30ms {
                break
            }
            loop tick {
                break
            }
            for m in members step _ {
                let x = m
            }
            for x consume arr pacing 5ms (max 20ms) {
                let y = x
            }
        "#;
        let prog = parse_arena_program_to_ast(src).expect(
            "should parse loops with distinct step, tick, and pacing modifiers",
        );
        assert!(!prog.timelines.is_empty());
        let stmts = &prog.timelines[0].statements;
        assert_eq!(stmts.len(), 4);
        assert!(matches!(
            stmts[0].stmt,
            causm_core::Statement::Loop { max_ms: 30, .. }
        ));
        assert!(matches!(
            stmts[1].stmt,
            causm_core::Statement::LoopTick { .. }
        ));
        assert!(matches!(
            stmts[2].stmt,
            causm_core::Statement::ForStep { step_ms: None, .. }
        ));
        assert!(matches!(
            stmts[3].stmt,
            causm_core::Statement::For {
                pacing_ms: Some(5),
                max_ms: Some(20),
                ..
            }
        ));
    }

    #[test]
    fn test_syntax_namespaced_generic_static_call() {
        let src = r#"
            let buf = Collection.Buffer<u8>::new(1024)
        "#;
        let prog = parse_arena_program_to_ast(src)
            .expect("should parse namespaced generic static call successfully");
        assert!(!prog.timelines.is_empty());
        if let causm_core::Statement::Assignment { expr, .. } =
            &prog.timelines[0].statements[0].stmt
        {
            if let causm_core::Expression::Call { routine, args } = expr {
                assert_eq!(routine, "Collection.Buffer.new");
                assert_eq!(args.len(), 1);
            } else {
                panic!("expected Call expression");
            }
        } else {
            panic!("expected Assignment statement");
        }
    }

    #[test]
    fn test_syntax_state_type_annotation() {
        let src = r#"
            state total_cycles: int = 0
            state persistent_buffer: [int] = [0, 0, 0, 0]
        "#;
        let prog = parse_arena_program_to_ast(src).expect(
            "should parse state declarations with complex types successfully",
        );
        assert!(!prog.timelines.is_empty());
        assert_eq!(prog.timelines[0].statements.len(), 2);
        assert!(matches!(
            prog.timelines[0].statements[0].stmt,
            causm_core::Statement::StateDecl { .. }
        ));
        assert!(matches!(
            prog.timelines[0].statements[1].stmt,
            causm_core::Statement::StateDecl { .. }
        ));
    }

    #[test]
    fn test_syntax_select_timeout_reconcile() {
        let src = r#"
            select (taking 10ms) {
                timeout: {
                    print("Select timed out")
                }
            } reconcile auto
        "#;
        let prog = parse_arena_program_to_ast(src).expect(
            "should parse select with timeout and reconcile auto successfully",
        );
        assert!(!prog.timelines.is_empty());
        assert!(matches!(
            prog.timelines[0].statements[0].stmt,
            causm_core::Statement::Select { max_ms: 10, .. }
        ));
    }
}
