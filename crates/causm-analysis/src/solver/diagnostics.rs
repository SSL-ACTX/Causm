use super::facts::PointIndex;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntropicDiagnostic {
    UseAfterConsume {
        var: String,
        origin_point: Option<PointIndex>,
        consume_point: PointIndex,
        access_point: PointIndex,
        smt_formula: String,
    },
    TemporalUseAfterDecay {
        var: String,
        decay_point: PointIndex,
        access_point: PointIndex,
        t_expire_ms: u64,
        t_access_ms: u64,
        smt_formula: String,
    },
    CompoundStructFieldDecay {
        var: String,
        field: String,
        field_consume_point: PointIndex,
        struct_access_point: PointIndex,
        smt_formula: String,
    },
    LeaseConflict {
        source_var: String,
        lease_id: String,
        lease_point: PointIndex,
        consume_point: PointIndex,
        t_start_ms: u64,
        t_end_ms: u64,
        smt_formula: String,
    },
    CausalParadox {
        anchor_name: String,
        anchor_point: PointIndex,
        rewind_point: PointIndex,
        commit_point: PointIndex,
        anchor_clock: u64,
        horizon_clock: u64,
        smt_formula: String,
    },
    EntanglementConflict {
        var: String,
        partner_var: String,
        partner_consume_point: PointIndex,
        access_point: PointIndex,
        smt_formula: String,
    },
    DoubleConsumeConflict {
        var: String,
        first_consume_point: PointIndex,
        second_consume_point: PointIndex,
        smt_formula: String,
    },
    CrossBranchCollision {
        var: String,
        branch_consumed: String,
        consume_point: PointIndex,
        branch_accessed: String,
        access_point: PointIndex,
        split_point: PointIndex,
        smt_formula: String,
    },
    SpeculativeLeak {
        var: String,
        spec_point: PointIndex,
        access_point: PointIndex,
        smt_formula: String,
    },
}

fn find_word_token(src: &str, tok: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    while let Some(idx) = src[offset..].find(tok) {
        let abs_pos = offset + idx;
        let before_ok = abs_pos == 0
            || !src[..abs_pos].chars().last().unwrap().is_alphanumeric()
                && !src[..abs_pos].ends_with('_');
        let after_pos = abs_pos + tok.len();
        let after_ok = after_pos >= src.len()
            || !src[after_pos..].chars().next().unwrap().is_alphanumeric()
                && !src[after_pos..].starts_with('_');
        if before_ok && after_ok {
            return Some((abs_pos, tok.len()));
        }
        offset = abs_pos + 1;
    }
    None
}

fn render_span(
    pt: &PointIndex,
    label: &str,
    highlight: Option<&str>,
    is_primary: bool,
) -> String {
    let mut out = String::new();
    if pt.line == 0 {
        out.push_str(&format!(
            "\x1b[1;94m  --> \x1b[0m{}:?:?\n\x1b[1;94m   |\x1b[0m\n   = {}\n",
            pt.file, label
        ));
        return out;
    }
    let line_num_width = pt.line.to_string().len().max(2);
    let pad = " ".repeat(line_num_width);
    let src = pt.source_text.trim_end();

    // Prefer exact word boundary token match
    let (ul_start, ul_len) = if let Some(tok) = highlight {
        if let Some((pos, len)) = find_word_token(src, tok) {
            (pos, len)
        } else if let Some(pos) = src.find(tok) {
            (pos, tok.len())
        } else {
            let col0 = pt.col.saturating_sub(1);
            let tlen = src[col0..]
                .split_whitespace()
                .next()
                .map(|t| t.len())
                .unwrap_or(1);
            (col0, tlen)
        }
    } else {
        let col0 = pt.col.saturating_sub(1);
        let tlen = src[col0..]
            .split_whitespace()
            .next()
            .map(|t| t.len())
            .unwrap_or(1);
        (col0, tlen)
    };
    let display_col = ul_start + 1;

    out.push_str(&format!(
        "\x1b[1;94m  --> \x1b[0m{}:{}:{}\n",
        pt.file, pt.line, display_col
    ));
    out.push_str(&format!("\x1b[1;94m{pad} |\x1b[0m\n"));

    let underline_char = if is_primary { '^' } else { '-' };
    let underline =
        " ".repeat(ul_start) + &underline_char.to_string().repeat(ul_len.max(1));
    let color_code = if is_primary {
        "\x1b[1;91m"
    } else {
        "\x1b[1;94m"
    };

    out.push_str(&format!(
        "\x1b[1;94m{:>width$} | \x1b[0m{}\n",
        pt.line,
        src,
        width = line_num_width
    ));
    out.push_str(&format!(
        "\x1b[1;94m{pad} | \x1b[0m{}{}\x1b[0m {}{}\x1b[0m\n",
        color_code, underline, color_code, label
    ));
    out.push_str(&format!("\x1b[1;94m{pad} |\x1b[0m\n"));
    out
}

/// Render a rustc-styled help tip block: `   \x1b[1;94m= \x1b[1;96mhelp:\x1b[0m <tip>`.
fn format_help(tip: &str) -> String {
    format!("   \x1b[1;94m= \x1b[1;96mhelp:\x1b[0m {}\n", tip)
}

/// Render the formal SMT constraint block matching rustc note styling: `   \x1b[1;94m= \x1b[1;95mnote[SMT]:\x1b[0m ...`.
fn render_smt(formula: &str) -> String {
    let mut out = String::new();
    out.push_str("   \x1b[1;94m= \x1b[1;95mnote[SMT]:\x1b[0m \x1b[1mformal entropic constraint (UNSAT proof)\x1b[0m\n");
    out.push_str(&format!("               \x1b[33m{}\x1b[0m\n", formula));
    out
}

impl EntropicDiagnostic {
    pub fn formula(&self) -> &str {
        match self {
            Self::UseAfterConsume { smt_formula, .. }
            | Self::TemporalUseAfterDecay { smt_formula, .. }
            | Self::CompoundStructFieldDecay { smt_formula, .. }
            | Self::LeaseConflict { smt_formula, .. }
            | Self::CausalParadox { smt_formula, .. }
            | Self::EntanglementConflict { smt_formula, .. }
            | Self::DoubleConsumeConflict { smt_formula, .. }
            | Self::CrossBranchCollision { smt_formula, .. }
            | Self::SpeculativeLeak { smt_formula, .. } => smt_formula,
        }
    }

    pub fn format_diagnostic(&self, include_smt_formula: bool) -> String {
        let mut out = String::new();
        match self {
            Self::UseAfterConsume {
                var,
                origin_point,
                consume_point,
                access_point,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0001]\x1b[0m: \x1b[1muse of consumed variable `{var}`\x1b[0m\n"
                ));
                if let Some(orig) = origin_point {
                    out.push_str(&render_span(
                        orig,
                        &format!("`{var}` value introduced here"),
                        Some(var),
                        false,
                    ));
                }
                out.push_str(&render_span(
                    consume_point,
                    &format!(
                        "`{var}` linearly consumed here — ownership transferred"
                    ),
                    Some(var),
                    false,
                ));
                out.push_str(&render_span(
                    access_point,
                    &format!("illegal: `{var}` used here after being consumed"),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(&format!(
                    "variable `{var}` follows linear ownership; once consumed it cannot be reused."
                )));
                out.push_str(&format_help(
                    "consider cloning before consuming, or restructure so the value is used at most once."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::TemporalUseAfterDecay {
                var,
                decay_point,
                access_point,
                t_expire_ms,
                t_access_ms,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0002]\x1b[0m: \x1b[1muse of temporally decayed variable `{var}`\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    decay_point,
                    &format!("`{var}` lifetime expires at clock {t_expire_ms}ms"),
                    Some(var),
                    false,
                ));
                out.push_str(&render_span(
                    access_point,
                    &format!(
                        "illegal: `{var}` accessed at clock {t_access_ms}ms, after TTL {t_expire_ms}ms"
                    ),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(&format!(
                    "the lifetime annotation `decay({t_expire_ms}ms)` on `{var}` means it is invalid after {t_expire_ms}ms."
                )));
                out.push_str(&format_help(&format!(
                    "move the access before clock {t_expire_ms}ms, or extend the lifetime annotation."
                )));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::CompoundStructFieldDecay {
                var,
                field,
                field_consume_point,
                struct_access_point,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0003]\x1b[0m: \x1b[1muse of structurally decayed struct `{var}` after field `{field}` was consumed\x1b[0m\n"
                ));
                let field_path = format!("{var}.{field}");
                out.push_str(&render_span(
                    field_consume_point,
                    &format!("field `{var}.{field}` linearly consumed here"),
                    Some(&field_path),
                    false,
                ));
                out.push_str(&render_span(
                    struct_access_point,
                    &format!("illegal: struct `{var}` accessed here but `{field}` is gone"),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(
                    "consuming a field invalidates the owning struct under Invariant 3."
                ));
                out.push_str(&format_help(&format!(
                    "destructure all fields before use, or do not consume `{var}.{field}` before accessing `{var}`."
                )));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::LeaseConflict {
                source_var,
                lease_id,
                lease_point,
                consume_point,
                t_start_ms,
                t_end_ms,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0004]\x1b[0m: \x1b[1mcannot consume `{source_var}` while active lease `{lease_id}` is held\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    lease_point,
                    &format!(
                        "lease `{lease_id}` granted here for interval [{t_start_ms}ms, {t_end_ms}ms]"
                    ),
                    Some(lease_id),
                    false,
                ));
                out.push_str(&render_span(
                    consume_point,
                    &format!("illegal: `{source_var}` consumed here while lease is active"),
                    Some(source_var),
                    true,
                ));
                out.push_str(&format_help(
                    "a leased variable cannot be consumed until the lease block exits and the lease is dropped."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::CausalParadox {
                anchor_name,
                anchor_point,
                rewind_point,
                commit_point,
                anchor_clock,
                horizon_clock,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0005]\x1b[0m: \x1b[1mCausal Paradox — rewind to anchor `{anchor_name}` violates causal horizon\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    anchor_point,
                    &format!("anchor `{anchor_name}` established at clock {anchor_clock}ms"),
                    Some(anchor_name),
                    false,
                ));
                out.push_str(&render_span(
                    commit_point,
                    &format!("irreversible commitment established causal horizon at clock {horizon_clock}ms"),
                    None,
                    false,
                ));
                out.push_str(&render_span(
                    rewind_point,
                    &format!("illegal: cannot rewind back to {anchor_clock}ms past causal horizon {horizon_clock}ms"),
                    Some(anchor_name),
                    true,
                ));
                out.push_str(&format_help(
                    "commitments (e.g. yield, external I/O) are irreversible; timeline cannot rewind past them."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::EntanglementConflict {
                var,
                partner_var,
                partner_consume_point,
                access_point,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0006]\x1b[0m: \x1b[1muse of entangled variable `{var}` decayed after partner `{partner_var}` was consumed\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    partner_consume_point,
                    &format!(
                        "entangled partner `{partner_var}` linearly consumed here"
                    ),
                    Some(partner_var),
                    false,
                ));
                out.push_str(&render_span(
                    access_point,
                    &format!("illegal: `{var}` is decayed due to entanglement with `{partner_var}`"),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(
                    "entangling variables links their entropic states; consuming one collapses the entire entangled set."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::DoubleConsumeConflict {
                var,
                first_consume_point,
                second_consume_point,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0007]\x1b[0m: \x1b[1mDouble Consume Conflict — variable `{var}` has been consumed multiple times along the same causal timeline\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    first_consume_point,
                    &format!("variable `{var}` initially consumed here"),
                    Some(var),
                    false,
                ));
                out.push_str(&render_span(
                    second_consume_point,
                    &format!("illegal: `{var}` consumed again here without re-introduction"),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(
                    "Causm enforces affine linear typing; values cannot be consumed more than once without being re-bound."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::CrossBranchCollision {
                var,
                branch_consumed,
                consume_point,
                branch_accessed,
                access_point,
                split_point,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0008]\x1b[0m: \x1b[1mCross-Branch Collision — variable `{var}` consumed in `{branch_consumed}` accessed concurrently in `{branch_accessed}`\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    split_point,
                    &format!("timeline split into concurrent branches `[{branch_consumed}, {branch_accessed}]` here"),
                    None,
                    false,
                ));
                out.push_str(&render_span(
                    consume_point,
                    &format!(
                        "`{var}` linearly consumed in branch `{branch_consumed}`"
                    ),
                    Some(var),
                    false,
                ));
                out.push_str(&render_span(
                    access_point,
                    &format!("illegal: concurrent access of `{var}` in parallel branch `{branch_accessed}` without merge reconciliation"),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(
                    "parallel branches cannot access linear variables consumed by sibling branches without a reconciliation merge."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }

            Self::SpeculativeLeak {
                var,
                spec_point,
                access_point,
                smt_formula,
            } => {
                out.push_str(&format!(
                    "\x1b[1;91merror[E0009]\x1b[0m: \x1b[1mSpeculative Leak — speculative variable `{var}` accessed outside speculative boundary without commit\x1b[0m\n"
                ));
                out.push_str(&render_span(
                    spec_point,
                    &format!("speculative variable `{var}` introduced here"),
                    Some(var),
                    false,
                ));
                out.push_str(&render_span(
                    access_point,
                    &format!("illegal: uncommitted speculative variable `{var}` accessed in outer timeline"),
                    Some(var),
                    true,
                ));
                out.push_str(&format_help(
                    "speculative state must be explicitly committed with `commit` before leaking across speculation boundaries."
                ));
                if include_smt_formula {
                    out.push_str(&render_smt(smt_formula));
                }
            }
        }
        out
    }
}

impl fmt::Display for EntropicDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_diagnostic(true))
    }
}
