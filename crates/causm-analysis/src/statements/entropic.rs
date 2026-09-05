use crate::analyzer::{
    BranchState, EntropicAnalyzer, SemanticError, SemanticErrorKind,
};
use causm_core::*;
use std::collections::HashSet;

#[allow(non_snake_case, unused_variables)]
impl EntropicAnalyzer {
    pub(crate) fn Split(
        &mut self,
        parent: &String,
        branches: &[String],
    ) -> Result<(), SemanticError> {
        if !self.branch_contexts.contains_key(parent)
            || self.merged_branches.contains(parent)
        {
            return Err(
                self.annotate(SemanticErrorKind::InactiveTimeline(parent.clone()))
            );
        }

        let parent_state = self
            .branch_contexts
            .get(parent)
            .cloned()
            .unwrap_or_default();

        for branch in branches {
            self.merged_branches.remove(branch);
            self.branch_contexts.insert(
                branch.clone(),
                BranchState {
                    consumed: parent_state.consumed.clone(),
                    decayed: parent_state.decayed.clone(),
                    yields: HashSet::new(),
                    produced: HashSet::new(),
                    leased: parent_state.leased.clone(),
                    lease_bindings: parent_state.lease_bindings.clone(),
                    mutables: parent_state.mutables.clone(),
                    types: parent_state.types.clone(),
                    custom_types: parent_state.custom_types.clone(),
                    accumulated_cost: parent_state.accumulated_cost,
                    instantiated_at: parent_state.instantiated_at.clone(),
                },
            );
        }
        self.mark_consumed(parent)?;
        Ok(())
    }

    pub(crate) fn Merge(
        &mut self,
        branches: &[String],
        target: &String,
        resolutions: &MergeResolution,
    ) -> Result<(), SemanticError> {
        let mut all_defined = HashSet::new();
        let mut collisions = HashSet::new();

        for branch_name in branches {
            if self.merged_branches.contains(branch_name) {
                return Err(self.annotate(SemanticErrorKind::InactiveTimeline(
                    branch_name.clone(),
                )));
            }

            let branch_state =
                self.branch_contexts.get(branch_name).ok_or_else(|| {
                    self.annotate(SemanticErrorKind::InactiveTimeline(
                        branch_name.clone(),
                    ))
                })?;

            for (var, typ) in &branch_state.types {
                let resolved = self.resolve_type(typ);
                if let causm_core::types::Type::Struct(s) = resolved {
                    if let Some(scope) = s.scoped_branch {
                        if &scope != target {
                            return Err(self.annotate(
                                SemanticErrorKind::InvalidTimelineMove(
                                    var.clone(),
                                    scope.clone(),
                                    target.clone(),
                                ),
                            ));
                        }
                    }
                }
            }

            for var in &branch_state.produced {
                if !all_defined.insert(var.clone()) {
                    collisions.insert(var.clone());
                }
            }
        }

        if !resolutions.auto {
            for key in collisions {
                if !resolutions.rules.contains_key(&key) {
                    return Err(self.annotate(SemanticErrorKind::UnresolvedMerge(key)));
                }
            }
        }

        let mut merged_types = std::collections::HashMap::new();
        for branch_name in branches {
            let branch_state = self.branch_contexts.get(branch_name).unwrap();
            for (var, typ) in &branch_state.types {
                merged_types.insert(var.clone(), typ.clone());
            }
        }

        let target_state = self.branch_contexts.entry(target.clone()).or_default();
        for (var, typ) in merged_types {
            target_state.types.insert(var.clone(), typ);
            target_state.consumed.remove(&var);
        }
        target_state.consumed.remove(target);

        for branch_name in branches {
            self.merged_branches.insert(branch_name.clone());
        }
        self.merged_branches.remove(target);

        Ok(())
    }

    pub(crate) fn Anchor(&mut self, _name: &String) -> Result<(), SemanticError> {
        if self.entropy_mode == causm_core::EntropyMode::Chaos {
            return Err(self.annotate(SemanticErrorKind::ChaosModePreventsRewind));
        }
        if let Some(cap) = self.get_capability("System.Entropy") {
            if cap.parameters.get("mode").map(|s| s.as_str()) == Some("chaos") {
                return Err(
                    self.annotate(SemanticErrorKind::ChaosModePreventsRewind)
                );
            }
        }
        Ok(())
    }

    pub(crate) fn Rewind(&mut self, _name: &String) -> Result<(), SemanticError> {
        if self.entropy_mode == causm_core::EntropyMode::Chaos {
            return Err(self.annotate(SemanticErrorKind::ChaosModePreventsRewind));
        }
        if let Some(cap) = self.get_capability("System.Entropy") {
            if cap.parameters.get("mode").map(|s| s.as_str()) == Some("chaos") {
                return Err(
                    self.annotate(SemanticErrorKind::ChaosModePreventsRewind)
                );
            }
        }
        Ok(())
    }

    pub(crate) fn Entangle(
        &mut self,
        variables: &[String],
    ) -> Result<(), SemanticError> {
        for var in variables {
            if let Some(typ) = self.get_variable_type(var) {
                let resolved = self.resolve_type(&typ);
                if let causm_core::types::Type::Struct(s) = resolved {
                    if let Some(scope) = s.scoped_branch {
                        if scope != self.current_branch {
                            return Err(self.annotate(
                                SemanticErrorKind::InvalidTimelineMove(
                                    var.clone(),
                                    scope.clone(),
                                    self.current_branch.clone(),
                                ),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
