use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::expression::analyze_expression;
use ictl_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn If(
        &mut self,
        condition: &Expression,
        then_branch: &[SpannedStatement],
        else_branch: &Option<Vec<SpannedStatement>>,
        reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        let condition_type =
            crate::expression::infer_expression_type(self, condition)?;
        if condition_type != ictl_core::types::Type::Bool {
            return Err(self.annotate(SemanticErrorKind::TypeMismatch(format!(
                "if condition must be bool, got {:?}",
                condition_type
            ))));
        }
        analyze_expression(self, condition)?;

        let original_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        let mut then_contexts = self.branch_contexts.clone();
        then_contexts.insert(self.current_branch.clone(), original_state.clone());
        let previous_contexts =
            std::mem::replace(&mut self.branch_contexts, then_contexts);

        for inner_stmt in then_branch {
            self.analyze_statement(inner_stmt)?;
        }
        let then_end_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        self.branch_contexts = previous_contexts.clone();
        let mut else_contexts = self.branch_contexts.clone();
        else_contexts.insert(self.current_branch.clone(), original_state.clone());
        self.branch_contexts = else_contexts;

        if let Some(else_stmts) = else_branch {
            for inner_stmt in else_stmts {
                self.analyze_statement(inner_stmt)?;
            }
        }
        let else_end_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        self.branch_contexts = previous_contexts;
        let merged = self.merge_states(then_end_state, else_end_state, reconcile)?;
        self.branch_contexts
            .insert(self.current_branch.clone(), merged);
        Ok(())
    }

    pub(crate) fn Loop(
        &mut self,
        max_ms: &u64,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        if *max_ms == 0 {
            return Err(self.annotate(SemanticErrorKind::InvalidLoopBudget));
        }
        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        branch.accumulated_cost += *max_ms;
        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        Ok(())
    }

    pub(crate) fn Break(&mut self) -> Result<(), SemanticError> {
        Ok(())
    }

    pub(crate) fn LoopTick(
        &mut self,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        let slice_ms = self
            .current_slice_ms
            .ok_or_else(|| self.annotate(SemanticErrorKind::TickLoopWithoutSlice))?;

        let body_cost = crate::statement::estimate_block_cost(self, body);
        if body_cost > slice_ms {
            return Err(self.annotate(SemanticErrorKind::TickLoopBudgetExceeded(
                body_cost, slice_ms,
            )));
        }

        let has_break = body
            .iter()
            .any(|inner_stmt| matches!(inner_stmt.stmt, Statement::Break));

        if !has_break {
            return Err(self.annotate(SemanticErrorKind::TickLoopNeedsBreak));
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        Ok(())
    }

    pub(crate) fn Select(
        &mut self,
        _max_ms: &u64,
        cases: &[SelectCase],
        timeout: &Option<Vec<SpannedStatement>>,
        reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        let original_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        let mut branch_results = Vec::new();

        for case in cases {
            let case_type =
                crate::expression::infer_expression_type(self, &case.source)
                    .unwrap_or(ictl_core::types::Type::Unknown);
            analyze_expression(self, &case.source)?;

            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            self.set_variable_type(&case.binding, case_type);

            for stmt in &case.body {
                self.analyze_statement(stmt)?;
            }

            let mut end_state = self
                .branch_contexts
                .get(&self.current_branch)
                .cloned()
                .unwrap_or_default();
            end_state.consumed.remove(&case.binding);
            end_state.yields.remove(&case.binding);
            branch_results.push(end_state);
            self.branch_contexts = saved_contexts;
        }

        if let Some(timeout_body) = timeout {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());

            for stmt in timeout_body {
                self.analyze_statement(stmt)?;
            }

            let end_state = self
                .branch_contexts
                .get(&self.current_branch)
                .cloned()
                .unwrap_or_default();
            branch_results.push(end_state);
            self.branch_contexts = saved_contexts;
        } else {
            branch_results.push(original_state.clone());
        }

        if branch_results.is_empty() {
            return Ok(());
        }

        let mut final_state = branch_results[0].clone();
        for i in 1..branch_results.len() {
            final_state = self.merge_states(
                final_state,
                branch_results[i].clone(),
                reconcile,
            )?;
        }

        self.branch_contexts
            .insert(self.current_branch.clone(), final_state);
        Ok(())
    }

    pub(crate) fn MatchEntropy(
        &mut self,
        target: &Expression,
        valid_branch: &Option<(String, Vec<SpannedStatement>)>,
        decayed_branch: &Option<(String, Vec<SpannedStatement>)>,
        pending_branch: &Option<Vec<SpannedStatement>>,
        consumed_branch: &Option<Vec<SpannedStatement>>,
    ) -> Result<(), SemanticError> {
        let original_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();
        let mut context_candidates = Vec::new();

        if let Some((binding, branch_body)) = valid_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            self.branch_contexts
                .get_mut(&self.current_branch)
                .unwrap()
                .yields
                .insert(binding.clone());

            let case_type = crate::expression::infer_expression_type(self, target)?;
            self.set_variable_type(binding, case_type);
            analyze_expression(self, target)?;

            for stmt in branch_body {
                self.analyze_statement(stmt)?;
            }
            context_candidates.push(
                self.branch_contexts
                    .get(&self.current_branch)
                    .cloned()
                    .unwrap_or_default(),
            );
            self.branch_contexts = saved_contexts;
        }

        if let Some((binding, branch_body)) = decayed_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            self.branch_contexts
                .get_mut(&self.current_branch)
                .unwrap()
                .yields
                .insert(binding.clone());

            let case_type = crate::expression::infer_expression_type(self, target)?;
            self.set_variable_type(binding, case_type);
            analyze_expression(self, target)?;

            for stmt in branch_body {
                self.analyze_statement(stmt)?;
            }
            context_candidates.push(
                self.branch_contexts
                    .get(&self.current_branch)
                    .cloned()
                    .unwrap_or_default(),
            );
            self.branch_contexts = saved_contexts;
        }

        if let Some(branch_body) = pending_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            for stmt in branch_body {
                self.analyze_statement(stmt)?;
            }
            context_candidates.push(
                self.branch_contexts
                    .get(&self.current_branch)
                    .cloned()
                    .unwrap_or_default(),
            );
            self.branch_contexts = saved_contexts;
        }

        if let Some(branch_body) = consumed_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            for stmt in branch_body {
                self.analyze_statement(stmt)?;
            }
            context_candidates.push(
                self.branch_contexts
                    .get(&self.current_branch)
                    .cloned()
                    .unwrap_or_default(),
            );
            self.branch_contexts = saved_contexts;
        }

        let merged_state = context_candidates.into_iter().fold(
            original_state.clone(),
            |mut acc, s| {
                acc.consumed.extend(s.consumed);
                acc.decayed.extend(s.decayed);
                acc.yields.extend(s.yields);
                acc
            },
        );

        self.branch_contexts
            .insert(self.current_branch.clone(), merged_state);
        Ok(())
    }

    pub(crate) fn RoutineDef(
        &mut self,
        name: &String,
        params: &[ParamDecl],
        return_type: &Option<TypeName>,
        taking_ms: &Option<u64>,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        if self.routines.contains_key(name) {
            return Err(self.annotate(SemanticErrorKind::EntropyMismatch(format!(
                "duplicate routine {}",
                name
            ))));
        }

        for stmt in body {
            match &stmt.stmt {
                Statement::Split { .. }
                | Statement::Merge { .. }
                | Statement::RelativisticBlock { .. } => {
                    return Err(self.annotate(SemanticErrorKind::EntropyMismatch(
                        "routines cannot contain split/merge/relativistic blocks"
                            .to_string(),
                    )));
                }
                _ => {}
            }
        }

        let mut routine_analyzer = EntropicAnalyzer::new();
        routine_analyzer.routines = self.routines.clone();
        if let Some(main_state) = self.branch_contexts.get("main") {
            let routine_main =
                routine_analyzer.branch_contexts.get_mut("main").unwrap();
            routine_main.custom_types = main_state.custom_types.clone();
        }

        for param in params {
            let routine_state =
                routine_analyzer.branch_contexts.get_mut("main").unwrap();
            routine_state.yields.insert(param.name.clone());
            let param_type = param
                .typ
                .as_ref()
                .map(ictl_core::types::Type::from_typename)
                .unwrap_or(ictl_core::types::Type::Unknown);
            routine_analyzer.set_variable_type(&param.name, param_type);
        }

        if let Some(rt) = return_type {
            let routine_state =
                routine_analyzer.branch_contexts.get_mut("main").unwrap();
            routine_state.yields.insert("<return>".to_string());
            routine_analyzer.set_variable_type(
                "<return>",
                ictl_core::types::Type::from_typename(rt),
            );
        }

        for stmt in body {
            routine_analyzer.analyze_statement(stmt)?;
        }

        let estimated_cost =
            crate::statement::estimate_block_cost(&routine_analyzer, body);
        let final_taking_ms = if let Some(ms) = *taking_ms {
            if estimated_cost > ms {
                return Err(self.annotate(
                    SemanticErrorKind::RoutineBudgetExceeded(
                        name.clone(),
                        ms,
                        estimated_cost,
                    ),
                ));
            }
            ms
        } else {
            estimated_cost
        };

        let routine_params = params
            .iter()
            .map(|p| {
                (
                    p.mode.clone(),
                    p.name.clone(),
                    p.typ
                        .as_ref()
                        .map(ictl_core::types::Type::from_typename)
                        .unwrap_or(ictl_core::types::Type::Unknown),
                )
            })
            .collect();
        let routine_info = crate::analyzer::RoutineInfo {
            params: routine_params,
            return_type: return_type
                .as_ref()
                .map(ictl_core::types::Type::from_typename)
                .unwrap_or(ictl_core::types::Type::Unknown),
            taking_ms: final_taking_ms,
        };

        self.routines.insert(name.clone(), routine_info);
        Ok(())
    }

    pub(crate) fn Return(
        &mut self,
        src: &Option<String>,
    ) -> Result<(), SemanticError> {
        if let Some(name) = src {
            self.check_available(name)?;
        }
        Ok(())
    }

    pub(crate) fn For(
        &mut self,
        item_name: &str,
        mode: &ForMode,
        source: &str,
        body: &[SpannedStatement],
        pacing_ms: &Option<u64>,
        max_ms: &Option<u64>,
    ) -> Result<(), SemanticError> {
        let source_type = self
            .get_variable_type(source)
            .unwrap_or(ictl_core::types::Type::Unknown);

        let (loop_item_type, max_per_iteration) = match source_type {
            ictl_core::types::Type::Struct(_)
            | ictl_core::types::Type::Topology(_) => {
                let mut item_fields = std::collections::HashMap::new();
                item_fields
                    .insert("key".to_string(), ictl_core::types::Type::String);
                item_fields
                    .insert("value".to_string(), ictl_core::types::Type::Unknown);
                (
                    ictl_core::types::Type::Struct(ictl_core::types::StructType {
                        fields: item_fields,
                        decay_after_ms: None,
                        scoped_branch: None,
                    }),
                    None,
                )
            }
            ictl_core::types::Type::Array(inner) => (*inner.clone(), None),
            ictl_core::types::Type::PacedIterable {
                element_type,
                max_time_ms,
            } => (*element_type.clone(), Some(max_time_ms)),
            other => (other, None),
        };
        self.set_variable_type(item_name, loop_item_type);

        if let ForMode::Consume = mode {
            self.mark_consumed(source)?;
        }

        if let Some(limit) = max_per_iteration {
            let body_cost = crate::statement::estimate_block_cost(self, body);
            if body_cost > limit {
                return Err(self.annotate(SemanticErrorKind::EntropyMismatch(
                    format!(
                        "Loop body cost {}ms exceeds PacedIterable contract: {}ms",
                        body_cost, limit
                    ),
                )));
            }
        }

        if let Some(max) = max_ms {
            if *max == 0 {
                return Err(self.annotate(SemanticErrorKind::InvalidLoopBudget));
            }
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }

        if let Some(pacing) = pacing_ms {
            let body_cost = crate::statement::estimate_block_cost(self, body);
            if body_cost > *pacing {
                return Err(self.annotate(SemanticErrorKind::PacingViolation));
            }
        }
        Ok(())
    }

    pub(crate) fn SplitMap(
        &mut self,
        item_name: &str,
        mode: &ForMode,
        source: &str,
        body: &[SpannedStatement],
        _reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        let source_type = self
            .get_variable_type(source)
            .unwrap_or(ictl_core::types::Type::Unknown);

        let loop_item_type = match source_type {
            ictl_core::types::Type::Struct(_)
            | ictl_core::types::Type::Topology(_) => {
                let mut item_fields = std::collections::HashMap::new();
                item_fields
                    .insert("key".to_string(), ictl_core::types::Type::String);
                item_fields
                    .insert("value".to_string(), ictl_core::types::Type::Unknown);
                ictl_core::types::Type::Struct(ictl_core::types::StructType {
                    fields: item_fields,
                    decay_after_ms: None,
                    scoped_branch: None,
                })
            }
            ictl_core::types::Type::Array(inner) => *inner.clone(),
            other => other,
        };
        self.set_variable_type(item_name, loop_item_type);

        if let ForMode::Consume = mode {
            self.mark_consumed(source)?;
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }

        Ok(())
    }

    pub(crate) fn Yield(&mut self, _name: &String) -> Result<(), SemanticError> {
        Ok(())
    }

    pub(crate) fn Inspect(
        &mut self,
        _target: &String,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        let snapshot = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        self.inspection_depth += 1;
        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        self.inspection_depth -= 1;

        self.branch_contexts
            .insert(self.current_branch.clone(), snapshot);
        Ok(())
    }

    pub(crate) fn Commit(
        &mut self,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        Ok(())
    }

    pub(crate) fn SpeculationMode(
        &mut self,
        _mode: &SpeculationCommitMode,
    ) -> Result<(), SemanticError> {
        Ok(())
    }
}
