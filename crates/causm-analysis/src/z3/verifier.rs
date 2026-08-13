use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::solver::SolverBackend;
use causm_core::{
    BinaryOperator, BlockDirective, DecayedPattern, Expression, IsolateBlock,
    Program, SpannedStatement, Statement,
};
use std::collections::{HashMap, HashSet};

pub struct FormalVerifier<'a, S: SolverBackend = super::backend::Z3Backend> {
    solver: S,
    analyzer: &'a EntropicAnalyzer,
    variable_validity: HashMap<String, S::Bool>,
    variable_leased: HashMap<String, S::Bool>,
    anchors: HashMap<String, S::Int>,
    causal_horizon: S::Int,
    entanglements: Vec<HashSet<String>>,
    current_slice_ms: Option<u64>,
    in_entropy_match: bool,
}

impl<'a, S: SolverBackend> FormalVerifier<'a, S> {
    pub fn new(analyzer: &'a EntropicAnalyzer) -> Self {
        let mut solver = S::new();
        let causal_horizon = solver.int_from_u64(0);
        Self {
            solver,
            analyzer,
            variable_validity: HashMap::new(),
            variable_leased: HashMap::new(),
            anchors: HashMap::new(),
            causal_horizon,
            entanglements: Vec::new(),
            current_slice_ms: None,
            in_entropy_match: false,
        }
    }

    pub fn verify(&mut self, program: &Program) -> Result<(), SemanticError> {
        self.solver.reset();
        self.variable_validity.clear();
        self.variable_leased.clear();
        self.anchors.clear();
        self.causal_horizon = self.solver.int_from_u64(0);
        self.entanglements.clear();
        self.current_slice_ms = None;

        for (idx, timeline) in program.timelines.iter().enumerate() {
            if timeline.no_z3 {
                self.analyzer
                    .analyzed_wcet
                    .borrow_mut()
                    .insert(format!("Timeline {}", idx), 0);
                continue;
            }
            let mut clock = self.solver.int_from_u64(0);
            let path_cond = self.solver.bool_from_bool(true);
            for spanned in &timeline.statements {
                clock = self.verify_statement(spanned, &path_cond, &clock)?;
            }
            let wcet = self.find_max_value(&clock, &path_cond);
            self.analyzer
                .analyzed_wcet
                .borrow_mut()
                .insert(format!("Timeline {}", idx), wcet);
        }
        Ok(())
    }

    pub fn find_max_value(&mut self, val: &S::Int, path_cond: &S::Bool) -> u64 {
        self.solver.push();
        self.solver.assert(path_cond);

        let mut low = 0;
        let mut high = 100_000;

        if !self.solver.check() {
            self.solver.pop(1);
            return 0;
        }

        if let Some(initial_val) = self.solver.eval_u64(val) {
            low = initial_val;
            if low > high {
                high = low * 2;
            }
        }

        let mut max_val = low;

        while low <= high {
            let mid = low + (high - low) / 2;
            self.solver.push();
            let mid_int = self.solver.int_from_u64(mid);
            let check_gt = self.solver.int_ge(val, &mid_int);
            self.solver.assert(&check_gt);

            if self.solver.check() {
                max_val = mid;
                if let Some(eval_val) = self.solver.eval_u64(val) {
                    if eval_val > max_val {
                        max_val = eval_val;
                        low = eval_val + 1;
                        self.solver.pop(1);
                        continue;
                    }
                }
                low = mid + 1;
            } else {
                high = mid - 1;
            }
            self.solver.pop(1);
        }

        self.solver.pop(1);
        max_val
    }

    fn verify_statement(
        &mut self,
        spanned: &SpannedStatement,
        path_condition: &S::Bool,
        in_clock: &S::Int,
    ) -> Result<S::Int, SemanticError> {
        let cost =
            crate::statement::estimate_statement_cost(self.analyzer, &spanned.stmt);
        let cost_int = self.solver.int_from_u64(cost);
        let current_clock = self.solver.int_add(&[in_clock, &cost_int]);

        match &spanned.stmt {
            Statement::Assignment { target, expr, .. } => {
                self.verify_expression(expr, path_condition, &current_clock)?;
                let is_valid = self
                    .solver
                    .bool_const(&format!("{}_valid_{}", target, spanned.span.start));
                let impl_valid = self.solver.bool_implies(path_condition, &is_valid);
                self.solver.assert(&impl_valid);
                self.variable_validity.insert(target.clone(), is_valid);

                let is_leased = self.solver.bool_const(&format!(
                    "{}_leased_{}",
                    target, spanned.span.start
                ));
                let not_leased = self.solver.bool_not(&is_leased);
                let impl_not_leased =
                    self.solver.bool_implies(path_condition, &not_leased);
                self.solver.assert(&impl_not_leased);
                self.variable_leased.insert(target.clone(), is_leased);

                Ok(current_clock)
            }
            Statement::ChannelSend { value_id, .. } | Statement::Yield(value_id) => {
                self.check_available(value_id, path_condition)?;
                self.check_not_leased(value_id, path_condition)?;
                self.consume_variable(value_id, path_condition, spanned.span.start);

                if matches!(&spanned.stmt, Statement::ChannelSend { .. }) {
                    let new_horizon = self
                        .solver
                        .int_const(&format!("horizon_{}", spanned.span.start));
                    let eq_clock = self.solver.int_eq(&new_horizon, &current_clock);
                    let impl_eq_clock =
                        self.solver.bool_implies(path_condition, &eq_clock);
                    self.solver.assert(&impl_eq_clock);
                    let eq_horizon =
                        self.solver.int_eq(&new_horizon, &self.causal_horizon);
                    let not_pc = self.solver.bool_not(path_condition);
                    let impl_eq_horizon =
                        self.solver.bool_implies(&not_pc, &eq_horizon);
                    self.solver.assert(&impl_eq_horizon);
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
                    let paradox =
                        self.solver.int_lt(anchor_time, &self.causal_horizon);
                    self.solver.push();
                    let cond_and = self.solver.bool_and(&[path_condition, &paradox]);
                    self.solver.assert(&cond_and);
                    if self.solver.check() {
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
            Statement::DirectiveBlock { directives, body } => {
                let mut bypass_z3 = false;
                for dir in directives {
                    if matches!(dir, BlockDirective::NoZ3) {
                        bypass_z3 = true;
                    }
                }
                if bypass_z3 {
                    Ok(current_clock)
                } else {
                    let mut clock = current_clock;
                    for stmt in body {
                        clock =
                            self.verify_statement(stmt, path_condition, &clock)?;
                    }
                    Ok(clock)
                }
            }
            Statement::Isolate(block) => {
                self.verify_isolate(block, path_condition)?;
                Ok(in_clock.clone())
            }
            Statement::If {
                binding,
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

                let then_pc = self.solver.bool_and(&[path_condition, &cond_bool]);
                let one_int = self.solver.int_from_u64(1);
                let branch_start_clock = self.solver.int_add(&[in_clock, &one_int]);

                if let Some(binding_name) = binding {
                    let is_valid = self.solver.bool_from_bool(true);
                    let is_leased = self.solver.bool_from_bool(false);
                    self.variable_validity
                        .insert(binding_name.clone(), is_valid);
                    self.variable_leased.insert(binding_name.clone(), is_leased);
                }

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
                let not_cond = self.solver.bool_not(&cond_bool);
                let else_pc = self.solver.bool_and(&[path_condition, &not_cond]);
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

                    let m_v = self
                        .solver
                        .bool_const(&format!("{}_m_v_{}", var, spanned.span.start));
                    let ite_v = self.solver.bool_ite(&cond_bool, v_then, v_else);
                    let eq_v = self.solver.bool_eq(&m_v, &ite_v);
                    self.solver.assert(&eq_v);
                    merged_validity.insert(var.clone(), m_v);

                    let m_l = self
                        .solver
                        .bool_const(&format!("{}_m_l_{}", var, spanned.span.start));
                    let ite_l = self.solver.bool_ite(&cond_bool, l_then, l_else);
                    let eq_l = self.solver.bool_eq(&m_l, &ite_l);
                    self.solver.assert(&eq_l);
                    merged_leased.insert(var.clone(), m_l);
                }
                self.variable_validity = merged_validity;
                self.variable_leased = merged_leased;
                let m_h = self
                    .solver
                    .int_const(&format!("h_m_{}", spanned.span.start));
                let ite_h = self.solver.int_ite(
                    &cond_bool,
                    &post_then_horizon,
                    &post_else_horizon,
                );
                let eq_h = self.solver.int_eq(&m_h, &ite_h);
                self.solver.assert(&eq_h);
                self.causal_horizon = m_h;

                Ok(self.solver.int_ite(&cond_bool, &then_clock, &else_clock))
            }
            Statement::MatchEntropy {
                target,
                valid_branch,
                decayed_branch,
                pending_branch,
                consumed_branch,
            } => {
                self.in_entropy_match = true;
                let res =
                    self.verify_expression(target, path_condition, &current_clock);
                self.in_entropy_match = false;
                res?;

                let valid_cond = self
                    .solver
                    .bool_const(&format!("valid_cond_{}", spanned.span.start));
                let decayed_cond = self
                    .solver
                    .bool_const(&format!("decayed_cond_{}", spanned.span.start));
                let pending_cond = self
                    .solver
                    .bool_const(&format!("pending_cond_{}", spanned.span.start));
                let consumed_cond = self
                    .solver
                    .bool_const(&format!("consumed_cond_{}", spanned.span.start));

                let not_dec = self.solver.bool_not(&decayed_cond);
                let not_pen = self.solver.bool_not(&pending_cond);
                let not_con = self.solver.bool_not(&consumed_cond);
                let not_val = self.solver.bool_not(&valid_cond);

                let case_valid = self.solver.bool_and(&[
                    &valid_cond,
                    &not_dec,
                    &not_pen,
                    &not_con,
                ]);
                let case_decayed = self.solver.bool_and(&[
                    &not_val,
                    &decayed_cond,
                    &not_pen,
                    &not_con,
                ]);
                let case_pending = self.solver.bool_and(&[
                    &not_val,
                    &not_dec,
                    &pending_cond,
                    &not_con,
                ]);
                let case_consumed = self.solver.bool_and(&[
                    &not_val,
                    &not_dec,
                    &not_pen,
                    &consumed_cond,
                ]);

                let one_of_four = self.solver.bool_or(&[
                    &case_valid,
                    &case_decayed,
                    &case_pending,
                    &case_consumed,
                ]);
                let impl_one_of_four =
                    self.solver.bool_implies(path_condition, &one_of_four);
                self.solver.assert(&impl_one_of_four);

                let pre_match_validity = self.variable_validity.clone();
                let pre_match_leased = self.variable_leased.clone();
                let pre_match_horizon = self.causal_horizon.clone();

                let one_int = self.solver.int_from_u64(1);
                let branch_start_clock = self.solver.int_add(&[in_clock, &one_int]);

                // Valid branch
                let mut valid_clock = branch_start_clock.clone();
                if let Some((pattern, branch_body)) = valid_branch {
                    let valid_pc =
                        self.solver.bool_and(&[path_condition, &valid_cond]);
                    if let DecayedPattern::Binding(binding) = pattern {
                        if !binding.is_empty() {
                            let is_valid = self.solver.bool_const(&format!(
                                "{}_valid_{}",
                                binding, spanned.span.start
                            ));
                            let impl_is_valid =
                                self.solver.bool_implies(&valid_pc, &is_valid);
                            self.solver.assert(&impl_is_valid);
                            self.variable_validity.insert(binding.clone(), is_valid);

                            let is_leased = self.solver.bool_const(&format!(
                                "{}_leased_{}",
                                binding, spanned.span.start
                            ));
                            let not_leased = self.solver.bool_not(&is_leased);
                            let impl_not_leased =
                                self.solver.bool_implies(&valid_pc, &not_leased);
                            self.solver.assert(&impl_not_leased);
                            self.variable_leased.insert(binding.clone(), is_leased);
                        }
                    }

                    for stmt in branch_body {
                        valid_clock =
                            self.verify_statement(stmt, &valid_pc, &valid_clock)?;
                    }
                }
                let post_val_validity = self.variable_validity.clone();
                let post_val_leased = self.variable_leased.clone();
                let post_val_horizon = self.causal_horizon.clone();

                // Decayed branch
                self.variable_validity = pre_match_validity.clone();
                self.variable_leased = pre_match_leased.clone();
                self.causal_horizon = pre_match_horizon.clone();
                let mut decayed_clock = branch_start_clock.clone();
                if let Some((pattern, branch_body)) = decayed_branch {
                    let decayed_pc =
                        self.solver.bool_and(&[path_condition, &decayed_cond]);
                    if let DecayedPattern::Binding(binding) = pattern {
                        if !binding.is_empty() {
                            let is_valid = self.solver.bool_const(&format!(
                                "{}_valid_{}",
                                binding, spanned.span.start
                            ));
                            let impl_is_valid =
                                self.solver.bool_implies(&decayed_pc, &is_valid);
                            self.solver.assert(&impl_is_valid);
                            self.variable_validity.insert(binding.clone(), is_valid);

                            let is_leased = self.solver.bool_const(&format!(
                                "{}_leased_{}",
                                binding, spanned.span.start
                            ));
                            let not_leased = self.solver.bool_not(&is_leased);
                            let impl_not_leased =
                                self.solver.bool_implies(&decayed_pc, &not_leased);
                            self.solver.assert(&impl_not_leased);
                            self.variable_leased.insert(binding.clone(), is_leased);
                        }
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

                // Pending branch
                self.variable_validity = pre_match_validity.clone();
                self.variable_leased = pre_match_leased.clone();
                self.causal_horizon = pre_match_horizon.clone();
                let mut pending_clock = branch_start_clock.clone();
                if let Some((pattern, branch_body)) = pending_branch {
                    let pending_pc =
                        self.solver.bool_and(&[path_condition, &pending_cond]);
                    if let DecayedPattern::Binding(binding) = pattern {
                        if !binding.is_empty() {
                            let is_valid = self.solver.bool_const(&format!(
                                "{}_valid_{}",
                                binding, spanned.span.start
                            ));
                            let impl_is_valid =
                                self.solver.bool_implies(&pending_pc, &is_valid);
                            self.solver.assert(&impl_is_valid);
                            self.variable_validity.insert(binding.clone(), is_valid);

                            let is_leased = self.solver.bool_const(&format!(
                                "{}_leased_{}",
                                binding, spanned.span.start
                            ));
                            let not_leased = self.solver.bool_not(&is_leased);
                            let impl_not_leased =
                                self.solver.bool_implies(&pending_pc, &not_leased);
                            self.solver.assert(&impl_not_leased);
                            self.variable_leased.insert(binding.clone(), is_leased);
                        }
                    }

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

                // Consumed branch
                self.variable_validity = pre_match_validity.clone();
                self.variable_leased = pre_match_leased.clone();
                self.causal_horizon = pre_match_horizon.clone();
                let mut consumed_clock = branch_start_clock.clone();
                if let Some(branch_body) = consumed_branch {
                    let consumed_pc =
                        self.solver.bool_and(&[path_condition, &consumed_cond]);
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

                let bool_false = self.solver.bool_from_bool(false);
                for var in all_vars {
                    let v_val = post_val_validity.get(&var).unwrap_or(&bool_false);
                    let v_dec = post_dec_validity.get(&var).unwrap_or(&bool_false);
                    let v_pen = post_pen_validity.get(&var).unwrap_or(&bool_false);
                    let v_con = post_con_validity.get(&var).unwrap_or(&bool_false);

                    let m_v = self
                        .solver
                        .bool_const(&format!("{}_m_v_{}", var, spanned.span.start));

                    let case_val_v = self.solver.bool_and(&[&valid_cond, v_val]);
                    let case_dec_v = self.solver.bool_and(&[&decayed_cond, v_dec]);
                    let case_pen_v = self.solver.bool_and(&[&pending_cond, v_pen]);
                    let case_con_v = self.solver.bool_and(&[&consumed_cond, v_con]);

                    let val_expr = self.solver.bool_or(&[
                        &case_val_v,
                        &case_dec_v,
                        &case_pen_v,
                        &case_con_v,
                    ]);
                    let eq_v = self.solver.bool_eq(&m_v, &val_expr);
                    self.solver.assert(&eq_v);
                    merged_validity.insert(var.clone(), m_v);

                    let l_val = post_val_leased.get(&var).unwrap_or(&bool_false);
                    let l_dec = post_dec_leased.get(&var).unwrap_or(&bool_false);
                    let l_pen = post_pen_leased.get(&var).unwrap_or(&bool_false);
                    let l_con = post_con_leased.get(&var).unwrap_or(&bool_false);

                    let m_l = self
                        .solver
                        .bool_const(&format!("{}_m_l_{}", var, spanned.span.start));

                    let case_val_l = self.solver.bool_and(&[&valid_cond, l_val]);
                    let case_dec_l = self.solver.bool_and(&[&decayed_cond, l_dec]);
                    let case_pen_l = self.solver.bool_and(&[&pending_cond, l_pen]);
                    let case_con_l = self.solver.bool_and(&[&consumed_cond, l_con]);

                    let leased_expr = self.solver.bool_or(&[
                        &case_val_l,
                        &case_dec_l,
                        &case_pen_l,
                        &case_con_l,
                    ]);
                    let eq_l = self.solver.bool_eq(&m_l, &leased_expr);
                    self.solver.assert(&eq_l);
                    merged_leased.insert(var.clone(), m_l);
                }

                self.variable_validity = merged_validity;
                self.variable_leased = merged_leased;

                let m_h = self
                    .solver
                    .int_const(&format!("h_m_{}", spanned.span.start));

                let inner_ite1 = self.solver.int_ite(
                    &pending_cond,
                    &post_pen_horizon,
                    &post_con_horizon,
                );
                let inner_ite2 = self.solver.int_ite(
                    &decayed_cond,
                    &post_dec_horizon,
                    &inner_ite1,
                );
                let h_expr =
                    self.solver
                        .int_ite(&valid_cond, &post_val_horizon, &inner_ite2);

                let eq_h = self.solver.int_eq(&m_h, &h_expr);
                self.solver.assert(&eq_h);
                self.causal_horizon = m_h;

                let inner_clock_ite1 = self.solver.int_ite(
                    &pending_cond,
                    &pending_clock,
                    &consumed_clock,
                );
                let inner_clock_ite2 = self.solver.int_ite(
                    &decayed_cond,
                    &decayed_clock,
                    &inner_clock_ite1,
                );
                let final_clock = self.solver.int_ite(
                    &valid_cond,
                    &valid_clock,
                    &inner_clock_ite2,
                );

                Ok(final_clock)
            }
            Statement::Loop { max_ms, body } => {
                let mut loop_clock = self.solver.int_from_u64(0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }
                let mut unroll_clock = loop_clock.clone();
                for stmt in body {
                    unroll_clock =
                        self.verify_statement(stmt, path_condition, &unroll_clock)?;
                }
                let limit_int = self.solver.int_from_u64(*max_ms);
                let violation = self.solver.int_gt(&loop_clock, &limit_int);
                self.solver.push();
                let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                self.solver.assert(&cond_and);
                if self.solver.check() {
                    let actual_wcet = self.solver.eval_u64(&loop_clock).unwrap_or(0);
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::TemporalAssertionViolation(
                            actual_wcet,
                            *max_ms,
                        ),
                    ));
                }
                self.solver.pop(1);
                Ok(self.solver.int_add(&[in_clock, &limit_int]))
            }
            Statement::LoopTick { body } => {
                let slice_ms = self.current_slice_ms.unwrap_or(0);
                let mut loop_clock = self.solver.int_from_u64(0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }
                let mut unroll_clock = loop_clock.clone();
                for stmt in body {
                    unroll_clock =
                        self.verify_statement(stmt, path_condition, &unroll_clock)?;
                }
                let slice_int = self.solver.int_from_u64(slice_ms);
                Ok(self.solver.int_add(&[in_clock, &slice_int]))
            }
            Statement::LoopTickOn { channel: _, body } => {
                let slice_ms = self.current_slice_ms.unwrap_or(0);
                let mut loop_clock = self.solver.int_from_u64(0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }
                let mut unroll_clock = loop_clock.clone();
                for stmt in body {
                    unroll_clock =
                        self.verify_statement(stmt, path_condition, &unroll_clock)?;
                }
                let slice_int = self.solver.int_from_u64(slice_ms);
                Ok(self.solver.int_add(&[in_clock, &slice_int]))
            }
            Statement::While {
                condition: _,
                is_valid_check: _,
                max_ms,
                body,
            } => {
                let mut loop_clock = self.solver.int_from_u64(0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }
                let mut unroll_clock = loop_clock.clone();
                for stmt in body {
                    unroll_clock =
                        self.verify_statement(stmt, path_condition, &unroll_clock)?;
                }
                let limit_int = self.solver.int_from_u64(*max_ms);
                let violation = self.solver.int_gt(&loop_clock, &limit_int);
                self.solver.push();
                let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                self.solver.assert(&cond_and);
                if self.solver.check() {
                    let actual_wcet = self.solver.eval_u64(&loop_clock).unwrap_or(0);
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::TemporalAssertionViolation(
                            actual_wcet,
                            *max_ms,
                        ),
                    ));
                }
                self.solver.pop(1);
                Ok(self.solver.int_add(&[in_clock, &limit_int]))
            }
            Statement::ForStep {
                item_name,
                source: _,
                step_ms,
                body,
            } => {
                let mut loop_clock = self.solver.int_from_u64(0);
                {
                    let item_valid = self.solver.bool_const(&format!(
                        "{}_v1_{}",
                        item_name, spanned.span.start
                    ));
                    let impl_valid =
                        self.solver.bool_implies(path_condition, &item_valid);
                    self.solver.assert(&impl_valid);
                    self.variable_validity.insert(item_name.clone(), item_valid);
                    let item_leased = self.solver.bool_from_bool(false);
                    self.variable_leased.insert(item_name.clone(), item_leased);
                    for stmt in body {
                        loop_clock = self.verify_statement(
                            stmt,
                            path_condition,
                            &loop_clock,
                        )?;
                    }
                }
                {
                    let item_valid = self.solver.bool_const(&format!(
                        "{}_v2_{}",
                        item_name, spanned.span.start
                    ));
                    let impl_valid =
                        self.solver.bool_implies(path_condition, &item_valid);
                    self.solver.assert(&impl_valid);
                    self.variable_validity.insert(item_name.clone(), item_valid);
                    let item_leased = self.solver.bool_from_bool(false);
                    self.variable_leased.insert(item_name.clone(), item_leased);
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
                self.variable_leased.remove(item_name);

                let step_int = self.solver.int_from_u64(*step_ms);
                let violation = self.solver.int_gt(&loop_clock, &step_int);
                self.solver.push();
                let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                self.solver.assert(&cond_and);
                if self.solver.check() {
                    self.solver.pop(1);
                    return Err(self
                        .analyzer
                        .annotate(SemanticErrorKind::PacingViolation));
                }
                self.solver.pop(1);
                Ok(self.solver.int_add(&[in_clock, &step_int]))
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
                max_ms,
            } => {
                if let causm_core::ParamMode::Consume = mode {
                    self.check_available(source, path_condition)?;
                    self.consume_variable(
                        source,
                        path_condition,
                        spanned.span.start,
                    );
                } else if let causm_core::ParamMode::Decay = mode {
                    self.check_available(source, path_condition)?;
                    self.consume_variable(
                        source,
                        path_condition,
                        spanned.span.start,
                    );
                }
                let mut loop_clock = self.solver.int_from_u64(0);
                {
                    let item_valid = self.solver.bool_const(&format!(
                        "{}_v1_{}",
                        item_name, spanned.span.start
                    ));
                    let impl_valid =
                        self.solver.bool_implies(path_condition, &item_valid);
                    self.solver.assert(&impl_valid);
                    self.variable_validity.insert(item_name.clone(), item_valid);
                    let item_leased = self.solver.bool_from_bool(false);
                    self.variable_leased.insert(item_name.clone(), item_leased);
                    for stmt in body {
                        loop_clock = self.verify_statement(
                            stmt,
                            path_condition,
                            &loop_clock,
                        )?;
                    }
                }
                {
                    let item_valid = self.solver.bool_const(&format!(
                        "{}_v2_{}",
                        item_name, spanned.span.start
                    ));
                    let impl_valid =
                        self.solver.bool_implies(path_condition, &item_valid);
                    self.solver.assert(&impl_valid);
                    self.variable_validity.insert(item_name.clone(), item_valid);
                    let item_leased = self.solver.bool_from_bool(false);
                    self.variable_leased.insert(item_name.clone(), item_leased);
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
                self.variable_leased.remove(item_name);
                if let Some(pacing) = pacing_ms {
                    let pacing_int = self.solver.int_from_u64(*pacing);
                    let violation = self.solver.int_gt(&loop_clock, &pacing_int);
                    self.solver.push();
                    let cond_and =
                        self.solver.bool_and(&[path_condition, &violation]);
                    self.solver.assert(&cond_and);
                    if self.solver.check() {
                        self.solver.pop(1);
                        return Err(self
                            .analyzer
                            .annotate(SemanticErrorKind::PacingViolation));
                    }
                    self.solver.pop(1);
                }
                if let Some(max) = max_ms {
                    let max_int = self.solver.int_from_u64(*max);
                    let iteration_cost = if let Some(pacing) = pacing_ms {
                        self.solver.int_from_u64(*pacing)
                    } else {
                        loop_clock.clone()
                    };
                    let violation = self.solver.int_gt(&iteration_cost, &max_int);
                    self.solver.push();
                    let cond_and =
                        self.solver.bool_and(&[path_condition, &violation]);
                    self.solver.assert(&cond_and);
                    if self.solver.check() {
                        let actual_wcet =
                            self.solver.eval_u64(&iteration_cost).unwrap_or(0);
                        self.solver.pop(1);
                        return Err(self.analyzer.annotate(
                            SemanticErrorKind::TemporalAssertionViolation(
                                actual_wcet,
                                *max,
                            ),
                        ));
                    }
                    self.solver.pop(1);
                    Ok(self.solver.int_add(&[in_clock, &max_int]))
                } else {
                    Ok(in_clock.clone())
                }
            }
            Statement::RoutineDef {
                name,
                params,
                taking_ms,
                body,
                ..
            } => {
                let mut routine_verifier = FormalVerifier::<S>::new(self.analyzer);
                let true_bool = routine_verifier.solver.bool_from_bool(true);
                for p in params {
                    let is_valid = routine_verifier
                        .solver
                        .bool_const(&format!("{}_p_{}", p.name, spanned.span.start));
                    let eq_true =
                        routine_verifier.solver.bool_eq(&is_valid, &true_bool);
                    routine_verifier.solver.assert(&eq_true);
                    routine_verifier
                        .variable_validity
                        .insert(p.name.clone(), is_valid);
                }
                let mut body_clock = routine_verifier.solver.int_from_u64(0);
                for stmt in body {
                    body_clock = routine_verifier.verify_statement(
                        stmt,
                        &true_bool,
                        &body_clock,
                    )?;
                }

                let wcet = routine_verifier.find_max_value(&body_clock, &true_bool);
                self.analyzer
                    .analyzed_wcet
                    .borrow_mut()
                    .insert(name.clone(), wcet);

                if let Some(limit) = taking_ms {
                    let limit_int = routine_verifier.solver.int_from_u64(*limit);
                    let violation =
                        routine_verifier.solver.int_gt(&body_clock, &limit_int);
                    routine_verifier.solver.push();
                    routine_verifier.solver.assert(&violation);
                    if routine_verifier.solver.check() {
                        let actual_wcet = routine_verifier
                            .solver
                            .eval_u64(&body_clock)
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
                let limit_int = self.solver.int_from_u64(*limit_ms);
                let violation = match operator {
                    BinaryOperator::Gt => self.solver.int_le(in_clock, &limit_int),
                    BinaryOperator::Lt => self.solver.int_ge(in_clock, &limit_int),
                    BinaryOperator::Ge => self.solver.int_lt(in_clock, &limit_int),
                    BinaryOperator::Le => self.solver.int_gt(in_clock, &limit_int),
                    BinaryOperator::Eq => {
                        let eq_bool = self.solver.int_eq(in_clock, &limit_int);
                        self.solver.bool_not(&eq_bool)
                    }
                    BinaryOperator::Neq => self.solver.int_eq(in_clock, &limit_int),
                    _ => self.solver.bool_from_bool(false),
                };
                self.solver.push();
                let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                self.solver.assert(&cond_and);
                if self.solver.check() {
                    let actual_wcet = self.solver.eval_u64(in_clock).unwrap_or(0);
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
                let is_valid = self.solver.bool_const(&format!(
                    "{}_valid_{}",
                    binding, spanned.span.start
                ));
                let impl_valid = self.solver.bool_implies(path_condition, &is_valid);
                self.solver.assert(&impl_valid);
                self.variable_validity.insert(binding.clone(), is_valid);
                let is_leased = self.solver.bool_const(&format!(
                    "{}_leased_{}",
                    binding, spanned.span.start
                ));
                let impl_leased =
                    self.solver.bool_implies(path_condition, &is_leased);
                self.solver.assert(&impl_leased);
                self.variable_leased.insert(binding.clone(), is_leased);

                let mut body_clock = self.solver.int_from_u64(0);
                for stmt in body {
                    body_clock =
                        self.verify_statement(stmt, path_condition, &body_clock)?;
                }
                let limit_int = self.solver.int_from_u64(*duration_ms);
                let violation = self.solver.int_gt(&body_clock, &limit_int);
                self.solver.push();
                let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                self.solver.assert(&cond_and);
                if self.solver.check() {
                    let actual_wcet = self.solver.eval_u64(&body_clock).unwrap_or(0);
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
                Ok(self.solver.int_add(&[in_clock, &limit_int]))
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
        path_condition: &S::Bool,
        span_start: usize,
    ) {
        let new_valid = self
            .solver
            .bool_const(&format!("{}_consumed_{}", name, span_start));
        let not_valid = self.solver.bool_not(&new_valid);
        let impl_not_valid = self.solver.bool_implies(path_condition, &not_valid);
        self.solver.assert(&impl_not_valid);
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
            let other_valid = self.solver.bool_const(&format!(
                "{}_decayed_by_{}_{}",
                other, name, span_start
            ));
            let not_valid = self.solver.bool_not(&other_valid);
            let impl_not_valid =
                self.solver.bool_implies(path_condition, &not_valid);
            self.solver.assert(&impl_not_valid);
            self.variable_validity.insert(other, other_valid);
        }
    }

    fn check_available(
        &mut self,
        name: &str,
        path_condition: &S::Bool,
    ) -> Result<(), SemanticError> {
        if self.in_entropy_match {
            return Ok(());
        }
        if let Some(valid_bool) = self.variable_validity.get(name) {
            self.solver.push();
            let not_valid = self.solver.bool_not(valid_bool);
            let cond_and = self.solver.bool_and(&[path_condition, &not_valid]);
            self.solver.assert(&cond_and);
            if self.solver.check() {
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
        path_condition: &S::Bool,
    ) -> Result<(), SemanticError> {
        if let Some(leased_bool) = self.variable_leased.get(name) {
            self.solver.push();
            let cond_and = self.solver.bool_and(&[path_condition, leased_bool]);
            self.solver.assert(&cond_and);
            if self.solver.check() {
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
        _path_condition: &S::Bool,
    ) -> S::Bool {
        match expr {
            Expression::Boolean(b) => self.solver.bool_from_bool(*b),
            _ => self.solver.bool_const(&format!("expr_bool_{:?}", expr)),
        }
    }

    fn verify_expression(
        &mut self,
        expr: &Expression,
        path_condition: &S::Bool,
        in_clock: &S::Int,
    ) -> Result<S::Int, SemanticError> {
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
                    let cost_int = self.solver.int_from_u64(info.taking_ms);
                    Ok(self.solver.int_add(&[in_clock, &cost_int]))
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
        path_condition: &S::Bool,
    ) -> Result<(), SemanticError> {
        let budget = block.manifest.cpu_budget_ms.unwrap_or(u64::MAX);
        let mut clock = self.solver.int_from_u64(0);
        let old_anchors = self.anchors.clone();
        let old_slice = self.current_slice_ms;

        self.current_slice_ms = block.manifest.slice_ms;

        for spanned in &block.body {
            clock = self.verify_statement(spanned, path_condition, &clock)?;
        }
        self.anchors = old_anchors;
        self.current_slice_ms = old_slice;

        let budget_int = self.solver.int_from_u64(budget);
        let violation = self.solver.int_gt(&clock, &budget_int);
        self.solver.push();
        let cond_and = self.solver.bool_and(&[path_condition, &violation]);
        self.solver.assert(&cond_and);
        if self.solver.check() {
            self.solver.pop(1);
            return Err(self.analyzer.annotate(
                SemanticErrorKind::TemporalAssertionViolation(0, budget),
            ));
        }
        self.solver.pop(1);
        Ok(())
    }
}
