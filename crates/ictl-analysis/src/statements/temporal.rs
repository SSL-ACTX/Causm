use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use ictl_core::*;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn RelativisticBlock(
        &mut self,
        time: &TimeCoordinate,
        body: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        let old_branch = self.current_branch.clone();
        if let TimeCoordinate::Branch(id) = time {
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
            cap_set.insert(cap.path.clone(), cap.clone());
        }
        self.capability_stack.push(cap_set);

        let previous_slice = self.current_slice_ms;
        self.current_slice_ms = block.manifest.cpu_budget_ms;

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

    pub(crate) fn Watchdog(
        &mut self,
        _target: &String,
        _timeout_ms: &u64,
        recovery: &[SpannedStatement],
    ) -> Result<(), SemanticError> {
        for inner_stmt in recovery {
            self.analyze_statement(inner_stmt)?;
        }
        Ok(())
    }

    pub(crate) fn Await(&mut self, target: &String) -> Result<(), SemanticError> {
        let state = self.branch_contexts.get(&self.current_branch).unwrap();
        if state.consumed.contains(target) {
            return Err(
                self.annotate(SemanticErrorKind::UseAfterConsume(target.clone()))
            );
        }
        Ok(())
    }
}
