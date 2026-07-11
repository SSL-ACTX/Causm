use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::{
    BinaryOperator, Expression, IsolateBlock, Program, SpannedStatement, Statement,
};
use std::collections::{HashMap, HashSet};
use z3::{ast::Bool, ast::Int, Solver};

pub struct FormalVerifier<'a> {
    solver: Solver,
    analyzer: &'a EntropicAnalyzer,
    // Maps variable name to its "is_valid" boolean in Z3
    variable_validity: HashMap<String, Bool>,
    variable_leased: HashMap<String, Bool>,
    anchors: HashMap<String, Int>,
    causal_horizon: Int,
    // Entanglement groups: list of sets of variable names that share entropic state
    entanglements: Vec<HashSet<String>>,
    current_slice_ms: Option<u64>,
}

impl<'a> FormalVerifier<'a> {
    pub fn new(analyzer: &'a EntropicAnalyzer) -> Self {
        Self {
            solver: Solver::new(),
            analyzer,
            variable_validity: HashMap::new(),
            variable_leased: HashMap::new(),
            anchors: HashMap::new(),
            causal_horizon: Int::from_u64(0),
            entanglements: Vec::new(),
            current_slice_ms: None,
        }
    }

    pub fn verify(&mut self, program: &Program) -> Result<(), SemanticError> {
        self.solver.reset();
        self.variable_validity.clear();
        self.variable_leased.clear();
        self.anchors.clear();
        self.causal_horizon = Int::from_u64(0);
        self.entanglements.clear();
        self.current_slice_ms = None;

        for timeline in &program.timelines {
            let mut clock = Int::from_u64(0);
            for spanned in &timeline.statements {
                clock =
                    self.verify_statement(spanned, &Bool::from_bool(true), &clock)?;
            }
        }
        Ok(())
    }

    fn verify_statement(
        &mut self,
        spanned: &SpannedStatement,
        path_condition: &Bool,
        in_clock: &Int,
    ) -> Result<Int, SemanticError> {
        let cost =
            crate::statement::estimate_statement_cost(self.analyzer, &spanned.stmt);
        let current_clock = Int::add(&[in_clock, &Int::from_u64(cost)]);

        match &spanned.stmt {
            Statement::Assignment { target, expr, .. } => {
                self.verify_expression(expr, path_condition, &current_clock)?;
                let is_valid = Bool::new_const(format!(
                    "{}_valid_{}",
                    target, spanned.span.start
                ));
                self.solver.assert(&path_condition.implies(&is_valid));
                self.variable_validity.insert(target.clone(), is_valid);

                let is_leased = Bool::new_const(format!(
                    "{}_leased_{}",
                    target, spanned.span.start
                ));
                self.solver
                    .assert(&path_condition.implies(&is_leased.not()));
                self.variable_leased.insert(target.clone(), is_leased);

                Ok(current_clock)
            }
            Statement::ChannelSend { value_id, .. } | Statement::Yield(value_id) => {
                self.check_available(value_id, path_condition)?;
                self.check_not_leased(value_id, path_condition)?;
                self.consume_variable(value_id, path_condition, spanned.span.start);

                if matches!(&spanned.stmt, Statement::ChannelSend { .. }) {
                    let new_horizon =
                        Int::new_const(format!("horizon_{}", spanned.span.start));
                    self.solver.assert(
                        &path_condition.implies(&new_horizon.eq(&current_clock)),
                    );
                    self.solver.assert(
                        &path_condition
                            .not()
                            .implies(&new_horizon.eq(&self.causal_horizon)),
                    );
                    self.causal_horizon = new_horizon;
                }
                Ok(current_clock)
            }
            Statement::RelativisticBlock { body, .. } => {
                let mut block_clock = in_clock.clone();
                for stmt in body {
                    block_clock =
                        self.verify_statement(stmt, path_condition, &block_clock)?;
                }
                Ok(block_clock)
            }
            Statement::Split { .. } => Ok(current_clock),
            Statement::Merge { .. } => Ok(current_clock),
            Statement::Entangle { variables } => {
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
                Ok(current_clock)
            }
            Statement::Anchor(name) => {
                self.anchors.insert(name.clone(), current_clock.clone());
                Ok(current_clock)
            }
            Statement::Rewind(name)
            | Statement::AcausalReset {
                anchor_name: name, ..
            } => {
                if let Some(anchor_time) = self.anchors.get(name) {
                    let paradox = anchor_time.lt(&self.causal_horizon);
                    self.solver.push();
                    self.solver.assert(&Bool::and(&[path_condition, &paradox]));
                    if self.solver.check() == z3::SatResult::Sat {
                        self.solver.pop(1);
                        return Err(self.analyzer.annotate(SemanticErrorKind::EntropyMismatch(
                            format!("Causal Paradox: Rewind to '{}' violates causal horizon", name)
                        )));
                    }
                    self.solver.pop(1);
                } else {
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::UndefinedVariable(name.clone()),
                    ));
                }
                Ok(current_clock)
            }
            Statement::Isolate(block) => {
                self.verify_isolate(block, path_condition)?;
                Ok(in_clock.clone())
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let cond_bool =
                    self.evaluate_expression_as_bool(condition, path_condition);
                let pre_if_validity = self.variable_validity.clone();
                let pre_if_leased = self.variable_leased.clone();
                let pre_if_horizon = self.causal_horizon.clone();

                let then_pc = Bool::and(&[path_condition, &cond_bool]);
                // Start branches from in_clock + 1 (overhead of the if itself)
                let branch_start_clock = Int::add(&[in_clock, &Int::from_u64(1)]);

                let mut then_clock = branch_start_clock.clone();
                for stmt in then_branch {
                    then_clock =
                        self.verify_statement(stmt, &then_pc, &then_clock)?;
                }
                let post_then_validity = self.variable_validity.clone();
                let post_then_leased = self.variable_leased.clone();
                let post_then_horizon = self.causal_horizon.clone();

                self.variable_validity = pre_if_validity.clone();
                self.variable_leased = pre_if_leased.clone();
                self.causal_horizon = pre_if_horizon;
                let else_pc = Bool::and(&[path_condition, &cond_bool.not()]);
                let mut else_clock = branch_start_clock.clone();
                if let Some(else_stmt) = else_branch {
                    for stmt in else_stmt {
                        else_clock =
                            self.verify_statement(stmt, &else_pc, &else_clock)?;
                    }
                }
                let post_else_validity = self.variable_validity.clone();
                let post_else_leased = self.variable_leased.clone();
                let post_else_horizon = self.causal_horizon.clone();

                let mut merged_validity = HashMap::new();
                let mut merged_leased = HashMap::new();
                for var in pre_if_validity.keys() {
                    let v_then = post_then_validity.get(var).unwrap();
                    let v_else = post_else_validity.get(var).unwrap();
                    let l_then = post_then_leased.get(var).unwrap();
                    let l_else = post_else_leased.get(var).unwrap();
                    let m_v = Bool::new_const(format!(
                        "{}_m_v_{}",
                        var, spanned.span.start
                    ));
                    self.solver
                        .assert(&m_v.eq(&Bool::ite(&cond_bool, v_then, v_else)));
                    merged_validity.insert(var.clone(), m_v);
                    let m_l = Bool::new_const(format!(
                        "{}_m_l_{}",
                        var, spanned.span.start
                    ));
                    self.solver
                        .assert(&m_l.eq(&Bool::ite(&cond_bool, l_then, l_else)));
                    merged_leased.insert(var.clone(), m_l);
                }
                self.variable_validity = merged_validity;
                self.variable_leased = merged_leased;
                let m_h = Int::new_const(format!("h_m_{}", spanned.span.start));
                self.solver.assert(&m_h.eq(&Bool::ite(
                    &cond_bool,
                    &post_then_horizon,
                    &post_else_horizon,
                )));
                self.causal_horizon = m_h;

                Ok(Bool::ite(&cond_bool, &then_clock, &else_clock))
            }
            Statement::MatchEntropy {
                target,
                valid_branch,
                decayed_branch,
                pending_branch,
                consumed_branch,
            } => {
                self.verify_expression(target, path_condition, &current_clock)?;

                let valid_cond =
                    Bool::new_const(format!("valid_cond_{}", spanned.span.start));
                let decayed_cond =
                    Bool::new_const(format!("decayed_cond_{}", spanned.span.start));
                let pending_cond =
                    Bool::new_const(format!("pending_cond_{}", spanned.span.start));
                let consumed_cond =
                    Bool::new_const(format!("consumed_cond_{}", spanned.span.start));

                let one_of_four = Bool::or(&[
                    &Bool::and(&[
                        &valid_cond,
                        &decayed_cond.not(),
                        &pending_cond.not(),
                        &consumed_cond.not(),
                    ]),
                    &Bool::and(&[
                        &valid_cond.not(),
                        &decayed_cond,
                        &pending_cond.not(),
                        &consumed_cond.not(),
                    ]),
                    &Bool::and(&[
                        &valid_cond.not(),
                        &decayed_cond.not(),
                        &pending_cond,
                        &consumed_cond.not(),
                    ]),
                    &Bool::and(&[
                        &valid_cond.not(),
                        &decayed_cond.not(),
                        &pending_cond.not(),
                        &consumed_cond,
                    ]),
                ]);
                self.solver.assert(&path_condition.implies(&one_of_four));

                let pre_match_validity = self.variable_validity.clone();
                let pre_match_leased = self.variable_leased.clone();
                let pre_match_horizon = self.causal_horizon.clone();

                let branch_start_clock = Int::add(&[in_clock, &Int::from_u64(1)]);

                // 1. Valid branch
                let mut valid_clock = branch_start_clock.clone();
                if let Some((binding, branch_body)) = valid_branch {
                    let valid_pc = Bool::and(&[path_condition, &valid_cond]);
                    if !binding.is_empty() {
                        let is_valid = Bool::new_const(format!(
                            "{}_valid_{}",
                            binding, spanned.span.start
                        ));
                        self.solver.assert(&valid_pc.implies(&is_valid));
                        self.variable_validity.insert(binding.clone(), is_valid);
                        let is_leased = Bool::new_const(format!(
                            "{}_leased_{}",
                            binding, spanned.span.start
                        ));
                        self.solver.assert(&valid_pc.implies(&is_leased.not()));
                        self.variable_leased.insert(binding.clone(), is_leased);
                    }

                    for stmt in branch_body {
                        valid_clock =
                            self.verify_statement(stmt, &valid_pc, &valid_clock)?;
                    }
                }
                let post_val_validity = self.variable_validity.clone();
                let post_val_leased = self.variable_leased.clone();
                let post_val_horizon = self.causal_horizon.clone();

                // 2. Decayed branch
                self.variable_validity = pre_match_validity.clone();
                self.variable_leased = pre_match_leased.clone();
                self.causal_horizon = pre_match_horizon.clone();
                let mut decayed_clock = branch_start_clock.clone();
                if let Some((binding, branch_body)) = decayed_branch {
                    let decayed_pc = Bool::and(&[path_condition, &decayed_cond]);
                    if !binding.is_empty() {
                        let is_valid = Bool::new_const(format!(
                            "{}_valid_{}",
                            binding, spanned.span.start
                        ));
                        self.solver.assert(&decayed_pc.implies(&is_valid));
                        self.variable_validity.insert(binding.clone(), is_valid);
                        let is_leased = Bool::new_const(format!(
                            "{}_leased_{}",
                            binding, spanned.span.start
                        ));
                        self.solver.assert(&decayed_pc.implies(&is_leased.not()));
                        self.variable_leased.insert(binding.clone(), is_leased);
                    }

                    for stmt in branch_body {
                        decayed_clock = self.verify_statement(
                            stmt,
                            &decayed_pc,
                            &decayed_clock,
                        )?;
                    }
                }
                let post_dec_validity = self.variable_validity.clone();
                let post_dec_leased = self.variable_leased.clone();
                let post_dec_horizon = self.causal_horizon.clone();

                // 3. Pending branch
                self.variable_validity = pre_match_validity.clone();
                self.variable_leased = pre_match_leased.clone();
                self.causal_horizon = pre_match_horizon.clone();
                let mut pending_clock = branch_start_clock.clone();
                if let Some(branch_body) = pending_branch {
                    let pending_pc = Bool::and(&[path_condition, &pending_cond]);
                    for stmt in branch_body {
                        pending_clock = self.verify_statement(
                            stmt,
                            &pending_pc,
                            &pending_clock,
                        )?;
                    }
                }
                let post_pen_validity = self.variable_validity.clone();
                let post_pen_leased = self.variable_leased.clone();
                let post_pen_horizon = self.causal_horizon.clone();

                // 4. Consumed branch
                self.variable_validity = pre_match_validity.clone();
                self.variable_leased = pre_match_leased.clone();
                self.causal_horizon = pre_match_horizon.clone();
                let mut consumed_clock = branch_start_clock.clone();
                if let Some(branch_body) = consumed_branch {
                    let consumed_pc = Bool::and(&[path_condition, &consumed_cond]);
                    for stmt in branch_body {
                        consumed_clock = self.verify_statement(
                            stmt,
                            &consumed_pc,
                            &consumed_clock,
                        )?;
                    }
                }
                let post_con_validity = self.variable_validity.clone();
                let post_con_leased = self.variable_leased.clone();
                let post_con_horizon = self.causal_horizon.clone();

                // Merge
                let mut merged_validity = HashMap::new();
                let mut merged_leased = HashMap::new();

                let mut all_vars = HashSet::new();
                for candidate in &[
                    &post_val_validity,
                    &post_dec_validity,
                    &post_pen_validity,
                    &post_con_validity,
                ] {
                    all_vars.extend(candidate.keys().cloned());
                }

                for var in all_vars {
                    let v_val = post_val_validity
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));
                    let v_dec = post_dec_validity
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));
                    let v_pen = post_pen_validity
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));
                    let v_con = post_con_validity
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));

                    let m_v = Bool::new_const(format!(
                        "{}_m_v_{}",
                        var, spanned.span.start
                    ));
                    let val_expr = Bool::or(&[
                        &Bool::and(&[&valid_cond, &v_val]),
                        &Bool::and(&[&decayed_cond, &v_dec]),
                        &Bool::and(&[&pending_cond, &v_pen]),
                        &Bool::and(&[&consumed_cond, &v_con]),
                    ]);
                    self.solver.assert(&m_v.eq(&val_expr));
                    merged_validity.insert(var.clone(), m_v);

                    let l_val = post_val_leased
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));
                    let l_dec = post_dec_leased
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));
                    let l_pen = post_pen_leased
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));
                    let l_con = post_con_leased
                        .get(&var)
                        .cloned()
                        .unwrap_or_else(|| Bool::from_bool(false));

                    let m_l = Bool::new_const(format!(
                        "{}_m_l_{}",
                        var, spanned.span.start
                    ));
                    let leased_expr = Bool::or(&[
                        &Bool::and(&[&valid_cond, &l_val]),
                        &Bool::and(&[&decayed_cond, &l_dec]),
                        &Bool::and(&[&pending_cond, &l_pen]),
                        &Bool::and(&[&consumed_cond, &l_con]),
                    ]);
                    self.solver.assert(&m_l.eq(&leased_expr));
                    merged_leased.insert(var.clone(), m_l);
                }

                self.variable_validity = merged_validity;
                self.variable_leased = merged_leased;

                let m_h = Int::new_const(format!("h_m_{}", spanned.span.start));
                let h_expr = Bool::ite(
                    &valid_cond,
                    &post_val_horizon,
                    &Bool::ite(
                        &decayed_cond,
                        &post_dec_horizon,
                        &Bool::ite(
                            &pending_cond,
                            &post_pen_horizon,
                            &post_con_horizon,
                        ),
                    ),
                );
                self.solver.assert(&m_h.eq(&h_expr));
                self.causal_horizon = m_h;

                let final_clock = Bool::ite(
                    &valid_cond,
                    &valid_clock,
                    &Bool::ite(
                        &decayed_cond,
                        &decayed_clock,
                        &Bool::ite(&pending_cond, &pending_clock, &consumed_clock),
                    ),
                );

                Ok(final_clock)
            }
            Statement::Loop { max_ms, body } => {
                let mut loop_clock = Int::from_u64(0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }
                let mut unroll_clock = loop_clock.clone();
                for stmt in body {
                    unroll_clock =
                        self.verify_statement(stmt, path_condition, &unroll_clock)?;
                }
                let violation = loop_clock.gt(&Int::from_u64(*max_ms));
                self.solver.push();
                self.solver
                    .assert(&Bool::and(&[path_condition, &violation]));
                if self.solver.check() == z3::SatResult::Sat {
                    let model = self.solver.get_model().unwrap();
                    let actual_wcet = model
                        .eval(&loop_clock, true)
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::TemporalAssertionViolation(
                            actual_wcet,
                            *max_ms,
                        ),
                    ));
                }
                self.solver.pop(1);
                Ok(Int::add(&[in_clock, &Int::from_u64(*max_ms)]))
            }
            Statement::LoopTick { body } => {
                let slice_ms = self.current_slice_ms.unwrap_or(0);
                let mut loop_clock = Int::from_u64(0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }
                let mut unroll_clock = loop_clock.clone();
                for stmt in body {
                    unroll_clock =
                        self.verify_statement(stmt, path_condition, &unroll_clock)?;
                }
                Ok(Int::add(&[in_clock, &Int::from_u64(slice_ms)]))
            }
            Statement::Slice { milliseconds } => {
                self.current_slice_ms = Some(*milliseconds);
                Ok(in_clock.clone())
            }
            Statement::For {
                item_name,
                mode,
                source,
                body,
                pacing_ms,
                ..
            } => {
                if let causm_core::ForMode::Consume = mode {
                    self.check_available(source, path_condition)?;
                    self.consume_variable(
                        source,
                        path_condition,
                        spanned.span.start,
                    );
                }
                let mut loop_clock = Int::from_u64(0);
                {
                    let item_valid = Bool::new_const(format!(
                        "{}_v1_{}",
                        item_name, spanned.span.start
                    ));
                    self.solver.assert(&path_condition.implies(&item_valid));
                    self.variable_validity.insert(item_name.clone(), item_valid);
                    for stmt in body {
                        loop_clock = self.verify_statement(
                            stmt,
                            path_condition,
                            &loop_clock,
                        )?;
                    }
                }
                {
                    let item_valid = Bool::new_const(format!(
                        "{}_v2_{}",
                        item_name, spanned.span.start
                    ));
                    self.solver.assert(&path_condition.implies(&item_valid));
                    self.variable_validity.insert(item_name.clone(), item_valid);
                    let mut unroll_clock = loop_clock.clone();
                    for stmt in body {
                        unroll_clock = self.verify_statement(
                            stmt,
                            path_condition,
                            &unroll_clock,
                        )?;
                    }
                }
                self.variable_validity.remove(item_name);
                if let Some(pacing) = pacing_ms {
                    let violation = loop_clock.gt(&Int::from_u64(*pacing));
                    self.solver.push();
                    self.solver
                        .assert(&Bool::and(&[path_condition, &violation]));
                    if self.solver.check() == z3::SatResult::Sat {
                        self.solver.pop(1);
                        return Err(self
                            .analyzer
                            .annotate(SemanticErrorKind::PacingViolation));
                    }
                    self.solver.pop(1);
                }
                Ok(in_clock.clone())
            }
            Statement::RoutineDef {
                params,
                taking_ms,
                body,
                ..
            } => {
                let mut routine_verifier = FormalVerifier::new(self.analyzer);
                for p in params {
                    let is_valid = Bool::new_const(format!(
                        "{}_p_{}",
                        p.name, spanned.span.start
                    ));
                    routine_verifier
                        .solver
                        .assert(&is_valid.eq(&Bool::from_bool(true)));
                    routine_verifier
                        .variable_validity
                        .insert(p.name.clone(), is_valid);
                }
                let mut body_clock = Int::from_u64(0);
                for stmt in body {
                    body_clock = routine_verifier.verify_statement(
                        stmt,
                        &Bool::from_bool(true),
                        &body_clock,
                    )?;
                }
                if let Some(limit) = taking_ms {
                    let violation = body_clock.gt(&Int::from_u64(*limit));
                    routine_verifier.solver.push();
                    routine_verifier.solver.assert(&violation);
                    if routine_verifier.solver.check() == z3::SatResult::Sat {
                        let model = routine_verifier.solver.get_model().unwrap();
                        let actual_wcet = model
                            .eval(&body_clock, true)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        routine_verifier.solver.pop(1);
                        return Err(self.analyzer.annotate(
                            SemanticErrorKind::TemporalAssertionViolation(
                                actual_wcet,
                                *limit,
                            ),
                        ));
                    }
                    routine_verifier.solver.pop(1);
                }
                Ok(in_clock.clone())
            }
            Statement::AssertTime {
                operator, limit_ms, ..
            } => {
                let violation = match operator {
                    BinaryOperator::Gt => in_clock.le(&Int::from_u64(*limit_ms)),
                    BinaryOperator::Lt => in_clock.ge(&Int::from_u64(*limit_ms)),
                    BinaryOperator::Ge => in_clock.lt(&Int::from_u64(*limit_ms)),
                    BinaryOperator::Le => in_clock.gt(&Int::from_u64(*limit_ms)),
                    BinaryOperator::Eq => {
                        in_clock.eq(&Int::from_u64(*limit_ms)).not()
                    }
                    BinaryOperator::Neq => in_clock.eq(&Int::from_u64(*limit_ms)),
                    _ => Bool::from_bool(false),
                };
                self.solver.push();
                self.solver
                    .assert(&Bool::and(&[path_condition, &violation]));
                if self.solver.check() == z3::SatResult::Sat {
                    let model = self.solver.get_model().unwrap();
                    let actual_wcet = model
                        .eval(in_clock, true)
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::TemporalAssertionViolation(
                            actual_wcet,
                            *limit_ms,
                        ),
                    ));
                }
                self.solver.pop(1);
                Ok(current_clock)
            }
            Statement::Lease {
                binding,
                source,
                duration_ms,
                body,
            } => {
                self.check_available(source, path_condition)?;
                let pre_lease_validity = self.variable_validity.clone();
                let pre_lease_leased = self.variable_leased.clone();
                let is_valid = Bool::new_const(format!(
                    "{}_valid_{}",
                    binding, spanned.span.start
                ));
                self.solver.assert(&path_condition.implies(&is_valid));
                self.variable_validity.insert(binding.clone(), is_valid);
                let is_leased = Bool::new_const(format!(
                    "{}_leased_{}",
                    binding, spanned.span.start
                ));
                self.solver.assert(&path_condition.implies(&is_leased));
                self.variable_leased.insert(binding.clone(), is_leased);

                let mut body_clock = Int::from_u64(0);
                for stmt in body {
                    body_clock =
                        self.verify_statement(stmt, path_condition, &body_clock)?;
                }
                let violation = body_clock.gt(&Int::from_u64(*duration_ms));
                self.solver.push();
                self.solver
                    .assert(&Bool::and(&[path_condition, &violation]));
                if self.solver.check() == z3::SatResult::Sat {
                    let model = self.solver.get_model().unwrap();
                    let actual_wcet = model
                        .eval(&body_clock, true)
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::LeaseDurationExceeded(
                            actual_wcet,
                            *duration_ms,
                        ),
                    ));
                }
                self.solver.pop(1);
                self.variable_validity = pre_lease_validity;
                self.variable_leased = pre_lease_leased;
                Ok(Int::add(&[in_clock, &Int::from_u64(*duration_ms)]))
            }
            Statement::Expression(expr)
            | Statement::Print(expr)
            | Statement::Debug(expr) => {
                self.verify_expression(expr, path_condition, &current_clock)
            }
            _ => Ok(current_clock),
        }
    }

    fn consume_variable(
        &mut self,
        name: &str,
        path_condition: &Bool,
        span_start: usize,
    ) {
        let new_valid = Bool::new_const(format!("{}_consumed_{}", name, span_start));
        self.solver
            .assert(&path_condition.implies(&new_valid.not()));
        self.variable_validity.insert(name.to_string(), new_valid);
        let mut entangled_to_mark = Vec::new();
        for set in &self.entanglements {
            if set.contains(name) {
                for other in set {
                    if other != name {
                        entangled_to_mark.push(other.clone());
                    }
                }
            }
        }
        for other in entangled_to_mark {
            let other_valid = Bool::new_const(format!(
                "{}_decayed_by_{}_{}",
                other, name, span_start
            ));
            self.solver
                .assert(&path_condition.implies(&other_valid.not()));
            self.variable_validity.insert(other, other_valid);
        }
    }

    fn check_available(
        &mut self,
        name: &str,
        path_condition: &Bool,
    ) -> Result<(), SemanticError> {
        if let Some(valid_bool) = self.variable_validity.get(name) {
            self.solver.push();
            self.solver
                .assert(&Bool::and(&[path_condition, &valid_bool.not()]));
            if self.solver.check() == z3::SatResult::Sat {
                self.solver.pop(1);
                return Err(self.analyzer.annotate(
                    SemanticErrorKind::UseAfterConsume(name.to_string()),
                ));
            }
            self.solver.pop(1);
        } else {
            return Err(self
                .analyzer
                .annotate(SemanticErrorKind::UndefinedVariable(name.to_string())));
        }
        Ok(())
    }

    fn check_not_leased(
        &mut self,
        name: &str,
        path_condition: &Bool,
    ) -> Result<(), SemanticError> {
        if let Some(leased_bool) = self.variable_leased.get(name) {
            self.solver.push();
            self.solver
                .assert(&Bool::and(&[path_condition, leased_bool]));
            if self.solver.check() == z3::SatResult::Sat {
                self.solver.pop(1);
                return Err(self
                    .analyzer
                    .annotate(SemanticErrorKind::LeaseViolation(name.to_string())));
            }
            self.solver.pop(1);
        }
        Ok(())
    }

    fn evaluate_expression_as_bool(
        &mut self,
        expr: &Expression,
        _path_condition: &Bool,
    ) -> Bool {
        match expr {
            Expression::Boolean(b) => Bool::from_bool(*b),
            _ => Bool::new_const(format!("expr_bool_{:?}", expr)),
        }
    }

    fn verify_expression(
        &mut self,
        expr: &Expression,
        path_condition: &Bool,
        in_clock: &Int,
    ) -> Result<Int, SemanticError> {
        match expr {
            Expression::Identifier(name) => {
                self.check_available(name, path_condition)?;
                Ok(in_clock.clone())
            }
            Expression::BinaryOp { left, right, .. } => {
                self.verify_expression(left, path_condition, in_clock)?;
                self.verify_expression(right, path_condition, in_clock)?;
                Ok(in_clock.clone())
            }
            Expression::UnaryOp { expr, .. } => {
                self.verify_expression(expr, path_condition, in_clock)?;
                Ok(in_clock.clone())
            }
            Expression::Call { routine, args } => {
                for arg in args {
                    self.verify_expression(arg, path_condition, in_clock)?;
                }
                if let Some(info) = self.analyzer.routines.get(routine) {
                    Ok(Int::add(&[in_clock, &Int::from_u64(info.taking_ms)]))
                } else {
                    Ok(in_clock.clone())
                }
            }
            _ => Ok(in_clock.clone()),
        }
    }

    fn verify_isolate(
        &mut self,
        block: &IsolateBlock,
        path_condition: &Bool,
    ) -> Result<(), SemanticError> {
        let budget = block.manifest.cpu_budget_ms.unwrap_or(u64::MAX);
        let mut clock = Int::from_u64(0);
        let old_anchors = self.anchors.clone();
        let old_slice = self.current_slice_ms;

        self.current_slice_ms = block.manifest.slice_ms;

        for spanned in &block.body {
            clock = self.verify_statement(spanned, path_condition, &clock)?;
        }
        self.anchors = old_anchors;
        self.current_slice_ms = old_slice;

        let violation = clock.gt(&Int::from_u64(budget));
        self.solver.push();
        self.solver
            .assert(&Bool::and(&[path_condition, &violation]));
        if self.solver.check() == z3::SatResult::Sat {
            self.solver.pop(1);
            return Err(self.analyzer.annotate(
                SemanticErrorKind::TemporalAssertionViolation(0, budget),
            ));
        }
        self.solver.pop(1);
        Ok(())
    }
}
