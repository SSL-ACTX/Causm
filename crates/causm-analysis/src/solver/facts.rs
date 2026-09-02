use causm_core::{
    Expression, LifetimeAnnotation, Pattern, Program, Statement, TimeCoordinate,
};
use causm_ir::cfg::BlockId;
use causm_ir::ssa::SsaCFG;
use std::collections::{HashMap, HashSet};

/// Location of an instruction within the Control Flow Graph / Timeline structure.
///
/// Carries source location metadata so diagnostics can render rustc-grade
/// multi-span output with actual source lines and `^^^` underlines.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointIndex {
    pub timeline_idx: usize,
    pub statement_idx: usize,
    pub sub_point: usize,
    /// Filename of the source file (e.g. `foo.csm`).
    pub file: String,
    /// 1-indexed source line number.
    pub line: usize,
    /// 1-indexed source column number.
    pub col: usize,
    /// Verbatim source text of the statement at this point.
    pub source_text: String,
}

impl PointIndex {
    pub fn new(timeline_idx: usize, statement_idx: usize, sub_point: usize) -> Self {
        Self {
            timeline_idx,
            statement_idx,
            sub_point,
            file: String::new(),
            line: 0,
            col: 0,
            source_text: String::new(),
        }
    }

    pub fn with_source(
        timeline_idx: usize,
        statement_idx: usize,
        sub_point: usize,
        file: String,
        line: usize,
        col: usize,
        source_text: String,
    ) -> Self {
        Self { timeline_idx, statement_idx, sub_point, file, line, col, source_text }
    }
}

impl PartialOrd for PointIndex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PointIndex {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.timeline_idx, self.statement_idx, self.sub_point)
            .cmp(&(other.timeline_idx, other.statement_idx, other.sub_point))
    }
}

/// Point within a Basic Block in the SSA Control Flow Graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SsaPointIndex {
    pub block_id: BlockId,
    pub instruction_idx: usize,
}

impl SsaPointIndex {
    pub fn new(block_id: BlockId, instruction_idx: usize) -> Self {
        Self {
            block_id,
            instruction_idx,
        }
    }
}

/// Relational fact extracted from an SSA instruction or CFG point.
///
/// These facts form the formal base inputs to the Entropius invariant solver.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntropicFact {
    /// Variable `var` was allocated or bound at CFG point `point`.
    ValueIntroduced { var: String, point: PointIndex },
    /// Variable `var` was explicitly moved, consumed, or auto-dropped at `point`.
    LinearConsume { var: String, point: PointIndex },
    /// Field `field` of struct variable `var` was destructured/consumed at `point`.
    FieldConsume {
        var: String,
        field: String,
        point: PointIndex,
    },
    /// Lease on variable `var` is active over virtual clock interval `[t_start, t_end]`.
    LeaseIssued {
        var: String,
        lease_id: String,
        t_start: u64,
        t_end: u64,
        point: PointIndex,
    },
    /// Variable `var` has a TTL expiring at `t_expire`.
    TemporalDecay {
        var: String,
        t_expire: u64,
        point: PointIndex,
    },
    /// Variable `var` was read, peeked, or referenced at `point` at clock `t_current`.
    AccessAt {
        var: String,
        point: PointIndex,
        t_current: u64,
    },
    /// Directed Control Flow Graph edge between two basic blocks or points.
    CfgEdge {
        from_point: PointIndex,
        to_point: PointIndex,
    },
    /// Relativistic branch split into child branches.
    BranchSplit {
        parent: String,
        children: Vec<String>,
        point: PointIndex,
    },
    /// Relativistic branch merge back into target.
    BranchMerge {
        branches: Vec<String>,
        target: String,
        point: PointIndex,
    },
    /// Entanglement fact binding multiple variables so consuming one decays the others.
    Entangle {
        variables: Vec<String>,
        point: PointIndex,
    },
    /// Anchor marker in timeline with associated clock timestamp.
    Anchor {
        name: String,
        clock: u64,
        point: PointIndex,
    },
    /// Rewind instruction targeting a previously recorded anchor.
    Rewind {
        target: String,
        clock: u64,
        point: PointIndex,
    },
    /// Causal commitment (e.g. Yield / Commit) establishing a forward horizon.
    CausalCommit {
        clock: u64,
        point: PointIndex,
    },
}

/// Complete collection of relational facts and relations extracted for a program.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProgramFacts {
    pub facts: Vec<EntropicFact>,
    pub cfg_edges: Vec<(PointIndex, PointIndex)>,
    pub var_origins: HashMap<String, HashSet<PointIndex>>,
    pub var_consumes: HashMap<String, HashSet<PointIndex>>,
    pub var_accesses: HashMap<String, Vec<(PointIndex, u64)>>,
    pub var_decays: HashMap<String, Vec<(PointIndex, u64)>>,
    pub active_leases: HashMap<String, Vec<(String, u64, u64, PointIndex)>>,
    pub entanglements: Vec<HashSet<String>>,
    pub anchors: HashMap<String, (u64, PointIndex)>,
    pub rewinds: Vec<(String, u64, PointIndex)>,
    pub commits: Vec<(u64, PointIndex)>,
}

impl ProgramFacts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_fact(&mut self, fact: EntropicFact) {
        match &fact {
            EntropicFact::ValueIntroduced { var, point } => {
                self.var_origins
                    .entry(var.clone())
                    .or_default()
                    .insert(point.clone());
            }
            EntropicFact::LinearConsume { var, point } => {
                self.var_consumes
                    .entry(var.clone())
                    .or_default()
                    .insert(point.clone());
            }
            EntropicFact::FieldConsume { var, field, point } => {
                let path = format!("{}.{}", var, field);
                self.var_consumes
                    .entry(path)
                    .or_default()
                    .insert(point.clone());
            }
            EntropicFact::LeaseIssued {
                var,
                lease_id,
                t_start,
                t_end,
                point,
            } => {
                self.active_leases
                    .entry(var.clone())
                    .or_default()
                    .push((lease_id.clone(), *t_start, *t_end, point.clone()));
            }
            EntropicFact::TemporalDecay {
                var,
                t_expire,
                point,
            } => {
                self.var_decays
                    .entry(var.clone())
                    .or_default()
                    .push((point.clone(), *t_expire));
            }
            EntropicFact::AccessAt {
                var,
                point,
                t_current,
            } => {
                self.var_accesses
                    .entry(var.clone())
                    .or_default()
                    .push((point.clone(), *t_current));
            }
            EntropicFact::CfgEdge {
                from_point,
                to_point,
            } => {
                self.cfg_edges.push((from_point.clone(), to_point.clone()));
            }
            EntropicFact::BranchSplit { .. } | EntropicFact::BranchMerge { .. } => {}
            EntropicFact::Entangle { variables, .. } => {
                let mut new_set = HashSet::new();
                for v in variables {
                    new_set.insert(v.clone());
                }
                let mut merged_set = new_set;
                let mut i = 0;
                while i < self.entanglements.len() {
                    if self.entanglements[i].iter().any(|v| merged_set.contains(v)) {
                        merged_set.extend(self.entanglements.remove(i));
                    } else {
                        i += 1;
                    }
                }
                self.entanglements.push(merged_set);
            }
            EntropicFact::Anchor { name, clock, point } => {
                self.anchors.insert(name.clone(), (*clock, point.clone()));
            }
            EntropicFact::Rewind { target, clock, point } => {
                self.rewinds.push((target.clone(), *clock, point.clone()));
            }
            EntropicFact::CausalCommit { clock, point } => {
                self.commits.push((*clock, point.clone()));
            }
        }
        self.facts.push(fact);
    }
}

/// Context maintained during fact extraction across a program's timelines.
struct FactExtractor {
    timeline_idx: usize,
    statement_idx: usize,
    sub_point: usize,
    current_clock: u64,
    program_facts: ProgramFacts,
    /// All lines of the source file split by `\n`.
    source_lines: Vec<String>,
    /// Path of the source file being analyzed.
    filename: String,
    /// Line number (1-indexed) of the current statement being extracted.
    current_stmt_line: usize,
    /// Column number (1-indexed) of the current statement being extracted.
    current_stmt_col: usize,
    /// Verbatim source text of the current statement.
    current_stmt_source: String,
}

impl FactExtractor {
    fn new(source: &str, filename: &str) -> Self {
        Self {
            timeline_idx: 0,
            statement_idx: 0,
            sub_point: 0,
            current_clock: 0,
            program_facts: ProgramFacts::new(),
            source_lines: source.lines().map(|l| l.to_string()).collect(),
            filename: filename.to_string(),
            current_stmt_line: 0,
            current_stmt_col: 0,
            current_stmt_source: String::new(),
        }
    }

    fn next_point(&mut self) -> PointIndex {
        let pt = PointIndex::with_source(
            self.timeline_idx,
            self.statement_idx,
            self.sub_point,
            self.filename.clone(),
            self.current_stmt_line,
            self.current_stmt_col,
            self.current_stmt_source.clone(),
        );
        self.sub_point += 1;
        pt
    }

    fn push_fact(&mut self, fact: EntropicFact) {
        self.program_facts.insert_fact(fact);
    }

    fn extract_from_expression(&mut self, expr: &Expression, consuming: bool) {
        match expr {
            Expression::Identifier(name) => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::AccessAt {
                    var: name.clone(),
                    point: pt.clone(),
                    t_current: self.current_clock,
                });
                if consuming {
                    self.push_fact(EntropicFact::LinearConsume {
                        var: name.clone(),
                        point: pt,
                    });
                }
            }
            Expression::FieldAccess { target, field } => {
                if let Expression::Identifier(target_name) = &**target {
                    let pt = self.next_point();
                    self.push_fact(EntropicFact::AccessAt {
                        var: format!("{}.{}", target_name, field),
                        point: pt.clone(),
                        t_current: self.current_clock,
                    });
                    if consuming {
                        self.push_fact(EntropicFact::FieldConsume {
                            var: target_name.clone(),
                            field: field.clone(),
                            point: pt,
                        });
                    }
                } else {
                    self.extract_from_expression(target, consuming);
                }
            }
            Expression::MethodCall { target, args, .. } => {
                self.extract_from_expression(target, false);
                for arg in args {
                    self.extract_from_expression(arg, false);
                }
            }
            Expression::Call { args, .. } => {
                for arg in args {
                    self.extract_from_expression(arg, false);
                }
            }
            Expression::BinaryOp { left, right, .. } => {
                self.extract_from_expression(left, false);
                self.extract_from_expression(right, false);
            }
            Expression::UnaryOp { expr, .. }
            | Expression::RefOp(expr)
            | Expression::ToStr(expr)
            | Expression::StrBytes(expr)
            | Expression::Len(expr)
            | Expression::TryUnwrap(expr)
            | Expression::TypeAssertion { target: expr, .. }
            | Expression::TypeCast { expr, .. }
            | Expression::Turbofish { expr, .. } => {
                self.extract_from_expression(expr, false);
            }
            Expression::CloneOp(name) => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::AccessAt {
                    var: name.clone(),
                    point: pt,
                    t_current: self.current_clock,
                });
            }
            Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
                for inner_expr in fields.values() {
                    self.extract_from_expression(inner_expr, false);
                }
            }
            Expression::ArrayLiteral(elements) | Expression::Tuple(elements) => {
                for elem in elements {
                    self.extract_from_expression(elem, false);
                }
            }
            Expression::ArrayRepeat { value, count } => {
                self.extract_from_expression(value, false);
                self.extract_from_expression(count, false);
            }
            Expression::ArraySlice {
                target, start, end, ..
            } => {
                self.extract_from_expression(target, false);
                if let Some(s) = start {
                    self.extract_from_expression(s, false);
                }
                if let Some(e) = end {
                    self.extract_from_expression(e, false);
                }
            }
            Expression::IndexAccess { target, index } => {
                self.extract_from_expression(target, false);
                self.extract_from_expression(index, false);
            }
            Expression::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.extract_from_expression(condition, false);
                self.extract_from_expression(then_branch, false);
                self.extract_from_expression(else_branch, false);
            }
            Expression::Match { target, arms } => {
                self.extract_from_expression(target, false);
                for arm in arms {
                    if let Some(ref g) = arm.guard {
                        self.extract_from_expression(g, false);
                    }
                    self.extract_from_expression(&arm.body, false);
                }
            }
            Expression::GenericStaticCall { args, .. }
            | Expression::EnumVariant { args, .. }
            | Expression::Syscall { args, .. } => {
                for arg in args {
                    self.extract_from_expression(arg, false);
                }
            }
            Expression::FString(parts) => {
                for part in parts {
                    if let causm_core::FStringPart::Expr(e) = part {
                        self.extract_from_expression(e, false);
                    }
                }
            }
            Expression::Null
            | Expression::Literal(_)
            | Expression::Integer(_)
            | Expression::Float(_)
            | Expression::Boolean(_)
            | Expression::ArenaIntrospect(_)
            | Expression::CapabilityCheck(_)
            | Expression::ChannelReceive(_)
            | Expression::Deferred { .. } => {}
        }
    }

    fn extract_from_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier(var) => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: var.clone(),
                    point: pt,
                });
            }
            Pattern::TypeAssert { binding, .. } => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: binding.clone(),
                    point: pt,
                });
            }
            Pattern::Tuple(patterns) => {
                for pat in patterns {
                    self.extract_from_pattern(pat);
                }
            }
            Pattern::EnumVariant { args, .. } => {
                for pat in args {
                    self.extract_from_pattern(pat);
                }
            }
            Pattern::Literal(_) | Pattern::Wildcard => {}
        }
    }

    fn extract_from_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Assignment {
                target,
                expr,
                lifetime,
                ..
            } => {
                self.extract_from_expression(expr, false);
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: target.clone(),
                    point: pt.clone(),
                });

                if let Some(LifetimeAnnotation::Decayed(duration_ms)) = lifetime {
                    let expire_time = self.current_clock.saturating_add(*duration_ms);
                    self.push_fact(EntropicFact::TemporalDecay {
                        var: target.clone(),
                        t_expire: expire_time,
                        point: pt,
                    });
                }
            }
            Statement::DestructureAssignment { fields, expr, .. } => {
                self.extract_from_expression(expr, false);
                for (_field, binding) in fields {
                    let pt = self.next_point();
                    self.push_fact(EntropicFact::ValueIntroduced {
                        var: binding.clone(),
                        point: pt,
                    });
                }
            }
            Statement::StateDecl { target, expr, .. } => {
                self.extract_from_expression(expr, false);
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: target.clone(),
                    point: pt,
                });
            }
            Statement::Expression(expr) => {
                self.extract_from_expression(expr, false);
            }
            Statement::Using {
                binding,
                resource,
                body,
            } => {
                self.extract_from_expression(resource, false);
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: binding.clone(),
                    point: pt.clone(),
                });
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
                let drop_pt = self.next_point();
                self.push_fact(EntropicFact::LinearConsume {
                    var: binding.clone(),
                    point: drop_pt,
                });
            }
            Statement::Lease {
                binding,
                source,
                duration_ms,
                body,
                ..
            } => {
                let start_pt = self.next_point();
                let start_clock = self.current_clock;
                let end_clock = start_clock.saturating_add(*duration_ms);
                self.push_fact(EntropicFact::LeaseIssued {
                    var: source.clone(),
                    lease_id: binding.clone(),
                    t_start: start_clock,
                    t_end: end_clock,
                    point: start_pt.clone(),
                });
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: binding.clone(),
                    point: start_pt,
                });

                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::Split { parent, branches } => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::BranchSplit {
                    parent: parent.clone(),
                    children: branches.clone(),
                    point: pt,
                });
            }
            Statement::Merge {
                branches, target, ..
            } => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::BranchMerge {
                    branches: branches.clone(),
                    target: target.clone(),
                    point: pt,
                });
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.extract_from_expression(condition, false);
                for s in then_branch {
                    self.extract_from_statement(&s.stmt);
                }
                if let Some(el) = else_branch {
                    for s in el {
                        self.extract_from_statement(&s.stmt);
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
                self.extract_from_expression(expr, false);
                self.extract_from_pattern(pattern);
                for s in then_branch {
                    self.extract_from_statement(&s.stmt);
                }
                if let Some(el) = else_branch {
                    for s in el {
                        self.extract_from_statement(&s.stmt);
                    }
                }
            }
            Statement::Match { target, arms } => {
                self.extract_from_expression(target, false);
                for arm in arms {
                    self.extract_from_pattern(&arm.pattern);
                    if let Some(ref g) = arm.guard {
                        self.extract_from_expression(g, false);
                    }
                    for s in &arm.body {
                        self.extract_from_statement(&s.stmt);
                    }
                }
            }
            Statement::For {
                item_name,
                source,
                body,
                ..
            } => {
                let src_pt = self.next_point();
                self.push_fact(EntropicFact::AccessAt {
                    var: source.clone(),
                    point: src_pt,
                    t_current: self.current_clock,
                });
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: item_name.clone(),
                    point: pt,
                });
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::ForStep {
                item_name,
                source,
                body,
                ..
            } => {
                self.extract_from_expression(source, false);
                let pt = self.next_point();
                self.push_fact(EntropicFact::ValueIntroduced {
                    var: item_name.clone(),
                    point: pt,
                });
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::While {
                condition, body, ..
            } => {
                self.extract_from_expression(condition, false);
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::Loop { body, .. } | Statement::LoopTick { body } => {
                // To model loop back-edges in relational fact extraction,
                // trace iteration 1 then iteration 2.
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::RoutineDef { body, .. } => {
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::Entangle { variables } => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::Entangle {
                    variables: variables.clone(),
                    point: pt,
                });
            }
            Statement::Anchor(name) => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::Anchor {
                    name: name.clone(),
                    clock: self.current_clock,
                    point: pt,
                });
            }
            Statement::Rewind(name) => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::Rewind {
                    target: name.clone(),
                    clock: self.current_clock,
                    point: pt,
                });
            }
            Statement::Yield(expr_opt) => {
                if let Some(expr) = expr_opt {
                    self.extract_from_expression(expr, true);
                }
                let pt = self.next_point();
                self.push_fact(EntropicFact::CausalCommit {
                    clock: self.current_clock,
                    point: pt,
                });
            }
            Statement::RelativisticBlock { body, .. }
            | Statement::DirectiveBlock { body, .. }
            | Statement::DecayHandler { body, .. }
            | Statement::Isolate(causm_core::IsolateBlock { body, .. }) => {
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            Statement::Commit(body) => {
                let pt = self.next_point();
                self.push_fact(EntropicFact::CausalCommit {
                    clock: self.current_clock,
                    point: pt,
                });
                for s in body {
                    self.extract_from_statement(&s.stmt);
                }
            }
            _ => {}
        }
    }

    fn extract_all(mut self, program: &Program) -> ProgramFacts {
        for (t_idx, timeline) in program.timelines.iter().enumerate() {
            self.timeline_idx = t_idx;
            self.statement_idx = 0;
            self.sub_point = 0;

            match timeline.time {
                TimeCoordinate::Global(t) => {
                    self.current_clock = t;
                }
                TimeCoordinate::Relative(dt) => {
                    self.current_clock = self.current_clock.saturating_add(dt);
                }
                TimeCoordinate::Periodic(period) => {
                    self.current_clock = self.current_clock.saturating_add(period);
                }
                TimeCoordinate::Branch(_) => {}
            }

            let mut prev_pt: Option<PointIndex> = None;
            for (s_idx, spanned_stmt) in timeline.statements.iter().enumerate() {
                self.statement_idx = s_idx;
                self.sub_point = 0;

                // Compute line/col from the byte offset in the source.
                let byte_offset = spanned_stmt.span.start;
                let mut line = 1usize;
                let mut col = 1usize;
                let mut seen = 0usize;
                for (l_idx, src_line) in self.source_lines.iter().enumerate() {
                    let line_len = src_line.len() + 1; // +1 for '\n'
                    if seen + line_len > byte_offset {
                        line = l_idx + 1;
                        col = byte_offset - seen + 1;
                        break;
                    }
                    seen += line_len;
                }
                self.current_stmt_line = line;
                self.current_stmt_col = col;
                self.current_stmt_source = self
                    .source_lines
                    .get(line.saturating_sub(1))
                    .cloned()
                    .unwrap_or_default();

                let cur_pt = PointIndex::with_source(
                    self.timeline_idx,
                    self.statement_idx,
                    0,
                    self.filename.clone(),
                    line,
                    col,
                    self.current_stmt_source.clone(),
                );

                if let Some(prev) = prev_pt {
                    self.program_facts.insert_fact(EntropicFact::CfgEdge {
                        from_point: prev,
                        to_point: cur_pt.clone(),
                    });
                }
                prev_pt = Some(cur_pt);

                self.extract_from_statement(&spanned_stmt.stmt);
                // Advance clock by at least 1ms or the statement's base cost
                let stmt_cost = spanned_stmt.stmt.estimate_cost(|_| 1).max(1);
                self.current_clock = self.current_clock.saturating_add(stmt_cost);
            }
        }
        self.program_facts
    }
}

/// Extract relational facts and CFG edge relations from the AST & timeline structure.
///
/// `source` is the full source text; `filename` is the path for diagnostic rendering.
pub fn extract_facts(program: &Program, source: &str, filename: &str) -> ProgramFacts {
    let extractor = FactExtractor::new(source, filename);
    extractor.extract_all(program)
}


/// Extract SSA-level relational facts directly from an SsaCFG basic block graph.
pub fn extract_ssa_facts(ssa_cfg: &SsaCFG) -> ProgramFacts {
    let mut program_facts = ProgramFacts::new();

    for (&block_id, block) in &ssa_cfg.blocks {
        for (inst_idx, phi) in block.phi_nodes.iter().enumerate() {
            let pt = PointIndex::new(block_id as usize, inst_idx, 0);
            program_facts.insert_fact(EntropicFact::ValueIntroduced {
                var: format!("{}", phi.dest),
                point: pt.clone(),
            });
            for (_pred_id, src) in &phi.incoming {
                program_facts.insert_fact(EntropicFact::AccessAt {
                    var: format!("{}", src),
                    point: pt.clone(),
                    t_current: 0,
                });
            }
        }

        for (inst_idx, inst) in block.instructions.iter().enumerate() {
            let pt = PointIndex::new(block_id as usize, inst_idx, 0);
            match inst {
                causm_ir::ssa::SsaInstruction::LoadInt { dest, .. }
                | causm_ir::ssa::SsaInstruction::LoadFloat { dest, .. }
                | causm_ir::ssa::SsaInstruction::LoadBool { dest, .. }
                | causm_ir::ssa::SsaInstruction::LoadString { dest, .. }
                | causm_ir::ssa::SsaInstruction::LoadNull { dest }
                | causm_ir::ssa::SsaInstruction::ConstInt { dest, .. }
                | causm_ir::ssa::SsaInstruction::ConstFloat { dest, .. }
                | causm_ir::ssa::SsaInstruction::ConstBool { dest, .. }
                | causm_ir::ssa::SsaInstruction::ConstString { dest, .. }
                | causm_ir::ssa::SsaInstruction::ConstNull { dest } => {
                    program_facts.insert_fact(EntropicFact::ValueIntroduced {
                        var: format!("{}", dest),
                        point: pt,
                    });
                }
                causm_ir::ssa::SsaInstruction::BinaryOp {
                    dest, left, right, ..
                } => {
                    program_facts.insert_fact(EntropicFact::AccessAt {
                        var: format!("{}", left),
                        point: pt.clone(),
                        t_current: 0,
                    });
                    program_facts.insert_fact(EntropicFact::AccessAt {
                        var: format!("{}", right),
                        point: pt.clone(),
                        t_current: 0,
                    });
                    program_facts.insert_fact(EntropicFact::ValueIntroduced {
                        var: format!("{}", dest),
                        point: pt,
                    });
                }
                causm_ir::ssa::SsaInstruction::UnaryOp { dest, src, .. }
                | causm_ir::ssa::SsaInstruction::Clone { dest, src }
                | causm_ir::ssa::SsaInstruction::Move { dest, src }
                | causm_ir::ssa::SsaInstruction::StrBytes { dest, src }
                | causm_ir::ssa::SsaInstruction::ToStr { dest, src } => {
                    program_facts.insert_fact(EntropicFact::AccessAt {
                        var: format!("{}", src),
                        point: pt.clone(),
                        t_current: 0,
                    });
                    program_facts.insert_fact(EntropicFact::ValueIntroduced {
                        var: format!("{}", dest),
                        point: pt,
                    });
                }
                causm_ir::ssa::SsaInstruction::Consume { src } => {
                    program_facts.insert_fact(EntropicFact::LinearConsume {
                        var: format!("{}", src),
                        point: pt,
                    });
                }
                causm_ir::ssa::SsaInstruction::ConsumeField { src, field } => {
                    program_facts.insert_fact(EntropicFact::FieldConsume {
                        var: format!("{}", src),
                        field: field.clone(),
                        point: pt,
                    });
                }
                causm_ir::ssa::SsaInstruction::Lease {
                    target_reg,
                    source_reg,
                    duration_ms,
                } => {
                    program_facts.insert_fact(EntropicFact::LeaseIssued {
                        var: format!("{}", source_reg),
                        lease_id: format!("{}", target_reg),
                        t_start: 0,
                        t_end: *duration_ms,
                        point: pt.clone(),
                    });
                    program_facts.insert_fact(EntropicFact::ValueIntroduced {
                        var: format!("{}", target_reg),
                        point: pt,
                    });
                }
                causm_ir::ssa::SsaInstruction::Call {
                    routine: _,
                    args,
                    dest,
                } => {
                    for arg in args {
                        program_facts.insert_fact(EntropicFact::AccessAt {
                            var: format!("{}", arg),
                            point: pt.clone(),
                            t_current: 0,
                        });
                    }
                    program_facts.insert_fact(EntropicFact::ValueIntroduced {
                        var: format!("{}", dest),
                        point: pt,
                    });
                }
                _ => {}
            }
        }
    }

    program_facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use causm_core::*;

    #[test]
    fn test_fact_extraction_value_introduced_and_access() {
        let program = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![
                    SpannedStatement::new(
                        Statement::Assignment {
                            target: "x".to_string(),
                            mutable: false,
                            var_type: None,
                            lifetime: None,
                            expr: Expression::Integer(42),
                        },
                        Span { start: 0, end: 10 },
                    ),
                    SpannedStatement::new(
                        Statement::Assignment {
                            target: "y".to_string(),
                            mutable: false,
                            var_type: None,
                            lifetime: None,
                            expr: Expression::Identifier("x".to_string()),
                        },
                        Span { start: 11, end: 20 },
                    ),
                ],
            }],
        };

        let prog_facts = extract_facts(&program, "", "<test>");
        assert!(prog_facts.facts.iter().any(|f| matches!(
            f,
            EntropicFact::ValueIntroduced { var, .. } if var == "x"
        )));
        assert!(prog_facts.facts.iter().any(|f| matches!(
            f,
            EntropicFact::AccessAt { var, .. } if var == "x"
        )));
        assert!(prog_facts.facts.iter().any(|f| matches!(
            f,
            EntropicFact::ValueIntroduced { var, .. } if var == "y"
        )));
    }

    #[test]
    fn test_fact_extraction_temporal_decay() {
        let program = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(10),
                no_z3: false,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Assignment {
                        target: "buf".to_string(),
                        mutable: false,
                        var_type: None,
                        lifetime: Some(LifetimeAnnotation::Decayed(50)),
                        expr: Expression::Integer(100),
                    },
                    Span { start: 0, end: 20 },
                )],
            }],
        };

        let prog_facts = extract_facts(&program, "", "<test>");
        assert!(prog_facts.facts.iter().any(|f| matches!(
            f,
            EntropicFact::TemporalDecay { var, t_expire, .. } if var == "buf" && *t_expire == 60
        )));
    }
}
