use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use ictl_core::{
    BinaryOperator, Expression, IsolateBlock, Program, SpannedStatement, Statement,
};
use std::collections::HashMap;
use z3::{ast::Ast, ast::Bool, ast::Int, Context, Solver};

pub struct FormalVerifier<'a> {
    ctx: &'a Context,
    solver: Solver<'a>,
    analyzer: &'a EntropicAnalyzer,
    variable_validity: HashMap<String, Bool<'a>>,
    variable_leased: HashMap<String, Bool<'a>>,
}

impl<'a> FormalVerifier<'a> {
    pub fn new(ctx: &'a Context, analyzer: &'a EntropicAnalyzer) -> Self {
        Self {
            ctx,
            solver: Solver::new(ctx),
            analyzer,
            variable_validity: HashMap::new(),
            variable_leased: HashMap::new(),
        }
    }

    pub fn verify(&mut self, program: &Program) -> Result<(), SemanticError> {
        self.solver.reset();
        for timeline in &program.timelines {
            self.variable_validity.clear();
            self.variable_leased.clear();
            let mut clock = Int::from_u64(self.ctx, 0);
            for spanned in &timeline.statements {
                clock = self.verify_statement(
                    spanned,
                    &Bool::from_bool(self.ctx, true),
                    &clock,
                )?;
            }
        }
        Ok(())
    }

    fn verify_statement(
        &mut self,
        spanned: &SpannedStatement,
        path_condition: &Bool<'a>,
        in_clock: &Int<'a>,
    ) -> Result<Int<'a>, SemanticError> {
        let cost =
            crate::statement::estimate_statement_cost(self.analyzer, &spanned.stmt);
        let current_clock =
            Int::add(self.ctx, &[in_clock, &Int::from_u64(self.ctx, cost)]);

        match &spanned.stmt {
            Statement::Assignment { target, expr, .. } => {
                self.verify_expression(expr, path_condition)?;
                let is_valid = Bool::new_const(
                    self.ctx,
                    format!("{}_valid_{}", target, spanned.span.start),
                );
                self.solver.assert(&path_condition.implies(&is_valid));
                self.variable_validity.insert(target.clone(), is_valid);

                let is_leased = Bool::new_const(
                    self.ctx,
                    format!("{}_leased_{}", target, spanned.span.start),
                );
                self.solver
                    .assert(&path_condition.implies(&is_leased.not()));
                self.variable_leased.insert(target.clone(), is_leased);

                Ok(current_clock)
            }
            Statement::ChannelSend { value_id, .. } | Statement::Yield(value_id) => {
                self.check_available(value_id, path_condition)?;
                self.check_not_leased(value_id, path_condition)?;

                let new_valid = Bool::new_const(
                    self.ctx,
                    format!("{}_consumed_{}", value_id, spanned.span.start),
                );
                self.solver
                    .assert(&path_condition.implies(&new_valid.not()));
                self.variable_validity.insert(value_id.clone(), new_valid);
                Ok(current_clock)
            }
            Statement::Isolate(block) => {
                self.verify_isolate(block, path_condition)?;
                Ok(current_clock)
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

                let then_pc = Bool::and(self.ctx, &[path_condition, &cond_bool]);
                let mut then_clock = current_clock.clone();
                for stmt in then_branch {
                    then_clock =
                        self.verify_statement(stmt, &then_pc, &then_clock)?;
                }
                let post_then_validity = self.variable_validity.clone();
                let post_then_leased = self.variable_leased.clone();

                self.variable_validity = pre_if_validity.clone();
                self.variable_leased = pre_if_leased.clone();
                let else_pc =
                    Bool::and(self.ctx, &[path_condition, &cond_bool.not()]);
                let mut else_clock = current_clock.clone();
                if let Some(else_stmt) = else_branch {
                    for stmt in else_stmt {
                        else_clock =
                            self.verify_statement(stmt, &else_pc, &else_clock)?;
                    }
                }
                let post_else_validity = self.variable_validity.clone();
                let post_else_leased = self.variable_leased.clone();

                let mut merged_validity = HashMap::new();
                let mut merged_leased = HashMap::new();
                for var in pre_if_validity.keys() {
                    let v_then = post_then_validity.get(var).unwrap();
                    let v_else = post_else_validity.get(var).unwrap();
                    let l_then = post_then_leased.get(var).unwrap();
                    let l_else = post_else_leased.get(var).unwrap();

                    let m_v = Bool::new_const(
                        self.ctx,
                        format!("{}_m_v_{}", var, spanned.span.start),
                    );
                    self.solver
                        .assert(&m_v._eq(&Bool::ite(&cond_bool, v_then, v_else)));
                    merged_validity.insert(var.clone(), m_v);

                    let m_l = Bool::new_const(
                        self.ctx,
                        format!("{}_m_l_{}", var, spanned.span.start),
                    );
                    self.solver
                        .assert(&m_l._eq(&Bool::ite(&cond_bool, l_then, l_else)));
                    merged_leased.insert(var.clone(), m_l);
                }
                self.variable_validity = merged_validity;
                self.variable_leased = merged_leased;

                let max_clock = Int::new_const(
                    self.ctx,
                    format!("if_max_clock_{}", spanned.span.start),
                );
                self.solver.assert(&max_clock.ge(&then_clock));
                self.solver.assert(&max_clock.ge(&else_clock));
                Ok(max_clock)
            }
            Statement::Loop { max_ms, body } => {
                let mut loop_clock = Int::from_u64(self.ctx, 0);
                for stmt in body {
                    loop_clock =
                        self.verify_statement(stmt, path_condition, &loop_clock)?;
                }

                let violation = loop_clock.gt(&Int::from_u64(self.ctx, *max_ms));
                self.solver.push();
                self.solver
                    .assert(&Bool::and(self.ctx, &[path_condition, &violation]));
                if self.solver.check() == z3::SatResult::Sat {
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::TemporalAssertionViolation(0, *max_ms),
                    ));
                }
                self.solver.pop(1);
                Ok(Int::add(
                    self.ctx,
                    &[&current_clock, &Int::from_u64(self.ctx, *max_ms)],
                ))
            }
            Statement::AssertTime {
                operator, limit_ms, ..
            } => {
                let violation = match operator {
                    BinaryOperator::Gt => {
                        in_clock.le(&Int::from_u64(self.ctx, *limit_ms))
                    }
                    BinaryOperator::Lt => {
                        in_clock.ge(&Int::from_u64(self.ctx, *limit_ms))
                    }
                    BinaryOperator::Ge => {
                        in_clock.lt(&Int::from_u64(self.ctx, *limit_ms))
                    }
                    BinaryOperator::Le => {
                        in_clock.gt(&Int::from_u64(self.ctx, *limit_ms))
                    }
                    BinaryOperator::Eq => {
                        in_clock._eq(&Int::from_u64(self.ctx, *limit_ms)).not()
                    }
                    BinaryOperator::Neq => {
                        in_clock._eq(&Int::from_u64(self.ctx, *limit_ms))
                    }
                    _ => Bool::from_bool(self.ctx, false),
                };

                self.solver.push();
                self.solver
                    .assert(&Bool::and(self.ctx, &[path_condition, &violation]));
                if self.solver.check() == z3::SatResult::Sat {
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::TemporalAssertionViolation(0, *limit_ms),
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

                let is_valid = Bool::new_const(
                    self.ctx,
                    format!("{}_valid_{}", binding, spanned.span.start),
                );
                self.solver.assert(&path_condition.implies(&is_valid));
                self.variable_validity.insert(binding.clone(), is_valid);

                let is_leased = Bool::new_const(
                    self.ctx,
                    format!("{}_leased_{}", binding, spanned.span.start),
                );
                self.solver.assert(&path_condition.implies(&is_leased));
                self.variable_leased.insert(binding.clone(), is_leased);

                let mut body_clock = Int::from_u64(self.ctx, 0);
                for stmt in body {
                    body_clock =
                        self.verify_statement(stmt, path_condition, &body_clock)?;
                }

                let violation =
                    body_clock.gt(&Int::from_u64(self.ctx, *duration_ms));
                self.solver.push();
                self.solver
                    .assert(&Bool::and(self.ctx, &[path_condition, &violation]));
                if self.solver.check() == z3::SatResult::Sat {
                    self.solver.pop(1);
                    return Err(self.analyzer.annotate(
                        SemanticErrorKind::LeaseDurationExceeded(0, *duration_ms),
                    ));
                }
                self.solver.pop(1);

                self.variable_validity = pre_lease_validity;
                self.variable_leased = pre_lease_leased;
                Ok(Int::add(
                    self.ctx,
                    &[&current_clock, &Int::from_u64(self.ctx, *duration_ms)],
                ))
            }
            Statement::Expression(expr)
            | Statement::Print(expr)
            | Statement::Debug(expr) => {
                self.verify_expression(expr, path_condition)?;
                Ok(current_clock)
            }
            _ => Ok(current_clock),
        }
    }

    fn check_available(
        &mut self,
        name: &str,
        path_condition: &Bool<'a>,
    ) -> Result<(), SemanticError> {
        if let Some(valid_bool) = self.variable_validity.get(name) {
            self.solver.push();
            self.solver
                .assert(&Bool::and(self.ctx, &[path_condition, &valid_bool.not()]));
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
        path_condition: &Bool<'a>,
    ) -> Result<(), SemanticError> {
        if let Some(leased_bool) = self.variable_leased.get(name) {
            self.solver.push();
            self.solver
                .assert(&Bool::and(self.ctx, &[path_condition, leased_bool]));
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
        _path_condition: &Bool<'a>,
    ) -> Bool<'a> {
        match expr {
            Expression::Boolean(b) => Bool::from_bool(self.ctx, *b),
            _ => Bool::new_const(self.ctx, format!("expr_bool_{:?}", expr)),
        }
    }

    fn verify_expression(
        &mut self,
        expr: &Expression,
        path_condition: &Bool<'a>,
    ) -> Result<(), SemanticError> {
        match expr {
            Expression::Identifier(name) => {
                self.check_available(name, path_condition)?;
            }
            Expression::BinaryOp { left, right, .. } => {
                self.verify_expression(left, path_condition)?;
                self.verify_expression(right, path_condition)?;
            }
            Expression::UnaryOp { expr, .. } => {
                self.verify_expression(expr, path_condition)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn verify_isolate(
        &mut self,
        block: &IsolateBlock,
        path_condition: &Bool<'a>,
    ) -> Result<(), SemanticError> {
        let budget = block.manifest.cpu_budget_ms.unwrap_or(u64::MAX);
        let mut clock = Int::from_u64(self.ctx, 0);

        let old_validity = self.variable_validity.clone();
        let old_leased = self.variable_leased.clone();
        for spanned in &block.body {
            clock = self.verify_statement(spanned, path_condition, &clock)?;
        }
        self.variable_validity = old_validity;
        self.variable_leased = old_leased;

        let violation = clock.gt(&Int::from_u64(self.ctx, budget));
        self.solver.push();
        self.solver
            .assert(&Bool::and(self.ctx, &[path_condition, &violation]));
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
