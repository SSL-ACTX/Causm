use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use crate::expression::analyze_expression;
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn If(
        &mut self,
        binding: &Option<String>,
        condition: &Expression,
        then_branch: &[SpannedStatement],
        else_branch: &Option<Vec<SpannedStatement>>,
        reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        let condition_type =
            crate::expression::infer_expression_type(self, condition)?;

        if binding.is_some() {
            match condition {
                Expression::TypeAssertion { .. } => {}
                _ => {
                    return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                        "if let expression must be a type assertion".into(),
                    )));
                }
            }
        } else {
            if condition_type != causm_core::types::Type::Bool {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    format!("if condition must be bool, got {:?}", condition_type),
                )));
            }
        }

        crate::expression::analyze_expression_nonconsuming(self, condition)?;

        let original_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        let mut then_contexts = self.branch_contexts.clone();
        then_contexts.insert(self.current_branch.clone(), original_state.clone());
        let previous_contexts =
            std::mem::replace(&mut self.branch_contexts, then_contexts);

        if let Some(binding_name) = binding {
            let cast_type = match condition {
                Expression::TypeAssertion { cast_type, .. } => {
                    causm_core::types::Type::from_typename(cast_type)
                }
                _ => unreachable!(),
            };

            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.types.insert(binding_name.clone(), cast_type);
            branch.produced.insert(binding_name.clone());
            branch
                .instantiated_at
                .insert(binding_name.clone(), branch.accumulated_cost);
        }

        for inner_stmt in then_branch {
            self.analyze_statement(inner_stmt)?;
        }

        let mut then_end_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        if let Some(binding_name) = binding {
            if self.enforce_egc && !then_end_state.consumed.contains(binding_name) {
                return Err(self.annotate(SemanticErrorKind::UnconsumedVariable(
                    binding_name.clone(),
                )));
            }

            then_end_state.remove_variable_scope(binding_name);
            then_end_state.produced.remove(binding_name);
        }

        self.branch_contexts = previous_contexts.clone();
        let mut else_contexts = self.branch_contexts.clone();
        else_contexts.insert(self.current_branch.clone(), original_state.clone());
        self.branch_contexts = else_contexts;

        if let Some(else_stmts) = else_branch {
            for inner_stmt in else_stmts {
                self.analyze_statement(inner_stmt)?;
            }
        }
        let then_locals: Vec<String> = then_end_state
            .produced
            .difference(&original_state.produced)
            .cloned()
            .collect();
        for local in then_locals {
            then_end_state.remove_variable_scope(&local);
        }

        let mut else_end_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        let else_locals: Vec<String> = else_end_state
            .produced
            .difference(&original_state.produced)
            .cloned()
            .collect();
        for local in else_locals {
            else_end_state.remove_variable_scope(&local);
        }

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
        // u64::MAX is the wildcard sentinel for `taking _` — treat as unconstrained
        if *max_ms == 0 {
            return Err(self.annotate(SemanticErrorKind::InvalidLoopBudget));
        }
        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        if *max_ms != u64::MAX {
            branch.accumulated_cost += *max_ms;
        }
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

    pub(crate) fn LoopTickOn(
        &mut self,
        channel: &str,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        if !self.known_channels.contains(channel) {
            return Err(self.annotate(SemanticErrorKind::UndefinedVariable(
                channel.to_owned(),
            )));
        }

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

    pub(crate) fn While(
        &mut self,
        condition: &Expression,
        is_valid_check: &bool,
        max_ms: &u64,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        // u64::MAX is the wildcard sentinel for `taking _` — treat as unconstrained
        if *max_ms == 0 {
            return Err(self.annotate(SemanticErrorKind::InvalidLoopBudget));
        }

        crate::expression::analyze_expression_nonconsuming(self, condition)?;

        if !*is_valid_check {
            let cond_type =
                crate::expression::infer_expression_type(self, condition)?;
            if cond_type != causm_core::types::Type::Bool {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    "While condition must be a boolean".to_string(),
                )));
            }
        }

        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        if *max_ms != u64::MAX {
            branch.accumulated_cost += *max_ms;
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        Ok(())
    }

    pub(crate) fn ForStep(
        &mut self,
        item_name: &str,
        source: &Expression,
        step_ms: &Option<u64>,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        crate::expression::analyze_expression_nonconsuming(self, source)?;

        let source_type = crate::expression::infer_expression_type(self, source)?;
        let item_type = match source_type {
            causm_core::types::Type::Array(inner) => *inner,
            causm_core::types::Type::Unknown => causm_core::types::Type::Unknown,
            _ => {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    "ForStep source must be an array".to_string(),
                )));
            }
        };

        // Only enforce budget when step is a concrete value (not wildcard)
        if let Some(limit) = step_ms {
            let body_cost = crate::statement::estimate_block_cost(self, body);
            if body_cost > *limit {
                return Err(self.annotate(
                    SemanticErrorKind::TickLoopBudgetExceeded(body_cost, *limit),
                ));
            }
        }

        let old_type = {
            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.produced.insert(item_name.to_owned());
            branch.yields.insert(item_name.to_owned());
            branch.types.insert(item_name.to_owned(), item_type)
        };

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }

        let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
        branch.remove_variable_scope(item_name);
        if let Some(old) = old_type {
            branch.types.insert(item_name.to_owned(), old);
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
                    .unwrap_or(causm_core::types::Type::Unknown);
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
        for branch_result in branch_results.iter().skip(1) {
            final_state =
                self.merge_states(final_state, branch_result.clone(), reconcile)?;
        }

        self.branch_contexts
            .insert(self.current_branch.clone(), final_state);
        Ok(())
    }

    fn apply_pattern(
        &mut self,
        pattern: &DecayedPattern,
        target: &Expression,
    ) -> Result<(), SemanticError> {
        match pattern {
            DecayedPattern::Binding(binding) => {
                if !binding.is_empty() {
                    self.branch_contexts
                        .get_mut(&self.current_branch)
                        .unwrap()
                        .yields
                        .insert(binding.clone());

                    let case_type =
                        crate::expression::infer_expression_type(self, target)?;
                    self.set_variable_type(binding, case_type);
                }
            }
            DecayedPattern::Fields(fields) => {
                if let Expression::Identifier(ref target_name) = target {
                    let branch_state =
                        self.branch_contexts.get_mut(&self.current_branch).unwrap();
                    for (field_name, val) in fields {
                        let field_path = format!("{}.{}", target_name, field_name);
                        match val {
                            PatternValue::State(state_name) => {
                                match state_name.as_str() {
                                    "Consumed" => {
                                        branch_state.consumed.insert(field_path);
                                    }
                                    "Valid" => {
                                        branch_state.consumed.remove(&field_path);
                                        branch_state.decayed.remove(&field_path);
                                    }
                                    "Decayed" => {
                                        branch_state
                                            .decayed
                                            .insert(field_path.clone());
                                        branch_state.consumed.remove(&field_path);
                                    }
                                    "Pending" => {
                                        // Pending state
                                    }
                                    _ => {}
                                }
                            }
                            PatternValue::Expr(_) => {
                                branch_state.consumed.remove(&field_path);
                                branch_state.decayed.remove(&field_path);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn MatchEntropy(
        &mut self,
        target: &Expression,
        valid_branch: &Option<(
            DecayedPattern,
            Option<Expression>,
            Vec<SpannedStatement>,
        )>,
        decayed_branch: &Option<(
            DecayedPattern,
            Option<Expression>,
            Vec<SpannedStatement>,
        )>,
        pending_branch: &Option<(
            DecayedPattern,
            Option<Expression>,
            Vec<SpannedStatement>,
        )>,
        consumed_branch: &Option<(Option<Expression>, Vec<SpannedStatement>)>,
    ) -> Result<(), SemanticError> {
        let original_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();
        let mut context_candidates = Vec::new();

        if let Some((pattern, guard, branch_body)) = valid_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            self.apply_pattern(pattern, target)?;

            self.in_entropy_match = true;
            let res =
                crate::expression::analyze_expression_nonconsuming(self, target);
            self.in_entropy_match = false;
            res?;

            if let Some(guard_expr) = guard {
                crate::expression::analyze_expression(self, guard_expr)?;
            }

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

        if let Some((pattern, guard, branch_body)) = decayed_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            self.apply_pattern(pattern, target)?;

            self.in_entropy_match = true;
            let res =
                crate::expression::analyze_expression_nonconsuming(self, target);
            self.in_entropy_match = false;
            res?;

            if let Some(guard_expr) = guard {
                crate::expression::analyze_expression(self, guard_expr)?;
            }

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

        if let Some((pattern, guard, branch_body)) = pending_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            self.apply_pattern(pattern, target)?;

            self.in_entropy_match = true;
            let res =
                crate::expression::analyze_expression_nonconsuming(self, target);
            self.in_entropy_match = false;
            res?;

            if let Some(guard_expr) = guard {
                crate::expression::analyze_expression(self, guard_expr)?;
            }

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

        if let Some((guard, branch_body)) = consumed_branch {
            let saved_contexts = self.branch_contexts.clone();
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state.clone());

            if let Some(guard_expr) = guard {
                crate::expression::analyze_expression(self, guard_expr)?;
            }

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

        if context_candidates.is_empty() {
            self.branch_contexts
                .insert(self.current_branch.clone(), original_state);
            return Ok(());
        }

        let mut final_state = context_candidates[0].clone();
        let recon = Some(MergeResolution {
            rules: std::collections::HashMap::new(),
            auto: true,
            fallback: None,
            taking_ms: None,
        });

        for candidate in context_candidates.iter().skip(1) {
            final_state =
                self.merge_states(final_state, candidate.clone(), &recon)?;
        }

        self.branch_contexts
            .insert(self.current_branch.clone(), final_state);
        Ok(())
    }

    pub(crate) fn RoutineDef(
        &mut self,
        name: &String,
        params: &[ParamDecl],
        return_type: &Option<TypeName>,
        taking_ms: &Option<u64>,
        state_constraint: &Option<(String, String)>,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        if self.analyzed_routines.contains(name) {
            if body.is_empty() {
                return Ok(());
            }
            return Err(self.annotate(SemanticErrorKind::EntropyMismatch(format!(
                "duplicate routine {}",
                name
            ))));
        }
        self.analyzed_routines.insert(name.clone());

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
        let base_name = if let Some(angle_idx) = name.find('<') {
            if let Some(dot_idx) = name.find('.') {
                let struct_part = &name[..angle_idx];
                let method_part = &name[dot_idx..];
                format!("{}{}", struct_part, method_part)
            } else {
                name.clone()
            }
        } else {
            name.clone()
        };

        let preliminary_routine_info = crate::analyzer::RoutineInfo {
            params: params
                .iter()
                .map(|p| {
                    let mut param_type = p
                        .typ
                        .as_ref()
                        .map(causm_core::types::Type::from_typename)
                        .unwrap_or(causm_core::types::Type::Unknown);
                    if p.name == "self" && p.typ.is_none() {
                        if let Some(dot_idx) = name.find('.') {
                            let struct_name = &name[..dot_idx];
                            param_type = causm_core::types::Type::Custom(
                                struct_name.to_string(),
                            );
                        }
                    }
                    (p.mode.clone(), p.name.clone(), param_type)
                })
                .collect(),
            return_type: return_type
                .as_ref()
                .map(causm_core::types::Type::from_typename)
                .unwrap_or(causm_core::types::Type::Unknown),
            taking_ms: taking_ms.unwrap_or(0),
            state_constraint: state_constraint.clone(),
        };

        let mut routine_analyzer = EntropicAnalyzer::new();
        routine_analyzer.routines = self.routines.clone();
        routine_analyzer
            .routines
            .insert(name.clone(), preliminary_routine_info.clone());
        if base_name != *name {
            routine_analyzer
                .routines
                .insert(base_name.clone(), preliminary_routine_info);
        }
        routine_analyzer.type_decls = self.type_decls.clone();
        routine_analyzer.interfaces = self.interfaces.clone();
        routine_analyzer.current_routine = Some(name.clone());
        if let Some(main_state) = self.branch_contexts.get("main") {
            let routine_main =
                routine_analyzer.branch_contexts.get_mut("main").unwrap();
            routine_main.custom_types = main_state.custom_types.clone();
        }

        for param in params {
            let routine_state =
                routine_analyzer.branch_contexts.get_mut("main").unwrap();
            routine_state.produced.insert(param.name.clone());
            routine_state.consumed.remove(&param.name);
            routine_state.decayed.remove(&param.name);
            if matches!(param.mode, ParamMode::Peek | ParamMode::Lease) {
                routine_state.mutables.insert(param.name.clone());
            } else {
                routine_state.yields.insert(param.name.clone());
            }
            let mut param_type = param
                .typ
                .as_ref()
                .map(causm_core::types::Type::from_typename)
                .unwrap_or(causm_core::types::Type::Unknown);
            if param.name == "self" && param.typ.is_none() {
                if let Some(last_dot) = name.rfind('.') {
                    let full_struct_name = &name[..last_dot];
                    let bare_struct_name =
                        if let Some(first_dot) = full_struct_name.rfind('.') {
                            &full_struct_name[first_dot + 1..]
                        } else {
                            full_struct_name
                        };
                    let target_struct = if routine_analyzer
                        .get_custom_type(full_struct_name)
                        .is_some()
                        || routine_analyzer.type_decls.contains_key(full_struct_name)
                    {
                        full_struct_name
                    } else {
                        bare_struct_name
                    };
                    param_type =
                        causm_core::types::Type::Custom(target_struct.to_string());
                }
            }
            routine_analyzer.set_variable_type(&param.name, param_type);
        }

        if let Some(rt) = return_type {
            let routine_state =
                routine_analyzer.branch_contexts.get_mut("main").unwrap();
            routine_state.yields.insert("<return>".to_string());
            routine_analyzer.set_variable_type(
                "<return>",
                causm_core::types::Type::from_typename(rt),
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
                let mut param_type = p
                    .typ
                    .as_ref()
                    .map(causm_core::types::Type::from_typename)
                    .unwrap_or(causm_core::types::Type::Unknown);
                if p.name == "self" && p.typ.is_none() {
                    if let Some(dot_idx) = name.find('.') {
                        let struct_name = &name[..dot_idx];
                        param_type =
                            causm_core::types::Type::Custom(struct_name.to_string());
                    }
                }
                (p.mode.clone(), p.name.clone(), param_type)
            })
            .collect();
        let routine_info = crate::analyzer::RoutineInfo {
            params: routine_params,
            return_type: return_type
                .as_ref()
                .map(causm_core::types::Type::from_typename)
                .unwrap_or(causm_core::types::Type::Unknown),
            taking_ms: final_taking_ms,
            state_constraint: state_constraint.clone(),
        };

        self.routines.insert(name.clone(), routine_info.clone());
        if base_name != *name {
            self.routines.insert(base_name, routine_info);
        }
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
        mode: &ParamMode,
        source: &str,
        body: &[SpannedStatement],
        pacing_ms: &Option<u64>,
        max_ms: &Option<u64>,
    ) -> Result<(), SemanticError> {
        let source_type = self
            .get_variable_type(source)
            .unwrap_or(causm_core::types::Type::Unknown);

        let (loop_item_type, max_per_iteration) = match source_type {
            causm_core::types::Type::Struct(_)
            | causm_core::types::Type::Topology(_) => {
                let mut item_fields = std::collections::HashMap::new();
                item_fields
                    .insert("key".to_string(), causm_core::types::Type::String);
                item_fields
                    .insert("value".to_string(), causm_core::types::Type::Unknown);
                (
                    causm_core::types::Type::Struct(causm_core::types::StructType {
                        fields: item_fields,
                        decay_after_ms: None,
                        auto_drop: None,
                        scoped_branch: None,
                    }),
                    None,
                )
            }
            causm_core::types::Type::Array(inner) => (*inner.clone(), None),
            causm_core::types::Type::PacedIterable {
                element_type,
                max_time_ms,
            } => (*element_type.clone(), Some(max_time_ms)),
            other => (other, None),
        };
        self.set_variable_type(item_name, loop_item_type);

        if let ParamMode::Consume = mode {
            self.mark_consumed(source)?;
        } else if let ParamMode::Decay = mode {
            self.mark_decayed(source)?;
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
        mode: &ParamMode,
        source: &str,
        body: &[SpannedStatement],
        _reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        let source_type = self
            .get_variable_type(source)
            .unwrap_or(causm_core::types::Type::Unknown);

        let loop_item_type = match source_type {
            causm_core::types::Type::Struct(_)
            | causm_core::types::Type::Topology(_) => {
                let mut item_fields = std::collections::HashMap::new();
                item_fields
                    .insert("key".to_string(), causm_core::types::Type::String);
                item_fields
                    .insert("value".to_string(), causm_core::types::Type::Unknown);
                causm_core::types::Type::Struct(causm_core::types::StructType {
                    fields: item_fields,
                    decay_after_ms: None,
                    auto_drop: None,
                    scoped_branch: None,
                })
            }
            causm_core::types::Type::Array(inner) => *inner.clone(),
            other => other,
        };
        self.set_variable_type(item_name, loop_item_type);

        if let ParamMode::Consume = mode {
            self.mark_consumed(source)?;
        } else if let ParamMode::Decay = mode {
            self.mark_decayed(source)?;
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }

        Ok(())
    }

    pub(crate) fn Yield(&mut self, name: &str) -> Result<(), SemanticError> {
        self.mark_consumed(name)
    }

    pub(crate) fn Inspect(
        &mut self,
        binding: &str,
        target: &str,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        let snapshot = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        let target_type = self
            .get_variable_type(target)
            .unwrap_or(causm_core::types::Type::Unknown);
        self.set_variable_type(binding, target_type);

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

    pub(crate) fn Using(
        &mut self,
        binding: &String,
        resource: &Expression,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        crate::expression::analyze_expression(self, resource)?;
        let res_type = crate::expression::infer_expression_type(self, resource)?;
        {
            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.types.insert(binding.clone(), res_type);
            branch.produced.insert(binding.clone());
            branch.consumed.remove(binding);
            branch
                .instantiated_at
                .insert(binding.clone(), branch.accumulated_cost);
        }

        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }

        {
            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.decayed.remove(binding);
            branch.consumed.insert(binding.clone());
        }
        Ok(())
    }

    pub(crate) fn Match(
        &mut self,
        target: &Expression,
        arms: &[MatchArm],
    ) -> Result<(), SemanticError> {
        crate::expression::analyze_expression_nonconsuming(self, target)?;
        for arm in arms {
            let original_state = self
                .branch_contexts
                .get(&self.current_branch)
                .cloned()
                .unwrap_or_default();
            let mut arm_contexts = self.branch_contexts.clone();
            arm_contexts.insert(self.current_branch.clone(), original_state.clone());
            let previous_contexts =
                std::mem::replace(&mut self.branch_contexts, arm_contexts);

            bind_pattern_variables(self, &arm.pattern);

            if let Some(ref guard) = arm.guard {
                crate::expression::analyze_expression(self, guard)?;
            }

            for inner_stmt in &arm.body {
                self.analyze_statement(inner_stmt)?;
            }

            self.branch_contexts = previous_contexts;
        }
        Ok(())
    }

    pub(crate) fn IfLet(
        &mut self,
        pattern: &Pattern,
        expr: &Expression,
        then_branch: &[SpannedStatement],
        else_branch: &Option<Vec<SpannedStatement>>,
        reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        crate::expression::analyze_expression_nonconsuming(self, expr)?;

        let original_state = self
            .branch_contexts
            .get(&self.current_branch)
            .cloned()
            .unwrap_or_default();

        let mut then_contexts = self.branch_contexts.clone();
        then_contexts.insert(self.current_branch.clone(), original_state.clone());
        let previous_contexts =
            std::mem::replace(&mut self.branch_contexts, then_contexts);

        bind_pattern_variables(self, pattern);

        for inner_stmt in then_branch {
            self.analyze_statement(inner_stmt)?;
        }

        let mut then_end_state = self
            .branch_contexts
            .get(&self.current_branch)
            .unwrap()
            .clone();

        let bound_vars = collect_pattern_bound_names(pattern);
        for var in &bound_vars {
            then_end_state.remove_variable_scope(var);
            then_end_state.produced.remove(var);
        }

        self.branch_contexts = previous_contexts;

        let else_end_state = if let Some(else_stmts) = else_branch {
            let mut else_contexts = self.branch_contexts.clone();
            else_contexts
                .insert(self.current_branch.clone(), original_state.clone());
            let prev = std::mem::replace(&mut self.branch_contexts, else_contexts);
            for inner_stmt in else_stmts {
                self.analyze_statement(inner_stmt)?;
            }
            let s = self
                .branch_contexts
                .get(&self.current_branch)
                .unwrap()
                .clone();
            self.branch_contexts = prev;
            s
        } else {
            original_state
        };

        let merged = self.merge_states(then_end_state, else_end_state, reconcile)?;
        self.branch_contexts
            .insert(self.current_branch.clone(), merged);

        Ok(())
    }
}

pub(crate) fn bind_pattern_variables(
    analyzer: &mut EntropicAnalyzer,
    pattern: &Pattern,
) {
    match pattern {
        Pattern::Wildcard => {}
        Pattern::Identifier(name) => {
            let branch = analyzer
                .branch_contexts
                .get_mut(&analyzer.current_branch)
                .unwrap();
            branch
                .types
                .insert(name.clone(), causm_core::types::Type::Unknown);
            branch.produced.insert(name.clone());
            branch
                .instantiated_at
                .insert(name.clone(), branch.accumulated_cost);
        }
        Pattern::Literal(_) => {}
        Pattern::EnumVariant { args, .. } => {
            for arg in args {
                bind_pattern_variables(analyzer, arg);
            }
        }
        Pattern::TypeAssert {
            binding,
            target_type,
        } => {
            let cast_type = causm_core::types::Type::from_typename(target_type);
            let branch = analyzer
                .branch_contexts
                .get_mut(&analyzer.current_branch)
                .unwrap();
            branch.types.insert(binding.clone(), cast_type);
            branch.produced.insert(binding.clone());
            branch
                .instantiated_at
                .insert(binding.clone(), branch.accumulated_cost);
        }
    }
}

pub(crate) fn collect_pattern_bound_names(pattern: &Pattern) -> Vec<String> {
    let mut vars = Vec::new();
    match pattern {
        Pattern::Wildcard | Pattern::Literal(_) => {}
        Pattern::Identifier(name) => vars.push(name.clone()),
        Pattern::EnumVariant { args, .. } => {
            for arg in args {
                vars.extend(collect_pattern_bound_names(arg));
            }
        }
        Pattern::TypeAssert { binding, .. } => vars.push(binding.clone()),
    }
    vars
}
