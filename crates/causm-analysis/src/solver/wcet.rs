use super::backend::SolverBackend;
use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::{BinaryOperator, Program, SpannedStatement, Statement};

/// Worst-Case Execution Time (WCET) solver engine.
///
/// Computes topological WCET path bounds over virtual clocks using the SMT backend
/// and verifies temporal contracts (routine limits, isolate cpu budgets, assert_time, loop bounds)
/// using symbolic path condition reasoning.
pub struct WcetSolver<'a, S: SolverBackend = crate::oxiz::OxiZBackend> {
    solver: S,
    analyzer: &'a EntropicAnalyzer,
    current_slice_ms: Option<u64>,
}

impl<'a, S: SolverBackend> WcetSolver<'a, S> {
    pub fn new(analyzer: &'a EntropicAnalyzer) -> Self {
        Self {
            solver: S::new(),
            analyzer,
            current_slice_ms: None,
        }
    }

    /// Primary entry point: verifies all temporal budgets, isolate limits, routine WCET contracts,
    /// and populates analyzer.analyzed_wcet.
    pub fn verify_and_compute(&mut self, program: &Program) -> Result<(), SemanticError> {
        self.solver.reset();

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
                clock = self.verify_statement_wcet(spanned, &path_cond, &clock)?;
            }

            let wcet = self.find_max_value(&clock, &path_cond);
            self.analyzer
                .analyzed_wcet
                .borrow_mut()
                .insert(format!("Timeline {}", idx), wcet);
        }

        Ok(())
    }

    pub fn compute_wcet(&mut self, program: &Program) {
        let _ = self.verify_and_compute(program);
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
            self.solver.push();
            let initial_int = self.solver.int_from_u64(initial_val);
            let check_gt = self.solver.int_gt(val, &initial_int);
            self.solver.assert(&check_gt);
            if !self.solver.check() {
                self.solver.pop(2);
                return initial_val;
            }
            self.solver.pop(1);

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

    fn verify_statement_wcet(
        &mut self,
        spanned: &SpannedStatement,
        path_condition: &S::Bool,
        in_clock: &S::Int,
    ) -> Result<S::Int, SemanticError> {
        let cost = crate::statement::estimate_statement_cost(self.analyzer, &spanned.stmt);
        let cost_int = self.solver.int_from_u64(cost);
        let current_clock = self.solver.int_add(&[in_clock, &cost_int]);

        match &spanned.stmt {
            Statement::AssertTime {
                operator,
                limit_ms,
                fallback,
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

                if let Some(fb) = fallback {
                    let mut fb_clock = current_clock.clone();
                    for s in fb {
                        fb_clock = self.verify_statement_wcet(s, path_condition, &fb_clock)?;
                    }
                    Ok(fb_clock)
                } else {
                    Ok(current_clock)
                }
            }
            Statement::Isolate(block) => {
                let budget = block.manifest.cpu_budget_ms.unwrap_or(u64::MAX);
                let mut iso_clock = self.solver.int_from_u64(0);

                for s in &block.body {
                    iso_clock = self.verify_statement_wcet(s, path_condition, &iso_clock)?;
                }

                let budget_int = self.solver.int_from_u64(budget);
                let violation = self.solver.int_gt(&iso_clock, &budget_int);

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
                Ok(in_clock.clone())
            }
            Statement::RoutineDef {
                name,
                taking_ms,
                body,
                ..
            } => {
                let mut routine_solver = WcetSolver::<S>::new(self.analyzer);
                let true_bool = routine_solver.solver.bool_from_bool(true);
                let mut body_clock = routine_solver.solver.int_from_u64(0);

                for s in body {
                    body_clock = routine_solver.verify_statement_wcet(s, &true_bool, &body_clock)?;
                }

                let wcet = routine_solver.find_max_value(&body_clock, &true_bool);
                self.analyzer
                    .analyzed_wcet
                    .borrow_mut()
                    .insert(name.clone(), wcet);

                if let Some(limit) = *taking_ms {
                    let limit_int = routine_solver.solver.int_from_u64(limit);
                    let violation = routine_solver.solver.int_gt(&body_clock, &limit_int);
                    routine_solver.solver.push();
                    routine_solver.solver.assert(&violation);
                    if routine_solver.solver.check() {
                        let actual_wcet = routine_solver
                            .solver
                            .eval_u64(&body_clock)
                            .unwrap_or(0);
                        routine_solver.solver.pop(1);
                        return Err(self.analyzer.annotate(
                            SemanticErrorKind::TemporalAssertionViolation(
                                actual_wcet,
                                limit,
                            ),
                        ));
                    }
                    routine_solver.solver.pop(1);
                }
                Ok(in_clock.clone())
            }
            Statement::For {
                body,
                pacing_ms,
                max_ms,
                ..
            } => {
                let mut loop_clock = self.solver.int_from_u64(0);
                for s in body {
                    loop_clock = self.verify_statement_wcet(s, path_condition, &loop_clock)?;
                }

                if let Some(max) = max_ms {
                    let max_int = self.solver.int_from_u64(*max);
                    let iteration_cost = if let Some(pacing) = pacing_ms {
                        let pacing_int = self.solver.int_from_u64(*pacing);
                        let is_gt = self.solver.int_gt(&loop_clock, &pacing_int);
                        self.solver.int_ite(&is_gt, &loop_clock, &pacing_int)
                    } else {
                        loop_clock.clone()
                    };
                    let violation = self.solver.int_gt(&iteration_cost, &max_int);
                    self.solver.push();
                    let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                    self.solver.assert(&cond_and);
                    if self.solver.check() {
                        let actual_wcet = self.solver.eval_u64(&iteration_cost).unwrap_or(0);
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
                } else if let Some(pacing) = pacing_ms {
                    let pacing_int = self.solver.int_from_u64(*pacing);
                    Ok(self.solver.int_add(&[in_clock, &pacing_int]))
                } else {
                    Ok(self.solver.int_add(&[in_clock, &loop_clock]))
                }
            }
            Statement::ForStep {
                body,
                step_ms,
                ..
            } => {
                let mut loop_clock = self.solver.int_from_u64(0);
                for s in body {
                    loop_clock = self.verify_statement_wcet(s, path_condition, &loop_clock)?;
                }

                if let Some(ms) = step_ms {
                    let step_int = self.solver.int_from_u64(*ms);
                    let violation = self.solver.int_gt(&loop_clock, &step_int);
                    self.solver.push();
                    let cond_and = self.solver.bool_and(&[path_condition, &violation]);
                    self.solver.assert(&cond_and);
                    if self.solver.check() {
                        self.solver.pop(1);
                        return Err(self.analyzer.annotate(SemanticErrorKind::PacingViolation));
                    }
                    self.solver.pop(1);
                    Ok(self.solver.int_add(&[in_clock, &step_int]))
                } else {
                    Ok(self.solver.int_add(&[in_clock, &loop_clock]))
                }
            }
            Statement::If {
                condition: _,
                then_branch,
                else_branch,
                ..
            } => {
                let mut then_clock = current_clock.clone();
                for s in then_branch {
                    then_clock = self.verify_statement_wcet(s, path_condition, &then_clock)?;
                }

                let mut else_clock = current_clock.clone();
                if let Some(else_stmts) = else_branch {
                    for s in else_stmts {
                        else_clock = self.verify_statement_wcet(s, path_condition, &else_clock)?;
                    }
                }

                let then_is_gt = self.solver.int_gt(&then_clock, &else_clock);
                Ok(self.solver.int_ite(&then_is_gt, &then_clock, &else_clock))
            }
            Statement::DirectiveBlock { directives, body } => {
                let bypass = directives.iter().any(|d| matches!(d, causm_core::BlockDirective::NoZ3));
                if bypass {
                    Ok(current_clock)
                } else {
                    let mut block_clock = in_clock.clone();
                    for s in body {
                        block_clock = self.verify_statement_wcet(s, path_condition, &block_clock)?;
                    }
                    Ok(block_clock)
                }
            }
            Statement::Slice { milliseconds } => {
                self.current_slice_ms = Some(*milliseconds);
                Ok(in_clock.clone())
            }
            Statement::LoopTick { body } => {
                let slice = self.current_slice_ms.unwrap_or(1);
                let slice_int = self.solver.int_from_u64(slice);
                let mut body_clock = self.solver.int_from_u64(0);
                for s in body {
                    body_clock = self.verify_statement_wcet(s, path_condition, &body_clock)?;
                }
                let body_gt_slice = self.solver.int_gt(&body_clock, &slice_int);
                let final_tick_cost = self.solver.int_ite(&body_gt_slice, &body_clock, &slice_int);
                Ok(self.solver.int_add(&[in_clock, &final_tick_cost]))
            }
            Statement::RelativisticBlock { body, .. }
            | Statement::Commit(body)
            | Statement::DecayHandler { body, .. } => {
                let mut block_clock = in_clock.clone();
                for s in body {
                    block_clock = self.verify_statement_wcet(s, path_condition, &block_clock)?;
                }
                Ok(block_clock)
            }
            _ => Ok(current_clock),
        }
    }
}
