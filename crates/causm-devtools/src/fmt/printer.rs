use super::rules::FormatConfig;
use causm_core::{
    BinaryOperator, Expression, MergeResolution, ParamDecl, ParamMode, Program,
    SpannedStatement, Statement, TypeName, UnaryOperator,
};

pub fn format_program(program: &Program, config: &FormatConfig) -> String {
    let mut out = String::new();
    for (i, tb) in program.timelines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let time_str = match &tb.time {
            causm_core::TimeCoordinate::Global(t) => format!("{}ms", t),
            causm_core::TimeCoordinate::Relative(t) => format!("+{}ms", t),
            causm_core::TimeCoordinate::Branch(b) => b.clone(),
        };
        let directives = if tb.no_z3 { " @no_z3" } else { "" };
        out.push_str(&format!("@{}{}: {{\n", time_str, directives));
        for stmt in &tb.statements {
            format_spanned_statement(&mut out, stmt, config.indent_spaces, 1);
        }
        out.push_str("}\n");
    }
    out
}

fn format_spanned_statement(
    out: &mut String,
    stmt: &SpannedStatement,
    indent_step: usize,
    depth: usize,
) {
    let indent = " ".repeat(indent_step * depth);
    match &stmt.stmt {
        Statement::Assignment {
            target,
            mutable,
            var_type,
            expr,
            ..
        } => {
            let mut_str = if *mutable { "mut " } else { "" };
            let type_str = if let Some(t) = var_type {
                format!(": {}", format_type(t))
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{}let {}{}{} = {}\n",
                indent,
                mut_str,
                target,
                type_str,
                format_expr(expr)
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
                format_expr(expr)
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
                format_expr(resource)
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::RoutineDef {
            name,
            params,
            return_type,
            taking_ms,
            state_constraint,
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
            let contract_str = match taking_ms {
                Some(ms) => format!(" taking {}ms", ms),
                None => " taking _".to_string(),
            };
            let state_str = if let Some((var, state)) = state_constraint {
                format!(" where {}.state == {}", var, state)
            } else {
                String::new()
            };
            if body.is_empty() {
                out.push_str(&format!(
                    "{}routine {}({}){}{}{}\n",
                    indent, name, params_str, ret_str, contract_str, state_str
                ));
            } else {
                out.push_str(&format!(
                    "{}routine {}({}){}{}{} {{\n",
                    indent, name, params_str, ret_str, contract_str, state_str
                ));
                for s in body {
                    format_spanned_statement(out, s, indent_step, depth + 1);
                }
                out.push_str(&format!("{}}}\n", indent));
            }
        }
        Statement::TypeDecl {
            name,
            extends,
            fields,
            decay_after_ms,
            ..
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
            out.push_str(&format!(
                "{}type {} = {}struct{} {{\n",
                indent, name, ext_str, decay_str
            ));
            let mut sorted_fields: Vec<_> = fields.iter().collect();
            sorted_fields.sort_by_key(|(fname, _)| fname.as_str());
            let field_entries: Vec<_> = sorted_fields
                .iter()
                .map(|(fname, fdef)| {
                    format!(
                        "{}{}: {}",
                        " ".repeat(indent_step * (depth + 1)),
                        fname,
                        format_type(&fdef.typ)
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
                let taking_str = if let Some(t) = m.taking_ms {
                    format!(" taking {}ms", t)
                } else {
                    String::new()
                };
                if let Some(ref b) = m.default_body {
                    out.push_str(&format!(
                        "{}routine {}({}){}{}{} {{\n",
                        " ".repeat(indent_step * (depth + 1)),
                        m.name,
                        params_str,
                        ret_str,
                        taking_str,
                        ""
                    ));
                    for s in b {
                        format_spanned_statement(out, s, indent_step, depth + 2);
                    }
                    out.push_str(&format!(
                        "{}}}\n",
                        " ".repeat(indent_step * (depth + 1))
                    ));
                } else {
                    out.push_str(&format!(
                        "{}routine {}({}){}{}\n",
                        " ".repeat(indent_step * (depth + 1)),
                        m.name,
                        params_str,
                        ret_str,
                        taking_str
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
            let if_head = if let Some(b) = binding {
                format!("if let {} = {}", b, format_expr(condition))
            } else {
                format!("if ({})", format_expr(condition))
            };
            out.push_str(&format!("{}{} {{\n", indent, if_head));
            for s in then_branch {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            if let Some(eb) = else_branch {
                out.push_str(&format!("{}}} else {{\n", indent));
                for s in eb {
                    format_spanned_statement(out, s, indent_step, depth + 1);
                }
            }
            let rec_str = reconcile
                .as_ref()
                .map(format_merge_resolution)
                .unwrap_or_default();
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::Loop { max_ms, body } => {
            out.push_str(&format!("loop taking {}ms {{\n", max_ms));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
            out.push_str(&format!(
                "{}while {}({}) taking {}ms {{\n",
                indent,
                valid_str,
                format_expr(condition),
                max_ms
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
            let max_str = if let Some(m) = max_ms {
                format!(" taking {}ms", m)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{}for {} {} {}{}{} {{\n",
                indent, item_name, mode_str, source, pacing_str, max_str
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
                format_expr(source),
                step_str
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
            let formatted =
                args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            out.push_str(&format!("{}print({})\n", indent, formatted));
        }
        Statement::Debug(expr) => {
            out.push_str(&format!("{}debug({})\n", indent, format_expr(expr)));
        }
        Statement::Expression(expr) => {
            out.push_str(&format!("{}{}\n", indent, format_expr(expr)));
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
            let res_str = format_merge_resolution(resolutions);
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
            out.push_str(&format!("{}rewind {}\n", indent, name));
        }
        Statement::Commit(body) => {
            out.push_str(&format!("{}commit {{\n", indent));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Collapse => {
            out.push_str(&format!("{}collapse\n", indent));
        }
        Statement::Speculate {
            max_ms,
            body,
            fallback,
        } => {
            out.push_str(&format!("{}speculate (taking {}ms) {{\n", indent, max_ms));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            if let Some(fb) = fallback {
                out.push_str(&format!("{}}} fallback {{\n", indent));
                for s in fb {
                    format_spanned_statement(out, s, indent_step, depth + 1);
                }
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Inspect {
            binding,
            target,
            body,
        } => {
            out.push_str(&format!(
                "{}inspect {} = {} {{\n",
                indent, binding, target
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
                .map(format_merge_resolution)
                .unwrap_or_default();
            out.push_str(&format!(
                "{}lease {} = {} taking {}ms {{\n",
                indent, binding, source, duration_ms
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::ChannelOpen {
            name,
            capacity,
            decay_after_ms,
        } => {
            let decay_str = if let Some(ms) = decay_after_ms {
                format!(" decay_after {}ms", ms)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{}open_chan {} ({}){}\n",
                indent, name, capacity, decay_str
            ));
        }
        Statement::ChannelSend { chan_id, value_id } => {
            out.push_str(&format!(
                "{}chan_send {} ({})\n",
                indent, chan_id, value_id
            ));
        }
        Statement::EnumDecl { name, variants } => {
            out.push_str(&format!("{}enum {} {{\n", indent, name));
            let v_strs: Vec<_> = variants
                .iter()
                .map(|v| {
                    if v.payload_types.is_empty() {
                        format!(
                            "{}{}",
                            " ".repeat(indent_step * (depth + 1)),
                            v.name
                        )
                    } else {
                        let t_strs = v
                            .payload_types
                            .iter()
                            .map(format_type)
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!(
                            "{}{}({})",
                            " ".repeat(indent_step * (depth + 1)),
                            v.name,
                            t_strs
                        )
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
                .map(format_merge_resolution)
                .unwrap_or_default();
            out.push_str(&format!(
                "{}split_map {} {} {} {{\n",
                indent, item_name, mode_str, source
            ));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
            out.push_str(&format!("{}isolate {} {{\n", indent, name_str));
            if let Some(cpu) = iso.manifest.cpu_budget_ms {
                out.push_str(&format!(
                    "{}enable cpu({}ms)\n",
                    " ".repeat(indent_step * (depth + 1)),
                    cpu
                ));
            }
            if let Some(mem) = iso.manifest.memory_budget_bytes {
                out.push_str(&format!(
                    "{}enable memory({}bytes)\n",
                    " ".repeat(indent_step * (depth + 1)),
                    mem
                ));
            }
            if let Some(sl) = iso.manifest.slice_ms {
                out.push_str(&format!(
                    "{}slice {}ms\n",
                    " ".repeat(indent_step * (depth + 1)),
                    sl
                ));
            }
            let mut budgets: Vec<_> = iso.manifest.resource_budgets.iter().collect();
            budgets.sort_by_key(|(res, _)| res.as_str());
            for (res, amt) in budgets {
                out.push_str(&format!(
                    "{}enable {}({})\n",
                    " ".repeat(indent_step * (depth + 1)),
                    res,
                    amt
                ));
            }
            for cap in &iso.manifest.capabilities {
                if cap.parameters.is_empty() {
                    out.push_str(&format!(
                        "{}require {}\n",
                        " ".repeat(indent_step * (depth + 1)),
                        cap.path
                    ));
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
                        " ".repeat(indent_step * (depth + 1)),
                        cap.path,
                        p_strs
                    ));
                }
            }
            for s in &iso.body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
                format_expr(target)
            ));
            if let Some((pat, guard, body)) = valid_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g)))
                    .unwrap_or_default();
                let pat_str = format_decayed_pattern(pat);
                let pat_display = if pat_str.is_empty() {
                    String::new()
                } else {
                    format!("({})", pat_str)
                };
                out.push_str(&format!(
                    "{}Valid{}{}: {{\n",
                    " ".repeat(indent_step * (depth + 1)),
                    pat_display,
                    g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, indent_step, depth + 2);
                }
                out.push_str(&format!(
                    "{}}}\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
            }
            if let Some((pat, guard, body)) = decayed_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g)))
                    .unwrap_or_default();
                let pat_str = format_decayed_pattern(pat);
                let pat_display = if pat_str.is_empty() {
                    String::new()
                } else {
                    format!("({})", pat_str)
                };
                out.push_str(&format!(
                    "{}Decayed{}{}: {{\n",
                    " ".repeat(indent_step * (depth + 1)),
                    pat_display,
                    g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, indent_step, depth + 2);
                }
                out.push_str(&format!(
                    "{}}}\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
            }
            if let Some((pat, guard, body)) = pending_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g)))
                    .unwrap_or_default();
                let pat_str = format_decayed_pattern(pat);
                let pat_display = if pat_str.is_empty() {
                    String::new()
                } else {
                    format!("({})", pat_str)
                };
                out.push_str(&format!(
                    "{}Pending{}{}: {{\n",
                    " ".repeat(indent_step * (depth + 1)),
                    pat_display,
                    g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, indent_step, depth + 2);
                }
                out.push_str(&format!(
                    "{}}}\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
            }
            if let Some((guard, body)) = consumed_branch {
                let g_str = guard
                    .as_ref()
                    .map(|g| format!(" if {}", format_expr(g)))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{}Consumed{}: {{\n",
                    " ".repeat(indent_step * (depth + 1)),
                    g_str
                ));
                for s in body {
                    format_spanned_statement(out, s, indent_step, depth + 2);
                }
                out.push_str(&format!(
                    "{}}}\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::RelativisticBlock { time, body } => {
            let time_str = match time {
                causm_core::TimeCoordinate::Global(t) => format!("{}ms", t),
                causm_core::TimeCoordinate::Relative(t) => format!("+{}ms", t),
                causm_core::TimeCoordinate::Branch(b) => b.clone(),
            };
            out.push_str(&format!("{}@{}: {{\n", indent, time_str));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::Await(id) => {
            out.push_str(&format!("{}await({})\n", indent, id));
        }
        Statement::AwaitChan(id) => {
            out.push_str(&format!("{}await_chan({})\n", indent, id));
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
                    format_spanned_statement(out, s, indent_step, depth + 1);
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
            if let Some(msg) = cap.parameters.get("message") {
                out.push_str(&format!("{}log(\"{}\")\n", indent, msg));
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
                .map(format_merge_resolution)
                .unwrap_or_default();
            out.push_str(&format!("{}select (taking {}ms) {{\n", indent, max_ms));
            for case in cases {
                out.push_str(&format!(
                    "{}case {} = {}: {{\n",
                    " ".repeat(indent_step * (depth + 1)),
                    case.binding,
                    format_expr(&case.source)
                ));
                for s in &case.body {
                    format_spanned_statement(out, s, indent_step, depth + 2);
                }
                out.push_str(&format!(
                    "{}}}\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
            }
            if let Some(to) = timeout {
                out.push_str(&format!(
                    "{}timeout: {{\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
                for s in to {
                    format_spanned_statement(out, s, indent_step, depth + 2);
                }
                out.push_str(&format!(
                    "{}}}\n",
                    " ".repeat(indent_step * (depth + 1))
                ));
            }
            out.push_str(&format!("{}}}{}\n", indent, rec_str));
        }
        Statement::LoopTick { body } => {
            out.push_str(&format!("{}loop tick {{\n", indent));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::LoopTickOn { channel, body } => {
            out.push_str(&format!("{}loop tick on {} {{\n", indent, channel));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::DecayHandler { type_name, body } => {
            out.push_str(&format!("{}decay_handler for {} {{\n", indent, type_name));
            for s in body {
                format_spanned_statement(out, s, indent_step, depth + 1);
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
                format_spanned_statement(out, s, indent_step, depth + 1);
            }
            out.push_str(&format!("{}}}\n", indent));
        }
        Statement::AcausalReset {
            target,
            anchor_name,
        } => {
            out.push_str(&format!(
                "{}reset {} to {}\n",
                indent, target, anchor_name
            ));
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
                out.push_str(&format!(
                    "{}{} = {}\n",
                    indent,
                    format_expr(target),
                    format_expr(value)
                ));
            } else {
                out.push_str(&format!(
                    "{}{}.{} = {}\n",
                    indent,
                    format_expr(target),
                    field,
                    format_expr(value)
                ));
            }
        }
        other => {
            eprintln!(
                "\x1b[33mwarning:\x1b[0m Unhandled statement in formatter: {:?}",
                other
            );
        }
    }
}

fn format_decayed_pattern(pat: &causm_core::DecayedPattern) -> String {
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
                        format!("{} = {}", k, format_expr(e))
                    }
                })
                .collect();
            format!("{{ {} }}", f_strs.join(", "))
        }
    }
}

fn format_merge_resolution(res: &MergeResolution) -> String {
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
                    _ => "first_wins".to_string(),
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
            causm_core::BuiltinType::Float => "float".to_string(),
            causm_core::BuiltinType::Bool => "bool".to_string(),
            causm_core::BuiltinType::String => "string".to_string(),
            causm_core::BuiltinType::Struct => "struct".to_string(),
            causm_core::BuiltinType::Topology => "topology".to_string(),
            causm_core::BuiltinType::Array => "array".to_string(),
            other => format!("{:?}", other).to_lowercase(),
        },
        TypeName::Custom(c) => c.clone(),
        TypeName::Generic(name, params) => {
            let params_str = params
                .iter()
                .map(|p| match p {
                    causm_core::TypeParam::Type(t) => format_type(t),
                    causm_core::TypeParam::Amount(a) => a.to_string(),
                    causm_core::TypeParam::Duration(d) => format!("{}ms", d),
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

fn format_expr(expr: &Expression) -> String {
    match expr {
        Expression::Integer(i) => i.to_string(),
        Expression::Float(bits) => f64::from_bits(*bits).to_string(),
        Expression::Boolean(b) => b.to_string(),
        Expression::Literal(s) => format!("\"{}\"", s),
        Expression::Identifier(id) => id.clone(),
        Expression::Null => "null".to_string(),
        Expression::Call { routine, args } => {
            let args_str =
                args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{}({})", routine, args_str)
        }
        Expression::MethodCall {
            target,
            method,
            args,
            ..
        } => {
            let args_str =
                args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            format!("{}.{}({})", format_expr(target), method, args_str)
        }
        Expression::FieldAccess { target, field } => {
            format!("{}.{}", format_expr(target), field)
        }
        Expression::BinaryOp { left, op, right } => {
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
            };
            format!("{} {} {}", format_expr(left), op_str, format_expr(right))
        }
        Expression::UnaryOp { op, expr } => {
            let op_str = match op {
                UnaryOperator::Neg => "-",
                UnaryOperator::Not => "!",
            };
            format!("{}{}", op_str, format_expr(expr))
        }
        Expression::StructLit(type_opt, fields) => {
            let type_str = if let Some(t) = type_opt.borrow().as_ref() {
                format!("{}: ", t)
            } else {
                String::new()
            };
            let mut sorted_fields: Vec<_> = fields.iter().collect();
            sorted_fields.sort_by_key(|(k, _)| k.as_str());
            let mut f_strs = Vec::new();
            for (k, v) in sorted_fields {
                f_strs.push(format!("{} = {}", k, format_expr(v)));
            }
            format!("{}struct {{ {} }}", type_str, f_strs.join(", "))
        }
        Expression::CloneOp(id) => format!("clone({})", id),
        Expression::StrBytes(expr) => format!("str_bytes({})", format_expr(expr)),
        Expression::ToStr(expr) => format!("to_str({})", format_expr(expr)),
        Expression::Len(expr) => format!("len({})", format_expr(expr)),
        Expression::RefOp(inner) => format!("&{}", format_expr(inner)),
        Expression::ArrayLiteral(elements) => {
            let elems_str = elements
                .iter()
                .map(format_expr)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{}]", elems_str)
        }
        Expression::ArrayRepeat { value, count } => {
            format!("[{}; {}]", format_expr(value), format_expr(count))
        }
        Expression::ArraySlice {
            target,
            start,
            end,
            inclusive,
        } => {
            let s_str = start.as_ref().map(|s| format_expr(s)).unwrap_or_default();
            let dot_str = if *inclusive { "..=" } else { ".." };
            let e_str = end.as_ref().map(|e| format_expr(e)).unwrap_or_default();
            format!("{}[{}{}{}]", format_expr(target), s_str, dot_str, e_str)
        }
        Expression::IndexAccess { target, index } => {
            format!("{}[{}]", format_expr(target), format_expr(index))
        }
        Expression::ChannelReceive(chan) => format!("chan_recv({})", chan),
        Expression::TopologyLit(fields) => {
            let mut sorted_fields: Vec<_> = fields.iter().collect();
            sorted_fields.sort_by_key(|(k, _)| k.as_str());
            let mut f_strs = Vec::new();
            for (k, v) in sorted_fields {
                f_strs.push(format!("{} = {}", k, format_expr(v)));
            }
            format!("topology {{ {} }}", f_strs.join(", "))
        }
        Expression::TypeCast { expr, target_type } => {
            format!("{} as {}", format_expr(expr), format_type(target_type))
        }
        Expression::TypeAssertion { target, cast_type } => {
            format!("{}.({})", format_expr(target), format_type(cast_type))
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
            let args_str =
                args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
            let target_str = match target {
                causm_core::SyscallTarget::Number(n) => n.to_string(),
                causm_core::SyscallTarget::Symbol(s) => format!("\"{}\"", s),
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
                let args_str =
                    args.iter().map(format_expr).collect::<Vec<_>>().join(", ");
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
                    causm_core::FStringPart::Text(t) => s.push_str(t),
                    causm_core::FStringPart::Expr(e) => {
                        s.push('{');
                        s.push_str(&format_expr(e));
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
            format!(
                "if ({}) {{ {} }} else {{ {} }}",
                format_expr(condition),
                format_expr(then_branch),
                format_expr(else_branch)
            )
        }
        other => {
            eprintln!("\x1b[33mwarning:\x1b[0m Unhandled expression variant in formatter: {:?}", other);
            "null".to_string()
        }
    }
}
