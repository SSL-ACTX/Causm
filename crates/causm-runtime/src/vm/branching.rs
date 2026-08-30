use crate::gc::GarbageCollector;
use crate::vm::error::TemporalError;
use crate::vm::state::{Timeline, Vm};
use causm_core::value::{EntropicState, ValueMetadata};
use causm_core::{CausalReversion, MergeResolution, ResolutionStrategy};
use std::collections::HashMap;

impl Vm {
    pub fn split_timeline(
        &mut self,
        parent_id: &str,
        branches: Vec<&str>,
    ) -> Result<(), TemporalError> {
        let (parent_timeline, parent_global_time) = {
            let parent = self.get_branch(parent_id)?;
            let birth_time = parent.birth_global_time + parent.local_clock;
            (parent.clone(), birth_time)
        };

        for branch_name in branches {
            let new_branch = Timeline::fork_from(
                branch_name.to_string(),
                &parent_timeline,
                parent_global_time,
            );
            self.active_branches
                .insert(branch_name.to_string(), new_branch);

            // Propagate entanglement groups to new branch
            let mut new_entries = Vec::new();
            for group in &self.entanglements {
                let mut found_parent = false;
                let mut regs_to_add = Vec::new();
                for (b, r) in group {
                    if b == parent_id {
                        found_parent = true;
                        regs_to_add.push(*r);
                    }
                }
                if found_parent {
                    new_entries.push(regs_to_add);
                }
            }

            for regs in new_entries {
                for r in regs {
                    for group in &mut self.entanglements {
                        if group.contains(&(parent_id.to_string(), r)) {
                            group.insert((branch_name.to_string(), r));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn merge_timelines(
        &mut self,
        branches: Vec<&str>,
        target: &str,
        resolution: &MergeResolution,
    ) -> Result<(), TemporalError> {
        let mut merged_registers: Vec<Option<EntropicState>> = Vec::new();
        let mut merged_metadata: Vec<Option<ValueMetadata>> = Vec::new();
        let mut pending_reversion = None;

        // Build a mapping from register ID to resolution strategy
        let mut reg_resolutions: HashMap<u32, ResolutionStrategy> = HashMap::new();
        for (name, strategy) in &resolution.rules {
            if let Some(reg) = self.symbols.get(name) {
                reg_resolutions.insert(reg.0, strategy.clone());
            }
        }

        // Pre-seed merged_registers from the target arena so registers already
        // written there (e.g. by RelativisticBlock sync) are not discarded by the merge.
        {
            let target_regs = if target == "main" {
                self.root_timeline.arena.registers.clone()
            } else {
                self.active_branches
                    .get(target)
                    .map(|b| b.arena.registers.clone())
                    .unwrap_or_default()
            };
            let target_meta = if target == "main" {
                self.root_timeline.arena.metadata.clone()
            } else {
                self.active_branches
                    .get(target)
                    .map(|b| b.arena.metadata.clone())
                    .unwrap_or_default()
            };
            merged_registers.resize(target_regs.len(), None);
            merged_metadata.resize(target_meta.len(), None);
            for (idx, state) in target_regs.iter().enumerate() {
                merged_registers[idx] = Some(state.clone());
            }
            for (idx, meta) in target_meta.iter().enumerate() {
                merged_metadata[idx] = meta.clone();
            }
        }

        for branch_name in &branches {
            let branch =
                self.active_branches.get(*branch_name).ok_or_else(|| {
                    TemporalError::BranchNotFound(branch_name.to_string())
                })?;

            if merged_registers.len() < branch.arena.registers.len() {
                merged_registers.resize(branch.arena.registers.len(), None);
            }
            if merged_metadata.len() < branch.arena.metadata.len() {
                merged_metadata.resize(branch.arena.metadata.len(), None);
            }

            for (idx, state) in branch.arena.registers.iter().enumerate() {
                let meta = branch.arena.metadata.get(idx).and_then(|m| m.clone());
                if let Some(existing) = &merged_registers[idx] {
                    let resolved = match (existing, state) {
                        (EntropicState::Consumed, other) => {
                            if idx < merged_metadata.len() {
                                merged_metadata[idx] = meta;
                            }
                            other.clone()
                        }
                        (other, EntropicState::Consumed) => other.clone(),
                        (ext, incoming) => {
                            let strategy = reg_resolutions
                                .get(&(idx as u32))
                                .unwrap_or(&ResolutionStrategy::Auto);
                            let (resolved, rev) = self
                                .resolve_entropic_conflict(
                                    &idx.to_string(),
                                    ext,
                                    incoming,
                                    strategy,
                                    branch_name,
                                );
                            if pending_reversion.is_none() {
                                pending_reversion = rev;
                            }
                            if idx < merged_metadata.len() && meta.is_some() {
                                merged_metadata[idx] = meta;
                            }
                            resolved
                        }
                    };
                    merged_registers[idx] = Some(resolved);
                } else {
                    if idx < merged_metadata.len() {
                        merged_metadata[idx] = meta;
                    }
                    merged_registers[idx] = Some(state.clone());
                }
            }
        }

        if let Some(reversion) = pending_reversion {
            let anchor = {
                let target_branch = self.get_branch_mut(&reversion.branch)?;
                target_branch
                    .anchors
                    .get(&reversion.anchor)
                    .ok_or_else(|| {
                        TemporalError::AnchorNotFound(reversion.anchor.clone())
                    })?
                    .clone()
            };

            let target_branch = self.get_branch_mut(&reversion.branch)?;
            target_branch.arena = anchor.arena_snapshot;
            target_branch.local_clock = anchor.clock_snapshot;
            target_branch.cpu_budget_ms = anchor.cpu_budget_snapshot;
            target_branch.resource_budgets = anchor.resource_budgets_snapshot;
            target_branch.commit_horizon_passed = false;
            target_branch.pc = anchor.pc_snapshot;

            return Ok(());
        }

        let target_branch = self.get_branch_mut(target)?;
        for (idx, v) in merged_registers.into_iter().enumerate() {
            if let Some(state) = v {
                let meta = merged_metadata.get(idx).and_then(|m| m.clone());
                target_branch.arena.ensure_register(idx as u32);
                if let Some(m) = meta {
                    target_branch.arena.insert_with_metadata(idx as u32, state, m)?;
                } else {
                    target_branch.arena.insert(idx as u32, state)?;
                }
            }
        }
        for b in branches {
            if let Some(branch) = self.active_branches.shift_remove(b) {
                GarbageCollector::collect_branch(branch);
            }
        }
        Ok(())
    }

    pub fn terminate_branch(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        if branch_id == "main" {
            return Err(TemporalError::BranchNotFound(branch_id.to_string()));
        }

        if self.active_branches.contains_key(branch_id) {
            GarbageCollector::collect_branch_by_id(self, branch_id);
            Ok(())
        } else {
            Err(TemporalError::BranchNotFound(branch_id.to_string()))
        }
    }

    pub fn step_back(&mut self, branch_id: &str) -> Result<(), TemporalError> {
        let mut found_idx = None;
        for (i, (b_id, _)) in self.causal_trace.iter().enumerate().rev() {
            if b_id == branch_id {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            let (_, snapshot) = self.causal_trace.remove(idx);
            if branch_id == "main" {
                self.root_timeline = snapshot;
            } else {
                self.active_branches.insert(branch_id.to_string(), snapshot);
            }
            Ok(())
        } else {
            Err(TemporalError::EvalError("No trace found for branch".into()))
        }
    }

    pub(crate) fn resolve_entropic_conflict(
        &self,
        _key: &str,
        existing: &EntropicState,
        incoming: &EntropicState,
        strategy: &ResolutionStrategy,
        incoming_branch: &str,
    ) -> (EntropicState, Option<CausalReversion>) {
        if matches!(existing, EntropicState::Consumed)
            || matches!(incoming, EntropicState::Consumed)
        {
            match strategy {
                ResolutionStrategy::Priority(p) => {
                    if incoming_branch == p {
                        return (incoming.clone(), None);
                    } else {
                        return (existing.clone(), None);
                    }
                }
                ResolutionStrategy::FirstWins => {
                    return (existing.clone(), None);
                }
                ResolutionStrategy::Auto => match (existing, incoming) {
                    (EntropicState::Consumed, other) => {
                        return (other.clone(), None)
                    }
                    (other, EntropicState::Consumed) => {
                        return (other.clone(), None)
                    }
                    _ => return (EntropicState::Consumed, None),
                },
                _ => {
                    return (EntropicState::Consumed, None);
                }
            }
        }

        match strategy {
            ResolutionStrategy::FirstWins => (existing.clone(), None),
            ResolutionStrategy::Priority(p) => {
                if incoming_branch == p {
                    (incoming.clone(), None)
                } else {
                    (existing.clone(), None)
                }
            }
            ResolutionStrategy::Decay => (EntropicState::Consumed, None),
            ResolutionStrategy::TopologyUnion {
                key_rules,
                default,
                on_invalid,
            } => {
                let f1_opt = match existing {
                    EntropicState::Valid(causm_core::value::Payload::Topology(
                        f,
                    )) => Some(f),
                    EntropicState::Decayed(f) => Some(f),
                    _ => None,
                };
                let f2_opt = match incoming {
                    EntropicState::Valid(causm_core::value::Payload::Topology(
                        f,
                    )) => Some(f),
                    EntropicState::Decayed(f) => Some(f),
                    _ => None,
                };

                if let (Some(f1), Some(f2)) = (f1_opt, f2_opt) {
                    let mut merged_fields = f1.clone();
                    let mut final_reversion = None;

                    for (field_name, incoming_f_state) in f2 {
                        if let Some(existing_f_state) = merged_fields.get(field_name)
                        {
                            let field_strategy =
                                key_rules.get(field_name).unwrap_or(default);
                            let (resolved_f, rev) = self.resolve_entropic_conflict(
                                field_name,
                                existing_f_state,
                                incoming_f_state,
                                field_strategy,
                                incoming_branch,
                            );
                            merged_fields.insert(field_name.clone(), resolved_f);
                            if final_reversion.is_none() {
                                final_reversion = rev;
                            }
                        } else {
                            merged_fields.insert(
                                field_name.clone(),
                                incoming_f_state.clone(),
                            );
                        }
                    }

                    let has_consumed = merged_fields
                        .values()
                        .any(|s| matches!(s, EntropicState::Consumed));

                    if has_consumed {
                        if let Some(rev) = on_invalid {
                            return (EntropicState::Consumed, Some(rev.clone()));
                        }
                    }

                    let merged_state = if has_consumed {
                        EntropicState::Decayed(merged_fields)
                    } else {
                        EntropicState::Valid(causm_core::value::Payload::Topology(
                            merged_fields,
                        ))
                    };

                    (merged_state, final_reversion)
                } else {
                    (EntropicState::Consumed, on_invalid.clone())
                }
            }
            ResolutionStrategy::Auto => {
                if existing == incoming {
                    (existing.clone(), None)
                } else {
                    match (existing, incoming) {
                        (EntropicState::Consumed, other) => (other.clone(), None),
                        (other, EntropicState::Consumed) => (other.clone(), None),
                        (_, incoming_valid @ EntropicState::Valid(_)) => (incoming_valid.clone(), None),
                        (existing_valid @ EntropicState::Valid(_), _) => (existing_valid.clone(), None),
                        _ => (incoming.clone(), None),
                    }
                }
            }
            _ => (existing.clone(), None),
        }
    }
}
