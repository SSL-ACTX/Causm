use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn RelativisticBlock(
        &mut self,
        time: &TimeCoordinate,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        let old_branch = self.current_branch.clone();
        if let TimeCoordinate::Branch(id) = time {
            if !self.branch_contexts.contains_key(id) {
                let parent_state = self
                    .branch_contexts
                    .get(&self.current_branch)
                    .cloned()
                    .unwrap_or_default();
                self.branch_contexts.insert(id.clone(), parent_state);
            }
            self.current_branch = id.clone();
        }
        for inner_stmt in body {
            self.analyze_statement(inner_stmt)?;
        }
        self.current_branch = old_branch;
        Ok(())
    }

    pub(crate) fn Isolate(
        &mut self,
        block: &IsolateBlock,
    ) -> Result<(), SemanticError> {
        let mut cap_set = std::collections::HashMap::new();
        for cap in &block.manifest.capabilities {
            let key = if let Some(id) = cap.parameters.get("id") {
                format!("{}[id={}]", cap.path, id)
            } else {
                cap.path.clone()
            };
            cap_set.insert(key, cap.clone());
        }
        self.capability_stack.push(cap_set);

        let previous_slice = self.current_slice_ms;
        if let Some(slice) = block.manifest.slice_ms {
            self.current_slice_ms = Some(slice);
        }

        for inner_stmt in &block.body {
            self.analyze_statement(inner_stmt)?;
        }

        self.current_slice_ms = previous_slice;
        self.capability_stack.pop();
        Ok(())
    }

    pub(crate) fn Slice(&mut self, milliseconds: &u64) -> Result<(), SemanticError> {
        self.current_slice_ms = Some(*milliseconds);
        Ok(())
    }

    pub(crate) fn Speculate(
        &mut self,
        _max_ms: &u64,
        body: &[SpannedStatement],
        fallback: &Option<Vec<SpannedStatement>>,
    ) -> Result<(), SemanticError> {
        let context_snapshot = self.branch_contexts.clone();

        for stmt in body {
            self.analyze_statement(stmt)?;
        }

        self.branch_contexts = context_snapshot.clone();

        if let Some(fallback_body) = fallback {
            for stmt in fallback_body {
                self.analyze_statement(stmt)?;
            }
        }

        self.branch_contexts = context_snapshot;
        Ok(())
    }

    pub(crate) fn Collapse(&mut self) -> Result<(), SemanticError> {
        Ok(())
    }

    pub(crate) fn AssertTime(
        &mut self,
        operator: &BinaryOperator,
        limit_ms: &u64,
        fallback: &Option<Vec<SpannedStatement>>,
    ) -> Result<(), SemanticError> {
        let current_wcet = {
            let branch = self.branch_contexts.get(&self.current_branch).unwrap();
            branch.accumulated_cost
        };

        let statically_violated = match operator {
            BinaryOperator::Lt => current_wcet >= *limit_ms,
            BinaryOperator::Le => current_wcet > *limit_ms,
            _ => false,
        };

        if statically_violated && fallback.is_none() {
            return Err(self.annotate(
                SemanticErrorKind::TemporalAssertionViolation(
                    current_wcet,
                    *limit_ms,
                ),
            ));
        }

        if let Some(fb) = fallback {
            for inner_stmt in fb {
                self.analyze_statement(inner_stmt)?;
            }
        }
        Ok(())
    }

    pub(crate) fn Await(&mut self, target: &str) -> Result<(), SemanticError> {
        self.check_available(target)?;
        let target_type = self
            .get_variable_type(target)
            .unwrap_or(causm_core::types::Type::Unknown);
        match target_type {
            causm_core::types::Type::Promise(inner) => {
                self.set_variable_type(target, *inner);
            }
            causm_core::types::Type::Unknown => {}
            other => {
                return Err(self.annotate(SemanticErrorKind::TypeMismatch(
                    format!("await target must be a Promise, got {:?}", other),
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn Lease(
        &mut self,
        binding: &String,
        source: &String,
        duration_ms: &u64,
        body: &[SpannedStatement],
        _reconcile: &Option<MergeResolution>,
    ) -> Result<(), SemanticError> {
        self.check_available(source)?;

        {
            let branch = self.branch_contexts.get(&self.current_branch).unwrap();
            if branch.leased.contains(source) {
                return Err(
                    self.annotate(SemanticErrorKind::NestedLeasing(source.clone()))
                );
            }
        }

        let source_type = self
            .get_variable_type(source)
            .unwrap_or(causm_core::types::Type::Unknown);

        // Mark source as leased
        {
            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.leased.insert(source.clone());
            branch.lease_bindings.insert(binding.clone());
        }

        let old_binding_type = self.get_variable_type(binding);

        // Set binding type
        self.set_variable_type(binding, source_type);

        // Analyze block
        let block_wcet = crate::statement::estimate_block_cost(self, body);
        if block_wcet > *duration_ms {
            return Err(self.annotate(SemanticErrorKind::LeaseDurationExceeded(
                block_wcet,
                *duration_ms,
            )));
        }

        for inner_stmt in body {
            if matches!(inner_stmt.stmt, Statement::Break | Statement::Return(_)) {
                return Err(
                    self.annotate(SemanticErrorKind::IllegalLeaseControlFlow)
                );
            }
            self.analyze_statement(inner_stmt)?;
        }

        // Relinquish lease
        {
            let branch = self.branch_contexts.get_mut(&self.current_branch).unwrap();
            branch.leased.remove(source);
            branch.lease_bindings.remove(binding);
            branch.accumulated_cost += *duration_ms; // Jump to end of lease duration
        }

        if let Some(old_ty) = old_binding_type {
            self.set_variable_type(binding, old_ty);
        } else {
            self.remove_variable_scope(binding);
        }

        Ok(())
    }
}
