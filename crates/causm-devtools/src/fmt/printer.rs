use super::rules::FormatConfig;
use causm_core::{
    BinaryOperator, Expression, FStringPart, LifetimeAnnotation, MergeResolution,
    ParamDecl, ParamMode, Pattern, Program, SpannedStatement, Statement,
    SyscallTarget, TypeName, TypeParam, UnaryOperator,
};

/// Formats a full Causm AST Program according to the provided `FormatConfig`.
pub fn format_program(program: &Program, config: &FormatConfig) -> String {
    let mut out = String::new();
    for (i, tb) in program.timelines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let header = match &tb.time {
            causm_core::TimeCoordinate::Global(t) => format!("@{}ms", t),
            causm_core::TimeCoordinate::Relative(t) => format!("@+{}ms", t),
            causm_core::TimeCoordinate::Branch(b) => format!("@{}", b),
            causm_core::TimeCoordinate::Periodic(t) => format!("@every {}ms", t),
        };
        let directives = if tb.no_z3 { " @no_z3" } else { "" };
        out.push_str(&format!("{}{}: {{\n", header, directives));
        let mut prev_was_def = false;
        for stmt in &tb.statements {
            let is_def = matches!(
                stmt.stmt,
                Statement::RoutineDef { .. }
                    | Statement::TypeDecl { .. }
                    | Statement::InterfaceDecl { .. }
                    | Statement::EnumDecl { .. }
                    | Statement::DecayHandler { .. }
                    | Statement::Isolate { .. }
                    | Statement::ForeignBlock { .. }
            );
            if prev_was_def || (is_def && !out.ends_with("{\n")) {
                out.push('\n');
            }
            format_spanned_statement(&mut out, stmt, config, 1);
            prev_was_def = is_def;
        }
        out.push_str("}\n");
    }
    out
}

fn indent_str(config: &FormatConfig, depth: usize) -> String {
    " ".repeat(config.indent_spaces * depth)
}

fn format_spanned_statement(
    out: &mut String,
    stmt: &SpannedStatement,
    config: &FormatConfig,
    depth: usize,
) {
    let indent = indent_str(config, depth);
    match &stmt.stmt {
        Statement::Assignment {
            target,
            mutable,
            var_type,
            lifetime,
            expr,
        } => {
            let mut_str = if *mutable { "mut " } else { "" };
            let lt_str = match lifetime {
                Some(LifetimeAnnotation::Valid) => "@valid ",
                Some(LifetimeAnnotation::Decayed(ms)) => {
                    &format!("@decayed({}ms) ", ms)
                }
                Some(LifetimeAnnotation::DecayRate(ms)) => {
                    &format!("@decay_rate({}ms) ", ms)
                }
                None => "",
            };
            let type_str = if let Some(t) = var_type {
                format!(": {}", format_type(t))
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{}let {}{}{}{} = {}\n",
                indent,
                mut_str,
                lt_str,
                target,
                type_str,
                format_expr(expr, config, depth)
            ));
        }
        Statement::DestructureAssignment {
            fields,
            mutable,
            expr,
        } => {
            let mut_str = if *mutable { "mut " } else { "" };
            let fields_str = fields
                .iter()
                .map(|(src, alias)| {
                    if src == alias {
                        src.clone()
                    } else {
                        format!("{} as {}", src, alias)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "{}let {}{{ {} }} = {}\n",
                indent,
                mut_str,
                fields_str,
                format_expr(expr, config, depth)
            ));
        }
        Statement::Using {
            binding,
            resource,
            body,
        } => {
            out.push_str(&format!(
                "{}using {} = {} {{\n",
                indent,
                binding,
                format_expr(resource, config, depth)
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
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
            let params_str = params
                .iter()
                .map(format_param)
                .collect::<Vec<_>>()
                .join(", ");
            let ret_str = if let Some(rt) = return_type {
                format!(" -> {}", format_type(rt))
            } else {
                String::new()
            };
            let req_str = if !required_capabilities.is_empty() {
                let caps = required_capabilities
                    .iter()
                    .map(|c| {
                        if c.parameters.is_empty() {
                            c.path.clone()
                        } else {
                            let mut sorted_params: Vec<_> =
                                c.parameters.iter().collect();
                            sorted_params.sort_by_key(|(k, _)| k.as_str());
                            let p_strs = sorted_params
                                .iter()
                                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("{}[{}]", c.path, p_strs)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(" require {}", caps)
            } else {
                String::new()
            };
            let contract_str = match taking_ms {
                Some(ms) => format!(" taking {}ms", ms),
                None => " taking _".to_string(),
            };
            let state_str = if let Some((var, state)) = state_constraint {
                format!(" where {}.state == {}", var, state)
            } else {
                String::new()
            };
            if body.len() == 1 {
                if let Statement::Expression(ref expr) = body[0].stmt {
                    out.push_str(&format!(
                        "{}routine {}({}){}{}{}{} => {}\n",
                        indent,
                        name,
                        params_str,
                        ret_str,
                        req_str,
                        contract_str,
                        state_str,
                        format_expr(expr, config, depth)
                    ));
                    return;
                }
            }
            if body.is_empty() {
                out.push_str(&format!(
                    "{}routine {}({}){}{}{}{}\n",
                    indent,
                    name,
                    params_str,
                    ret_str,
                    req_str,
                    contract_str,
                    state_str
                ));
            } else {
                out.push_str(&format!(
                    "{}routine {}({}){}{}{}{} {{\n",
                    indent,
                    name,
                    params_str,
                    ret_str,
                    req_str,
                    contract_str,
                    state_str
                ));
                for s in body {
                    format_spanned_statement(out, s, config, depth + 1);
                }
                out.push_str(&format!("{}}}\n", indent));
            }
        }
        Statement::TypeDecl {
            name,
            extends,
            fields,
            decay_after_ms,
            auto_drop,
            scoped_branch,
        } => {
            let ext_str = if let Some(parent) = extends {
                format!("{} + ", parent)
            } else {
                String::new()
            };
            let decay_str = if let Some(ms) = decay_after_ms {
                format!(" decay_after {}ms", ms)
            } else {
                String::new()
            };
            let drop_str = if let Some(ad) = auto_drop {
                format!(
                    " auto_drop(\"{}\", \"{}\", {})",
                    ad.lib_name, ad.routine_name, ad.field_name
                )
            } else {
                String::new()
            };
            let scoped_str = if let Some(sb) = scoped_branch {
                format!(" scoped(@{})", sb)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{}type {} = {}struct{}{}{} {{\n",
                indent, name, ext_str, decay_str, drop_str, scoped_str
            ));
            let mut sorted_fields: Vec<_> = fields.iter().collect();
            sorted_fields.sort_by_key(|(fname, _)| fname.as_str());
            let inner_indent = indent_str(config, depth + 1);
            let field_entries: Vec<_> = sorted_fields
                .iter()
                .map(|(fname, fdef)| {
                    let const_prefix = if fdef.is_const { "const " } else { "" };
                    let default_suffix =
                        if let Some(ref def_val) = fdef.default_value {
                            format!(" = {}", format_expr(def_val, config, depth + 1))
                        } else {
                            String::new()
                        };
                    format!(
                        "{}{}{}: {}{}",
                        inner_indent,
                        const_prefix,
                        fname,
                        format_type(&fdef.typ),
                        default_suffix
                    )
                })
                .collect();
            out.push_str(&field_entries.join(",\n"));
            if !field_entries.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::InterfaceDecl {
            name,
            extends,
            methods,
        } => {
            let ext_str = if !extends.is_empty() {
                format!(" = {} + interface", extends.join(" + "))
            } else {
                String::new()
            };
            out.push_str(&format!("{}interface {}{} {{\n", indent, name, ext_str));
            let inner_indent = indent_str(config, depth + 1);
            for m in methods {
                let params_str = m
                    .params
                    .iter()
                    .map(format_param)
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret_str = if let Some(ref rt) = m.return_type {
                    format!(" -> {}", format_type(rt))
                } else {
                    String::new()
                };
                let req_str = if !m.required_capabilities.is_empty() {
                    let caps = m
                        .required_capabilities
                        .iter()
                        .map(|c| {
                            if c.parameters.is_empty() {
                                c.path.clone()
                            } else {
                                let mut sorted_params: Vec<_> =
                                    c.parameters.iter().collect();
                                sorted_params.sort_by_key(|(k, _)| k.as_str());
                                let p_strs = sorted_params
                                    .iter()
                                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                format!("{}[{}]", c.path, p_strs)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(" require {}", caps)
                } else {
                    String::new()
                };
                let taking_str = if let Some(t) = m.taking_ms {
                    format!(" taking {}ms", t)
                } else {
                    String::new()
                };
                let state_str = if let Some((ref v, ref s)) = m.state_constraint {
                    format!(" where {}.state == {}", v, s)
                } else {
                    String::new()
                };
                if let Some(ref b) = m.default_body {
                    out.push_str(&format!(
                        "{}routine {}({}){}{}{}{} {{\n",
                        inner_indent,
                        m.name,
                        params_str,
                        ret_str,
                        req_str,
                        taking_str,
                        state_str
                    ));
                    for s in b {
                        format_spanned_statement(out, s, config, depth + 2);
                    }
                    out.push_str(&format!("{}}}\n", inner_indent));
                } else {
                    out.push_str(&format!(
                        "{}routine {}({}){}{}{}{}\n",
                        inner_indent,
                        m.name,
                        params_str,
                        ret_str,
                        req_str,
                        taking_str,
                        state_str
                    ));
                }
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::If {
            binding,
            condition,
            then_branch,
            else_branch,
            reconcile,
        } => {
            format_if_statement(
                out,
                binding.as_deref(),
                condition,
                then_branch,
                else_branch.as_deref(),
                reconcile.as_ref(),
                config,
                depth,
            );
        }
        Statement::Loop { max_ms, body } => {
            let taking_str = if *max_ms != u64::MAX {
                format!(" taking {}ms", max_ms)
            } else {
                " taking _".to_string()
            };
            out.push_str(&format!("{}loop{} {{\n", indent, taking_str));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::While {
            condition,
            is_valid_check,
            max_ms,
            body,
        } => {
            let valid_str = if *is_valid_check { "valid " } else { "" };
            let taking_str = if *max_ms != u64::MAX {
                format!(" taking {}ms", max_ms)
            } else {
                " taking _".to_string()
            };
            out.push_str(&format!(
                "{}while {}({}){} {{\n",
                indent,
                valid_str,
                format_expr(condition, config, depth),
                taking_str
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::For {
            item_name,
            mode,
            source,
            body,
            pacing_ms,
            max_ms,
        } => {
            let mode_str = match mode {
                ParamMode::Peek => "peek",
                ParamMode::Consume => "consume",
                ParamMode::Clone => "clone",
                ParamMode::Decay => "decay",
                ParamMode::Lease => "lease",
            };
            let pacing_str = if let Some(p) = pacing_ms {
                format!(" pacing taking {}ms", p)
            } else {
                String::new()
            };
            let max_str = match max_ms {
                Some(m) if *m != u64::MAX => format!(" taking {}ms", m),
                _ => String::new(),
            };
            out.push_str(&format!(
                "{}for {} {} {}{}{} {{\n",
                indent, item_name, mode_str, source, pacing_str, max_str
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::ForStep {
            item_name,
            source,
            step_ms,
            body,
        } => {
            let step_str = match step_ms {
                Some(ms) => format!("{}ms", ms),
                None => "_".to_string(),
            };
            out.push_str(&format!(
                "{}for {} in {} step {} {{\n",
                indent,
                item_name,
                format_expr(source, config, depth),
                step_str
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Break => {
            out.push_str(&format!("{}break\n", indent));
        }
        Statement::Return(val) => {
            if let Some(v) = val {
                out.push_str(&format!("{}return {}\n", indent, v));
            } else {
                out.push_str(&format!("{}return\n", indent));
            }
        }
        Statement::Yield(name) => {
            out.push_str(&format!("{}yield {}\n", indent, name));
        }
        Statement::Print(args) => {
            let formatted = args
                .iter()
                .map(|e| format_expr(e, config, depth))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("{}print({})\n", indent, formatted));
        }
        Statement::Debug(expr) => {
            out.push_str(&format!(
                "{}debug({})\n",
                indent,
                format_expr(expr, config, depth)
            ));
        }
        Statement::Expression(expr) => {
            out.push_str(&format!(
                "{}{}\n",
                indent,
                format_expr(expr, config, depth)
            ));
        }
        Statement::Split { parent, branches } => {
            out.push_str(&format!(
                "{}split {} into [{}]\n",
                indent,
                parent,
                branches.join(", ")
            ));
        }
        Statement::Merge {
            branches,
            target,
            resolutions,
        } => {
            let res_str = format_merge_resolution(resolutions, config, depth);
            out.push_str(&format!(
                "{}merge [{}] into {}{}\n",
                indent,
                branches.join(", "),
                target,
                res_str
            ));
        }
        Statement::Anchor(name) => {
            out.push_str(&format!("{}anchor {}\n", indent, name));
        }
        Statement::Rewind(name) => {
            out.push_str(&format!("{}rewind_to({})\n", indent, name));
        }
        Statement::Commit(body) => {
            out.push_str(&format!("{}commit {{\n", indent));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Collapse => {
            out.push_str(&format!("{}collapse\n", indent));
        }
        Statement::SpeculationMode(mode) => {
            let mode_str = match mode {
                causm_core::SpeculationCommitMode::Full => "full",
                causm_core::SpeculationCommitMode::Selective => "selective",
            };
            out.push_str(&format!("{}speculation_mode({})\n", indent, mode_str));
        }
        Statement::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            let taking_str = if *max_ms != u64::MAX {
                format!(" (taking {}ms)", max_ms)
            } else {
                String::new()
            };
            out.push_str(&format!("{}speculate{} {{\n", indent, taking_str));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            if let Some(fb) = fallback {
                out.push_str(&format!("{}}} fallback {{\n", indent));
                for s in fb {
                    format_spanned_statement(out, s, config, depth + 1);
                }
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Lease {
            binding,
            source,
            duration_ms,
            body,
            reconcile,
        } => {
            let rec_str = reconcile
                .as_ref()
                .map(|r| format_merge_resolution(r, config, depth))
                .unwrap_or_default();
            out.push_str(&format!(
                "{}lease {} = {} taking {}ms {{\n",
                indent, binding, source, duration_ms
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::EnumDecl { name, variants } => {
            out.push_str(&format!("{}enum {} {{\n", indent, name));
            let inner_indent = indent_str(config, depth + 1);
            let v_strs: Vec<_> = variants
                .iter()
                .map(|v| {
                    if v.payload_types.is_empty() {
                        format!("{}{}", inner_indent, v.name)
                    } else {
                        let t_strs = v
                            .payload_types
                            .iter()
                            .map(format_type)
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("{}{}({})", inner_indent, v.name, t_strs)
                    }
                })
                .collect();
            out.push_str(&v_strs.join(",\n"));
            if !v_strs.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::SplitMap {
            item_name,
            mode,
            source,
            body,
            reconcile,
        } => {
            let mode_str = match mode {
                ParamMode::Peek => "peek",
                ParamMode::Consume => "consume",
                ParamMode::Clone => "clone",
                ParamMode::Decay => "decay",
                ParamMode::Lease => "lease",
            };
            let rec_str = reconcile
                .as_ref()
                .map(|r| format_merge_resolution(r, config, depth))
                .unwrap_or_default();
            out.push_str(&format!(
                "{}split_map {} {} {} {{\n",
                indent, item_name, mode_str, source
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::Import { path, alias } => {
            if let Some(a) = alias {
                out.push_str(&format!("{}import \"{}\" as {}\n", indent, path, a));
            } else {
                out.push_str(&format!("{}import \"{}\"\n", indent, path));
            }
        }
        Statement::Isolate(iso) => {
            let name_str = iso.name.as_deref().unwrap_or("");
            let name_display = if name_str.is_empty() {
                String::new()
            } else {
                format!(" {}", name_str)
            };
            out.push_str(&format!("{}isolate{} {{\n", indent, name_display));
            let inner_indent = indent_str(config, depth + 1);
            if let Some(cpu) = iso.manifest.cpu_budget_ms {
                out.push_str(&format!("{}enable cpu({}ms)\n", inner_indent, cpu));
            }
            if let Some(mem) = iso.manifest.memory_budget_bytes {
                out.push_str(&format!(
                    "{}enable memory({}bytes)\n",
                    inner_indent, mem
                ));
            }
            if let Some(sl) = iso.manifest.slice_ms {
                out.push_str(&format!("{}slice {}ms\n", inner_indent, sl));
            }
            let mut budgets: Vec<_> = iso.manifest.resource_budgets.iter().collect();
            budgets.sort_by_key(|(res, _)| res.as_str());
            for (res, amt) in budgets {
                out.push_str(&format!("{}enable {}({})\n", inner_indent, res, amt));
            }
            for cap in &iso.manifest.capabilities {
                if cap.parameters.is_empty() {
                    out.push_str(&format!("{}require {}\n", inner_indent, cap.path));
                } else {
                    let mut sorted_params: Vec<_> = cap.parameters.iter().collect();
                    sorted_params.sort_by_key(|(k, _)| k.as_str());
                    let p_strs = sorted_params
                        .iter()
                        .map(|(k, v)| format!("{} = \"{}\"", k, v))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!(
                        "{}require {}({})\n",
                        inner_indent, cap.path, p_strs
                    ));
                }
            }
            for s in &iso.body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::MatchEntropy {
            target,
            valid_branch,
            decayed_branch,
            pending_branch,
            consumed_branch,
        } => {
            out.push_str(&format!(
                "{}match entropy({}) {{\n",
                indent,
                format_expr(target, config, depth)
            ));
            let inner_indent = indent_str(config, depth + 1);
            if let Some((pat, guard, body)) = valid_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g, config, depth + 1)))
                    .unwrap_or_default();
                let pat_str = format_decayed_pattern(pat, config, depth + 1);
                let pat_display = if pat_str.is_empty() {
                    String::new()
                } else {
                    format!("({})", pat_str)
                };
                out.push_str(&format!(
                    "{}Valid{}{}: {{\n",
                    inner_indent, pat_display, g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            if let Some((pat, guard, body)) = decayed_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g, config, depth + 1)))
                    .unwrap_or_default();
                let pat_str = format_decayed_pattern(pat, config, depth + 1);
                let pat_display = if pat_str.is_empty() {
                    String::new()
                } else {
                    format!("({})", pat_str)
                };
                out.push_str(&format!(
                    "{}Decayed{}{}: {{\n",
                    inner_indent, pat_display, g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            if let Some((pat, guard, body)) = pending_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g, config, depth + 1)))
                    .unwrap_or_default();
                let pat_str = format_decayed_pattern(pat, config, depth + 1);
                let pat_display = if pat_str.is_empty() {
                    String::new()
                } else {
                    format!("({})", pat_str)
                };
                out.push_str(&format!(
                    "{}Pending{}{}: {{\n",
                    inner_indent, pat_display, g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            if let Some((guard, body)) = consumed_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g, config, depth + 1)))
                    .unwrap_or_default();
                out.push_str(&format!("{}Consumed{}: {{\n", inner_indent, g_str));
                for s in body {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Match { target, arms } => {
            out.push_str(&format!(
                "{}match {} {{\n",
                indent,
                format_expr(target, config, depth)
            ));
            let inner_indent = indent_str(config, depth + 1);
            for arm in arms {
                let g_str = arm
                    .guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g, config, depth + 1)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{}{}{} => {{\n",
                    inner_indent,
                    format_pattern(&arm.pattern, config, depth + 1),
                    g_str
                ));
                for s in &arm.body {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::IfLet {
            pattern,
            expr,
            then_branch,
            else_branch,
            reconcile,
        } => {
            let rec_str = reconcile
                .as_ref()
                .map(|r| format_merge_resolution(r, config, depth))
                .unwrap_or_default();
            out.push_str(&format!(
                "{}if let {} = {} {{\n",
                indent,
                format_pattern(pattern, config, depth),
                format_expr(expr, config, depth)
            ));
            for s in then_branch {
                format_spanned_statement(out, s, config, depth + 1);
            }
            if let Some(eb) = else_branch {
                out.push_str(&format!("{}}} else {{\n", indent));
                for s in eb {
                    format_spanned_statement(out, s, config, depth + 1);
                }
            }
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::RelativisticBlock { time, body } => {
            let header = match time {
                causm_core::TimeCoordinate::Global(t) => format!("@{}ms", t),
                causm_core::TimeCoordinate::Relative(t) => format!("@+{}ms", t),
                causm_core::TimeCoordinate::Branch(b) => format!("@{}", b),
                causm_core::TimeCoordinate::Periodic(t) => format!("@every {}ms", t),
            };
            out.push_str(&format!("{}{}: {{\n", indent, header));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Await(id) => {
            out.push_str(&format!("{}await({})\n", indent, id));
        }
        Statement::AssertTime {
            operator,
            limit_ms,
            fallback,
        } => {
            let op_str = match operator {
                BinaryOperator::Eq => "==",
                BinaryOperator::Neq => "!=",
                BinaryOperator::Lt => "<",
                BinaryOperator::Gt => ">",
                BinaryOperator::Le => "<=",
                BinaryOperator::Ge => ">=",
                _ => "<=",
            };
            if let Some(fb) = fallback {
                out.push_str(&format!(
                    "{}assert_time(elapsed {} {}ms) else {{\n",
                    indent, op_str, limit_ms
                ));
                for s in fb {
                    format_spanned_statement(out, s, config, depth + 1);
                }
                out.push_str(&format!("{}}}\n", indent));
            } else {
                out.push_str(&format!(
                    "{}assert_time(elapsed {} {}ms)\n",
                    indent, op_str, limit_ms
                ));
            }
        }
        Statement::Capability(cap) => {
            if cap.parameters.is_empty() {
                out.push_str(&format!("{}require {}\n", indent, cap.path));
            } else {
                let mut sorted_params: Vec<_> = cap.parameters.iter().collect();
                sorted_params.sort_by_key(|(k, _)| k.as_str());
                let p_strs = sorted_params
                    .iter()
                    .map(|(k, v)| format!("{} = \"{}\"", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "{}require {}({})\n",
                    indent, cap.path, p_strs
                ));
            }
        }
        Statement::Select {
            max_ms,
            cases,
            timeout,
            reconcile,
        } => {
            let rec_str = reconcile
                .as_ref()
                .map(|r| format_merge_resolution(r, config, depth))
                .unwrap_or_default();
            let taking_str = if *max_ms != u64::MAX {
                format!(" (taking {}ms)", max_ms)
            } else {
                String::new()
            };
            out.push_str(&format!("{}select{} {{\n", indent, taking_str));
            let inner_indent = indent_str(config, depth + 1);
            for case in cases {
                out.push_str(&format!(
                    "{}case {} = {}: {{\n",
                    inner_indent,
                    case.binding,
                    format_expr(&case.source, config, depth + 1)
                ));
                for s in &case.body {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            if let Some(to) = timeout {
                out.push_str(&format!("{}timeout: {{\n", inner_indent));
                for s in to {
                    format_spanned_statement(out, s, config, depth + 2);
                }
                out.push_str(&format!("{}}}\n", inner_indent));
            }
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::LoopTick { body } => {
            out.push_str(&format!("{}loop tick {{\n", indent));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::DirectiveBlock { directives, body } => {
            let dir_strs = directives
                .iter()
                .map(|d| match d {
                    causm_core::BlockDirective::NoZ3 => "@no_z3",
                    causm_core::BlockDirective::Chaos => "@chaos",
                    causm_core::BlockDirective::Deterministic => "@deterministic",
                })
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!("{}{}: {{\n", indent, dir_strs));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::DecayHandler { type_name, body } => {
            out.push_str(&format!("{}decay_handler for {} {{\n", indent, type_name));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::FromImport { path, symbols } => {
            let sym_strs = symbols
                .iter()
                .map(|(sym, alias)| {
                    if let Some(a) = alias {
                        format!("{} as {}", sym, a)
                    } else {
                        sym.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "{}from \"{}\" import {}\n",
                indent, path, sym_strs
            ));
        }
        Statement::ForeignBlock {
            lib_name,
            abi,
            routines,
        } => {
            out.push_str(&format!(
                "{}foreign \"{}\" abi(\"{}\") {{\n",
                indent, lib_name, abi
            ));
            for s in routines {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Slice { milliseconds } => {
            out.push_str(&format!("{}slice {}ms\n", indent, milliseconds));
        }
        Statement::FieldUpdate {
            target,
            field,
            value,
        } => {
            if field.is_empty() {
                // Bare reassignment: `signal = expr` (no `let`) — field_update_stmt grammar
                out.push_str(&format!(
                    "{}{} = {}\n",
                    indent,
                    format_expr(target, config, depth),
                    format_expr(value, config, depth)
                ));
            } else {
                out.push_str(&format!(
                    "{}{}.{} = {}\n",
                    indent,
                    format_expr(target, config, depth),
                    field,
                    format_expr(value, config, depth)
                ));
            }
        }
        Statement::StateDecl {
            target,
            var_type,
            expr,
        } => {
            let type_str = if let Some(t) = var_type {
                format!(": {}", format_type(t))
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{}state {}{} = {}\n",
                indent,
                target,
                type_str,
                format_expr(expr, config, depth)
            ));
        }
        Statement::PolicyStmt { target, policy } => {
            let target_str = match target {
                causm_core::PolicyTarget::OnFull => "on_full",
                causm_core::PolicyTarget::OnDeadlineBreach => "on_deadline_breach",
                causm_core::PolicyTarget::OnOverflow => "on_overflow",
            };
            let policy_str = match policy {
                causm_core::SaturationPolicy::EvictDecayed => "EvictDecayed",
                causm_core::SaturationPolicy::RingBuffer => "RingBuffer",
                causm_core::SaturationPolicy::Throttle => "Throttle",
                causm_core::SaturationPolicy::FailFast => "FailFast",
            };
            out.push_str(&format!(
                "{}policy {} = {}\n",
                indent, target_str, policy_str
            ));
        }
        Statement::LoopOn { target, body } => {
            out.push_str(&format!(
                "{}loop on {} {{\n",
                indent,
                format_expr(target, config, depth)
            ));
            for s in body {
                format_spanned_statement(out, s, config, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Entangle { variables } => {
            out.push_str(&format!("{}entangle({})\n", indent, variables.join(", ")));
        }
        Statement::Send {
            value_id,
            target_branch,
        } => {
            out.push_str(&format!(
                "{}send({}, @{})\n",
                indent, value_id, target_branch
            ));
        }
    }
}

fn format_decayed_pattern(
    pat: &causm_core::DecayedPattern,
    config: &FormatConfig,
    depth: usize,
) -> String {
    match pat {
        causm_core::DecayedPattern::Binding(b) => b.clone(),
        causm_core::DecayedPattern::Fields(f) => {
            let mut sorted: Vec<_> = f.iter().collect();
            sorted.sort_by_key(|(k, _)| k.as_str());
            let f_strs: Vec<_> = sorted
                .iter()
                .map(|(k, v)| match v {
                    causm_core::PatternValue::State(s) => format!("{} = {}", k, s),
                    causm_core::PatternValue::Expr(e) => {
                        format!("{} = {}", k, format_expr(e, config, depth))
                    }
                })
                .collect();
            format!("{{ {} }}", f_strs.join(", "))
        }
    }
}

fn format_merge_resolution(
    res: &MergeResolution,
    _config: &FormatConfig,
    _depth: usize,
) -> String {
    if res.auto {
        return " reconcile auto".to_string();
    }
    if !res.rules.is_empty() {
        let mut sorted_rules: Vec<_> = res.rules.iter().collect();
        sorted_rules.sort_by_key(|(k, _)| k.as_str());
        let entries = sorted_rules
            .iter()
            .map(|(k, v)| {
                let strat_str = match v {
                    causm_core::ResolutionStrategy::FirstWins => {
                        "first_wins".to_string()
                    }
                    causm_core::ResolutionStrategy::Auto => "auto".to_string(),
                    causm_core::ResolutionStrategy::Decay => "decay".to_string(),
                    causm_core::ResolutionStrategy::Priority(p) => {
                        format!("priority({})", p)
                    }
                    causm_core::ResolutionStrategy::Custom(c) => c.clone(),
                    causm_core::ResolutionStrategy::TopologyUnion {
                        key_rules,
                        default: _,
                        on_invalid: _,
                    } => {
                        let mut sub_keys: Vec<_> = key_rules.iter().collect();
                        sub_keys.sort_by_key(|(sk, _)| sk.as_str());
                        let sub_entries = sub_keys
                            .iter()
                            .map(|(sk, _)| format!("{}=first_wins", sk))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("topology_union {{ {} }}", sub_entries)
                    }
                    causm_core::ResolutionStrategy::TopologyIntersect {
                        key_rules,
                        default: _,
                        on_invalid: _,
                    } => {
                        let mut sub_keys: Vec<_> = key_rules.iter().collect();
                        sub_keys.sort_by_key(|(sk, _)| sk.as_str());
                        let sub_entries = sub_keys
                            .iter()
                            .map(|(sk, _)| format!("{}=first_wins", sk))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("topology_intersect {{ {} }}", sub_entries)
                    }
                };
                format!("{}={}", k, strat_str)
            })
            .collect::<Vec<_>>()
            .join(", ");
        return format!(" reconcile({})", entries);
    }
    String::new()
}

fn format_param(param: &ParamDecl) -> String {
    let mode = match param.mode {
        ParamMode::Peek => "peek ",
        ParamMode::Consume => "consume ",
        ParamMode::Clone => "clone ",
        ParamMode::Decay => "decay ",
        ParamMode::Lease => "lease ",
    };
    let type_str = if let Some(ref t) = param.typ {
        format!(": {}", format_type(t))
    } else {
        String::new()
    };
    format!("{}{}{}", mode, param.name, type_str)
}

fn format_type(typ: &TypeName) -> String {
    match typ {
        TypeName::Builtin(b) => match b {
            causm_core::BuiltinType::Integer => "int".to_string(),
            causm_core::BuiltinType::I8 => "i8".to_string(),
            causm_core::BuiltinType::I16 => "i16".to_string(),
            causm_core::BuiltinType::I32 => "i32".to_string(),
            causm_core::BuiltinType::I64 => "i64".to_string(),
            causm_core::BuiltinType::U8 => "u8".to_string(),
            causm_core::BuiltinType::U16 => "u16".to_string(),
            causm_core::BuiltinType::U32 => "u32".to_string(),
            causm_core::BuiltinType::U64 => "u64".to_string(),
            causm_core::BuiltinType::Float => "float".to_string(),
            causm_core::BuiltinType::F32 => "f32".to_string(),
            causm_core::BuiltinType::F64 => "f64".to_string(),
            causm_core::BuiltinType::Bool => "bool".to_string(),
            causm_core::BuiltinType::String => "string".to_string(),
            causm_core::BuiltinType::Struct => "struct".to_string(),
            causm_core::BuiltinType::Topology => "topology".to_string(),
            causm_core::BuiltinType::Array => "array".to_string(),
        },
        TypeName::Custom(c) => c.clone(),
        TypeName::Generic(name, params) => {
            if (name == "array" || name == "Array") && params.len() == 1 {
                if let causm_core::TypeParam::Type(ref t) = params[0] {
                    return format!("[{}]", format_type(t));
                }
            }
            let params_str = params
                .iter()
                .map(|p| match p {
                    TypeParam::Type(t) => format_type(t),
                    TypeParam::Amount(a) => a.to_string(),
                    TypeParam::Duration(d) => format!("{}ms", d),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{}>", name, params_str)
        }
        TypeName::Optional(inner) => format!("{}?", format_type(inner)),
        TypeName::Union(variants) => variants
            .iter()
            .map(format_type)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

fn format_expr(expr: &Expression, config: &FormatConfig, depth: usize) -> String {
    match expr {
        Expression::Integer(i) => i.to_string(),
        Expression::Float(bits) => {
            let f = f64::from_bits(*bits);
            let s = f.to_string();
            if !s.contains('.') && !s.contains('e') && !s.contains('E') {
                format!("{}.0", s)
            } else {
                s
            }
        }
        Expression::Boolean(b) => b.to_string(),
        Expression::Literal(s) => format_string_literal(s),
        Expression::Identifier(id) => id.clone(),
        Expression::Null => "null".to_string(),
        Expression::Call { routine, args } => {
            let args_str = args
                .iter()
                .map(|e| format_expr(e, config, depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", routine, args_str)
        }
        Expression::MethodCall {
            target,
            method,
            args,
            ..
        } => {
            let args_str = args
                .iter()
                .map(|e| format_expr(e, config, depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}.{}({})",
                format_expr(target, config, depth),
                method,
                args_str
            )
        }
        Expression::FieldAccess { target, field } => {
            format!("{}.{}", format_expr(target, config, depth), field)
        }
        Expression::BinaryOp { left, op, right } => {
            let prec = op_precedence(op);
            let left_str = format_sub_expr(left, prec, false, config, depth);
            let right_str = format_sub_expr(right, prec, true, config, depth);
            let op_str = match op {
                BinaryOperator::Add => "+",
                BinaryOperator::Sub => "-",
                BinaryOperator::Mul => "*",
                BinaryOperator::Div => "/",
                BinaryOperator::Rem => "%",
                BinaryOperator::Pow => "^",
                BinaryOperator::Eq => "==",
                BinaryOperator::Neq => "!=",
                BinaryOperator::Lt => "<",
                BinaryOperator::Gt => ">",
                BinaryOperator::Le => "<=",
                BinaryOperator::Ge => ">=",
                BinaryOperator::LogicalAnd => "&&",
                BinaryOperator::LogicalOr => "||",
            };
            format!("{} {} {}", left_str, op_str, right_str)
        }
        Expression::UnaryOp { op, expr } => {
            let op_str = match op {
                UnaryOperator::Neg => "-",
                UnaryOperator::Not => "!",
            };
            format!("{}{}", op_str, format_expr(expr, config, depth))
        }
        Expression::StructLit(type_opt, fields) => {
            let type_str = if let Some(t) = type_opt.borrow().as_ref() {
                format!("{}: ", t)
            } else {
                String::new()
            };
            if fields.is_empty() {
                format!("{}struct {{}}", type_str)
            } else if fields.len() <= 3 {
                let mut sorted_fields: Vec<_> = fields.iter().collect();
                sorted_fields.sort_by_key(|(k, _)| k.as_str());
                let f_strs = sorted_fields
                    .iter()
                    .map(|(k, v)| {
                        format!("{} = {}", k, format_expr(v, config, depth))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let inline = format!("{}struct {{ {} }}", type_str, f_strs);
                if inline.len() <= 60 {
                    inline
                } else {
                    format_multiline_struct(&type_str, sorted_fields, config, depth)
                }
            } else {
                let mut sorted_fields: Vec<_> = fields.iter().collect();
                sorted_fields.sort_by_key(|(k, _)| k.as_str());
                format_multiline_struct(&type_str, sorted_fields, config, depth)
            }
        }
        Expression::CloneOp(id) => format!("clone({})", id),
        Expression::StrBytes(expr) => {
            format!("str_bytes({})", format_expr(expr, config, depth))
        }
        Expression::ToStr(expr) => {
            format!("to_str({})", format_expr(expr, config, depth))
        }
        Expression::Len(expr) => {
            format!("len({})", format_expr(expr, config, depth))
        }
        Expression::RefOp(inner) => {
            format!("&{}", format_expr(inner, config, depth))
        }
        Expression::TryUnwrap(inner) => {
            format!("{}?", format_expr(inner, config, depth))
        }
        Expression::ArrayLiteral(elements) => {
            let elems_str = elements
                .iter()
                .map(|e| format_expr(e, config, depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", elems_str)
        }
        Expression::ArrayRepeat { value, count } => {
            format!(
                "[{}; {}]",
                format_expr(value, config, depth),
                format_expr(count, config, depth)
            )
        }
        Expression::ArraySlice {
            target,
            start,
            end,
            inclusive,
        } => {
            let s_str = start
                .as_ref()
                .map(|s| format_expr(s, config, depth))
                .unwrap_or_default();
            let dot_str = if *inclusive { "..=" } else { ".." };
            let e_str = end
                .as_ref()
                .map(|e| format_expr(e, config, depth))
                .unwrap_or_default();
            format!(
                "{}[{}{}{}]",
                format_expr(target, config, depth),
                s_str,
                dot_str,
                e_str
            )
        }
        Expression::IndexAccess { target, index } => {
            format!(
                "{}[{}]",
                format_expr(target, config, depth),
                format_expr(index, config, depth)
            )
        }
        Expression::ChannelReceive(chan) => format!("chan_recv({})", chan),
        Expression::TopologyLit(fields) => {
            let mut sorted_fields: Vec<_> = fields.iter().collect();
            sorted_fields.sort_by_key(|(k, _)| k.as_str());
            let mut f_strs = Vec::new();
            for (k, v) in sorted_fields {
                f_strs.push(format!("{} = {}", k, format_expr(v, config, depth)));
            }
            format!("topology {{ {} }}", f_strs.join(", "))
        }
        Expression::TypeCast { expr, target_type } => {
            format!(
                "{} as {}",
                format_expr(expr, config, depth),
                format_type(target_type)
            )
        }
        Expression::TypeAssertion { target, cast_type } => {
            format!(
                "{}.({})",
                format_expr(target, config, depth),
                format_type(cast_type)
            )
        }
        Expression::Syscall {
            target,
            args,
            duration_ms,
        } => {
            let dur_str = if let Some(d) = duration_ms {
                format!(" taking {}ms", d)
            } else {
                String::new()
            };
            let args_str = args
                .iter()
                .map(|e| format_expr(e, config, depth))
                .collect::<Vec<_>>()
                .join(", ");
            let target_str = match target {
                SyscallTarget::Number(n) => n.to_string(),
                SyscallTarget::Symbol(s) => format!("\"{}\"", s),
            };
            format!(
                "syscall({}{}){}",
                target_str,
                if args_str.is_empty() {
                    "".to_string()
                } else {
                    format!(", {}", args_str)
                },
                dur_str
            )
        }
        Expression::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => {
            if args.is_empty() {
                format!("{}::{}", enum_name, variant_name)
            } else {
                let args_str = args
                    .iter()
                    .map(|e| format_expr(e, config, depth))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}::{}({})", enum_name, variant_name, args_str)
            }
        }
        Expression::Deferred {
            capability,
            params,
            deadline_ms,
        } => {
            let mut sorted_params: Vec<_> = params.iter().collect();
            sorted_params.sort_by_key(|(k, _)| k.as_str());
            let p_strs = sorted_params
                .iter()
                .map(|(k, v)| format!("{} = \"{}\"", k, v))
                .collect::<Vec<_>>()
                .join(", ");
            format!("defer {}({}) taking {}ms", capability, p_strs, deadline_ms)
        }
        Expression::FString(parts) => {
            let mut s = "f\"".to_string();
            for part in parts {
                match part {
                    FStringPart::Text(t) => {
                        for c in t.chars() {
                            match c {
                                '"' => s.push_str("\\\""),
                                '\\' => s.push_str("\\\\"),
                                '\n' => s.push_str("\\n"),
                                '\r' => s.push_str("\\r"),
                                '\t' => s.push_str("\\t"),
                                other => s.push(other),
                            }
                        }
                    }
                    FStringPart::Expr(e) => {
                        s.push('{');
                        s.push_str(&format_expr(e, config, depth));
                        s.push('}');
                    }
                }
            }
            s.push('"');
            s
        }
        Expression::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond_str = format_expr(condition, config, depth);
            let then_str = format_expr(then_branch, config, depth);
            let else_str = format_expr(else_branch, config, depth);
            let inline = format!(
                "if ({}) {{ {} }} else {{ {} }}",
                cond_str, then_str, else_str
            );
            if inline.len() <= 60 {
                inline
            } else {
                let inner_indent = indent_str(config, depth + 1);
                let outer_indent = indent_str(config, depth);
                match &**else_branch {
                    Expression::If { .. } => {
                        format!(
                            "if ({}) {{\n{}{}\n{}}} else {}",
                            cond_str,
                            inner_indent,
                            then_str,
                            outer_indent,
                            format_expr(else_branch, config, depth)
                        )
                    }
                    _ => {
                        format!(
                            "if ({}) {{\n{}{}\n{}}} else {{\n{}{}\n{}}}",
                            cond_str,
                            inner_indent,
                            then_str,
                            outer_indent,
                            inner_indent,
                            else_str,
                            outer_indent
                        )
                    }
                }
            }
        }
        Expression::Match { target, arms } => {
            if arms.len() <= 1 {
                let arm_strs: Vec<String> = arms
                    .iter()
                    .map(|a| {
                        let g_str = a
                            .guard
                            .as_ref()
                            .map(|g| {
                                format!(" if {}", format_expr(g, config, depth))
                            })
                            .unwrap_or_default();
                        format!(
                            "{}{} => {}",
                            format_pattern(&a.pattern, config, depth),
                            g_str,
                            format_expr(&a.body, config, depth)
                        )
                    })
                    .collect();
                format!(
                    "match {} {{ {} }}",
                    format_expr(target, config, depth),
                    arm_strs.join(", ")
                )
            } else {
                let inner_indent = indent_str(config, depth + 1);
                let outer_indent = indent_str(config, depth);
                let arm_lines: Vec<String> = arms
                    .iter()
                    .map(|a| {
                        let g_str = a
                            .guard
                            .as_ref()
                            .map(|g| {
                                format!(" if {}", format_expr(g, config, depth + 1))
                            })
                            .unwrap_or_default();
                        format!(
                            "{}{}{} => {},",
                            inner_indent,
                            format_pattern(&a.pattern, config, depth + 1),
                            g_str,
                            format_expr(&a.body, config, depth + 1)
                        )
                    })
                    .collect();
                format!(
                    "match {} {{\n{}\n{}}}",
                    format_expr(target, config, depth),
                    arm_lines.join("\n"),
                    outer_indent
                )
            }
        }
        Expression::ArenaIntrospect(kind) => {
            let kind_str = match kind {
                causm_core::ArenaIntrospect::Remaining => "remaining",
                causm_core::ArenaIntrospect::UsedBytes => "used_bytes",
                causm_core::ArenaIntrospect::Capacity => "capacity",
            };
            format!("arena.{}()", kind_str)
        }
        Expression::CapabilityCheck(cap) => {
            if cap.parameters.is_empty() {
                format!("capability({})", cap.path)
            } else {
                let mut sorted_params: Vec<_> = cap.parameters.iter().collect();
                sorted_params.sort_by_key(|(k, _)| k.as_str());
                let p_strs = sorted_params
                    .iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("capability({}[{}])", cap.path, p_strs)
            }
        }
        Expression::Turbofish { expr, type_args } => {
            let t_strs: Vec<String> = type_args
                .iter()
                .map(|t| match t {
                    TypeParam::Type(tn) => format_type(tn),
                    TypeParam::Amount(a) => a.to_string(),
                    TypeParam::Duration(d) => format!("{}ms", d),
                })
                .collect();
            format!(
                "{}::<{}>",
                format_expr(expr, config, depth),
                t_strs.join(", ")
            )
        }
        Expression::GenericStaticCall {
            type_name,
            type_args,
            method,
            args,
        } => {
            let t_strs: Vec<String> = type_args
                .iter()
                .map(|t| match t {
                    TypeParam::Type(tn) => format_type(tn),
                    TypeParam::Amount(a) => a.to_string(),
                    TypeParam::Duration(d) => format!("{}ms", d),
                })
                .collect();
            let args_str = args
                .iter()
                .map(|e| format_expr(e, config, depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}<{}>::{}({})",
                type_name,
                t_strs.join(", "),
                method,
                args_str
            )
        }
    }
}

fn format_multiline_struct(
    type_str: &str,
    sorted_fields: Vec<(&String, &Expression)>,
    config: &FormatConfig,
    depth: usize,
) -> String {
    let inner_indent = indent_str(config, depth + 1);
    let outer_indent = indent_str(config, depth);
    let mut f_lines = Vec::new();
    for (k, v) in sorted_fields {
        f_lines.push(format!(
            "{}{} = {}",
            inner_indent,
            k,
            format_expr(v, config, depth + 1)
        ));
    }
    format!(
        "{}struct {{\n{}\n{}}}",
        type_str,
        f_lines.join(",\n"),
        outer_indent
    )
}

pub fn format_pattern(pat: &Pattern, config: &FormatConfig, depth: usize) -> String {
    match pat {
        Pattern::Wildcard => "_".to_string(),
        Pattern::Identifier(id) => id.clone(),
        Pattern::Literal(e) => format_expr(e, config, depth),
        Pattern::EnumVariant {
            enum_name,
            variant_name,
            args,
        } => {
            let prefix = if let Some(e) = enum_name {
                format!("{}::{}", e, variant_name)
            } else {
                variant_name.clone()
            };
            if args.is_empty() {
                prefix
            } else {
                let args_str = args
                    .iter()
                    .map(|p| format_pattern(p, config, depth))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", prefix, args_str)
            }
        }
        Pattern::TypeAssert {
            binding,
            target_type,
        } => {
            format!("{} as {}", binding, format_type(target_type))
        }
    }
}

fn op_precedence(op: &BinaryOperator) -> u8 {
    match op {
        BinaryOperator::Pow => 60,
        BinaryOperator::Mul | BinaryOperator::Div | BinaryOperator::Rem => 50,
        BinaryOperator::Add | BinaryOperator::Sub => 40,
        BinaryOperator::Lt
        | BinaryOperator::Gt
        | BinaryOperator::Le
        | BinaryOperator::Ge => 30,
        BinaryOperator::Eq | BinaryOperator::Neq => 20,
        BinaryOperator::LogicalAnd => 10,
        BinaryOperator::LogicalOr => 5,
    }
}

fn format_sub_expr(
    expr: &Expression,
    parent_prec: u8,
    is_right: bool,
    config: &FormatConfig,
    depth: usize,
) -> String {
    let s = format_expr(expr, config, depth);
    match expr {
        Expression::BinaryOp { op, .. } => {
            let prec = op_precedence(op);
            if prec < parent_prec
                || (prec == parent_prec
                    && (is_right
                        || *op == BinaryOperator::Rem
                        || *op == BinaryOperator::Div
                        || *op == BinaryOperator::Sub))
            {
                return format!("({})", s);
            }
        }
        Expression::If { .. } | Expression::Match { .. } => {
            return format!("({})", s);
        }
        _ => {}
    }
    s
}

fn format_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[allow(clippy::too_many_arguments)]
fn format_if_statement(
    out: &mut String,
    binding: Option<&str>,
    condition: &Expression,
    then_branch: &[SpannedStatement],
    else_branch: Option<&[SpannedStatement]>,
    reconcile: Option<&MergeResolution>,
    config: &FormatConfig,
    depth: usize,
) {
    let indent = indent_str(config, depth);
    let if_head = if let Some(b) = binding {
        format!("if let {} = {}", b, format_expr(condition, config, depth))
    } else {
        format!("if ({})", format_expr(condition, config, depth))
    };
    out.push_str(&format!("{}{} {{\n", indent, if_head));
    for s in then_branch {
        format_spanned_statement(out, s, config, depth + 1);
    }
    if let Some(eb) = else_branch {
        if eb.len() == 1 {
            if let Statement::If {
                binding: next_binding,
                condition: next_cond,
                then_branch: next_then,
                else_branch: next_else,
                reconcile: next_rec,
            } = &eb[0].stmt
            {
                out.push_str(&format!("{}}} else ", indent));
                let next_head = if let Some(b) = next_binding {
                    format!(
                        "if let {} = {}",
                        b,
                        format_expr(next_cond, config, depth)
                    )
                } else {
                    format!("if ({})", format_expr(next_cond, config, depth))
                };
                out.push_str(&format!("{} {{\n", next_head));
                for s in next_then {
                    format_spanned_statement(out, s, config, depth + 1);
                }
                if let Some(n_eb) = next_else {
                    format_else_chain(
                        out,
                        n_eb,
                        next_rec.as_ref().or(reconcile),
                        config,
                        depth,
                    );
                    return;
                } else {
                    let rec_str = next_rec
                        .as_ref()
                        .or(reconcile)
                        .map(|r| format_merge_resolution(r, config, depth))
                        .unwrap_or_default();
                    out.push_str(&format!("{}}}{}\n", indent, rec_str));
                    return;
                }
            }
        }
        out.push_str(&format!("{}}} else {{\n", indent));
        for s in eb {
            format_spanned_statement(out, s, config, depth + 1);
        }
    }
    let rec_str = reconcile
        .map(|r| format_merge_resolution(r, config, depth))
        .unwrap_or_default();
    out.push_str(&format!("{}}}{}\n", indent, rec_str));
}

fn format_else_chain(
    out: &mut String,
    eb: &[SpannedStatement],
    reconcile: Option<&MergeResolution>,
    config: &FormatConfig,
    depth: usize,
) {
    let indent = indent_str(config, depth);
    if eb.len() == 1 {
        if let Statement::If {
            binding: next_binding,
            condition: next_cond,
            then_branch: next_then,
            else_branch: next_else,
            reconcile: next_rec,
        } = &eb[0].stmt
        {
            out.push_str(&format!("{}}} else ", indent));
            let next_head = if let Some(b) = next_binding {
                format!("if let {} = {}", b, format_expr(next_cond, config, depth))
            } else {
                format!("if ({})", format_expr(next_cond, config, depth))
            };
            out.push_str(&format!("{} {{\n", next_head));
            for s in next_then {
                format_spanned_statement(out, s, config, depth + 1);
            }
            if let Some(n_eb) = next_else {
                format_else_chain(
                    out,
                    n_eb,
                    next_rec.as_ref().or(reconcile),
                    config,
                    depth,
                );
                return;
            } else {
                let rec_str = next_rec
                    .as_ref()
                    .or(reconcile)
                    .map(|r| format_merge_resolution(r, config, depth))
                    .unwrap_or_default();
                out.push_str(&format!("{}}}{}\n", indent, rec_str));
                return;
            }
        }
    }
    out.push_str(&format!("{}}} else {{\n", indent));
    for s in eb {
        format_spanned_statement(out, s, config, depth + 1);
    }
    let rec_str = reconcile
        .map(|r| format_merge_resolution(r, config, depth))
        .unwrap_or_default();
    out.push_str(&format!("{}}}{}\n", indent, rec_str));
}

#[cfg(test)]
mod tests {
    use super::*;
    use causm_frontend::parser::parse_causm;

    fn check_roundtrip(source: &str) {
        let config = FormatConfig::default();
        let program = parse_causm(source).expect("source should parse");
        let formatted = format_program(&program, &config);
        let reparsed =
            parse_causm(&formatted).expect("formatted output should parse");
        assert!(
            causm_core::programs_ast_eq(&program, &reparsed),
            "AST mismatch after formatting:\n--- Original ---\n{}\n--- Formatted ---\n{}",
            source,
            formatted
        );
    }

    #[test]
    fn test_fmt_basic_timeline_and_let() {
        let code = "@0ms: {\n    let mut x: i64 = 42\n    let y = 10\n}\n";
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_nested_struct_and_expressions() {
        let code = r#"@0ms: {
    routine calc(peek a: i64, peek b: i64) -> i64 taking 5ms {
        let s = struct { x = a + b, y = a * b }
        s.x + s.y
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_match_and_if_let() {
        let code = r#"@0ms: {
    routine evaluate(peek opt: Option) -> i64 taking 2ms {
        if let Option::Some(val) = opt {
            val
        } else {
            0
        }
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_entropy_match_and_lease() {
        let code = r#"@0ms: {
    lease handle = resource taking 10ms {
        match entropy(handle) {
            Valid(v): {
                print(v)
            }
            Decayed: {
                print(0)
            }
            Consumed: {
                print(-1)
            }
        }
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_isolate_block_with_manifest() {
        let code = r#"@0ms: {
    isolate worker {
        enable cpu(50ms)
        enable memory(1024bytes)
        slice 10ms
        require net.http
        let res = 1
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_type_and_enum_decls() {
        let code = r#"@0ms: {
    type Packet = struct decay_after 50ms {
        data: array,
        id: i64
    }

    enum Status {
        Active,
        Error(string)
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_foreign_block() {
        let code = r#"@0ms: {
    foreign "libm" abi("C") {
        routine sin(peek x: float) -> float taking 1ms
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_advanced_loop_sc_roundtrip() {
        let code = r#"@0ms: {
    let counter: int = 0
    let total: int = 0
    while (counter < 5) taking 25ms {
        let total = total + counter
        let counter = counter + 1
    }
    debug(counter)
    debug(total)
    let buffer = "sensor_stream_payload"
    let ticks = 0
    while valid (buffer) taking 30ms {
        let ticks = ticks + 1
        if (ticks == 3) {
            let extracted = buffer
            debug(extracted)
        } reconcile auto
    }
    debug(ticks)
}

@40ms: {
    let readings = [15, 42, 88, 23, 99]
    let sum = 0
    let peak = 0
    for val in readings step 10ms {
        let sum = sum + val
        if (val > peak) {
            let peak = val
        } reconcile auto
    }
    debug(sum)
    debug(peak)
    slice 20ms
    let count = 0
    let terminated = false
    loop tick {
        let count = count + 1
        let terminated = true
        break
    }
    debug(count)
    debug(terminated)
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_causm_oop_showcase_roundtrip() {
        let code = r#"@0ms: {
    type Actor = struct {
        id: int,
        name: string
    }

    type Robot = Actor + struct decay_after 100ms {
        model: string
    }

    type PlayableRobot = Robot + struct {
        score: int
    }

    routine Actor.introduce(peek self) -> int taking 20ms {
        let name = self.name
        print("Hello, I am Actor: " + name)
        let res = 0
        yield res
    }

    routine Robot.status(peek self) -> int taking 20ms {
        print("Robot status: active")
        let res = 0
        yield res
    }

    routine PlayableRobot.status(peek self) -> int taking 20ms {
        let score = self.score
        print("Playable Robot status: active, score=" + score)
        let res = 0
        yield res
    }

    interface Worker {
        routine work(consume self) -> int taking 20ms
    }

    interface PlayableWorker = Worker + interface {
        routine play(peek self) -> int taking 20ms {
            let bonus = 100
            yield bonus
        }
    }

    routine PlayableRobot.work(consume self) -> int taking 20ms {
        print("Robot is working...")
        let res = 0
        yield res
    }

    routine Robot.check_battery(peek self) -> int taking 20ms where self.state == Valid {
        print("Battery OK")
        let res = 0
        yield res
    }

    let r: PlayableRobot = struct { id = 42, model = "Cyberdyne Model 101", name = "T-800", score = 999 }
    r.introduce()
    r.status()
    r.check_battery()
    let w: PlayableWorker = r
    let bonus = w.play()
    if let robot = w.(PlayableRobot) {
        let model = &robot.model
        print("Downcast successful! Model: " + model)
        robot.work()
    } else {
        print("Downcast failed.")
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_entropic_oop_showcase_roundtrip() {
        let code = r#"@0ms: {
    type SecurityNode = struct decay_after 1000ms {
        node_id: int,
        status: string
    }

    type QuantumVault = SecurityNode + struct {
        clearance_level: int,
        vault_code: int
    }

    interface Encryptable {
        routine encrypt_payload(peek self) -> int taking 15ms
    }

    interface AuditNode = Encryptable + interface {
        routine audit(peek self) -> int taking 10ms {
            print("Audit log recorded to immutable ledger.")
            let ok = 1
            yield ok
        }
    }

    routine QuantumVault<int>.inspect_secure(peek self) -> int taking 20ms {
        let code = self.vault_code
        print("Quantum Vault leased securely. Decrypted Code: " + code)
        yield code
    }

    routine QuantumVault.verify_clearance(peek self) -> int taking 15ms where self.state == Valid {
        let level = self.clearance_level
        print("Clearance Verified Level: " + level)
        yield level
    }

    routine QuantumVault.encrypt_payload(peek self) -> int taking 15ms {
        let code = self.vault_code
        print("Payload Encrypted with Quantum Entanglement.")
        yield code
    }

    routine QuantumVault.purge(consume self) taking 15ms {
        print("Quantum Vault state purged and memory reclaimed.")
    }

    let v: QuantumVault = struct { clearance_level = 9, node_id = 8192, status = "OPERATIONAL", vault_code = 987654 }
    let code = v.inspect_secure()
    let level = v.verify_clearance()
    let node: AuditNode = v
    let log_status = node.audit()
    if let target_vault = node.(QuantumVault) {
        let id = &target_vault.node_id
        let status = &target_vault.status
        print("Downcast successful! Node ID: " + id + " Status: " + status)
        target_vault.purge()
    } else {
        print("Downcast failed.")
    }
}
"#;
        check_roundtrip(code);
    }

    #[test]
    fn test_fmt_isochronous_matrix_complex_roundtrip() {
        let code = r#"@0ms: {
    routine compute_ema(peek current: int, peek prev: int) -> int taking 20ms {
        let weight = 2
        let p1 = current * weight
        let p2 = prev * 8
        let sum = p1 + p2
        let ema = sum / 10
        yield ema
    }

    isolate dsp_pipeline {
        enable memory(102400bytes)
        slice 40ms
        require System.Log
        let mut s1 = 150
        let mut s2 = 155
        let mut signal = 0
        loop tick {
            signal = compute_ema(s2, s1)
            break
        }
        loop tick {
            let display = "Signal: " + signal
            print(display)
            break
        }
    }
}
"#;
        check_roundtrip(code);
    }
}
