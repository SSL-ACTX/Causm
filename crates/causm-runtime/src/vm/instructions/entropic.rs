use crate::vm::error::TemporalError;
use crate::vm::state::{AnchorPoint, Vm};
use causm_core::value::{EntropicState, Payload};
use causm_ir::Reg;
use std::collections::HashSet;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn Consume(
        &mut self,
        branch_id: &str,
        src: Reg,
    ) -> Result<(), TemporalError> {
        self.consume_reg(branch_id, src.0)
    }

    pub(crate) fn AutoDrop(
        &mut self,
        branch_id: &str,
        target: Reg,
        spec: causm_core::types::AutoDropSpec,
    ) -> Result<(), TemporalError> {
        if let Ok(causm_core::value::Payload::Struct(fields)) =
            self.peek_reg(branch_id, target.0)
        {
            if let Some(causm_core::value::EntropicState::Valid(ref field_val)) =
                fields.get(&spec.field_name)
            {
                if let Ok(sym_ptr) = self
                    .foreign_manager
                    .get_or_load_symbol(&spec.lib_name, &spec.routine_name)
                {
                    unsafe {
                        let _ = crate::vm::ffi::invoke_foreign_symbol(
                            sym_ptr,
                            std::slice::from_ref(field_val),
                            &causm_core::types::Type::I32,
                        );
                    }
                }
            }
        }
        self.consume_reg(branch_id, target.0)
    }

    pub(crate) fn ConsumeField(
        &mut self,
        branch_id: &str,
        src: Reg,
        field: String,
    ) -> Result<(), TemporalError> {
        self.consume_field_reg(branch_id, src.0, &field)
    }

    pub(crate) fn Clone(
        &mut self,
        branch_id: &str,
        dest: Reg,
        src: Reg,
    ) -> Result<(), TemporalError> {
        self.check_and_apply_decay(branch_id, src.0)?;
        let (val, metadata) = {
            let branch = self.get_branch_mut(branch_id)?;
            let payload =
                branch.arena.peek(src.0).ok_or(TemporalError::MemoryFault(
                    causm_core::value::MemoryError::AlreadyConsumed,
                ))?;
            let cost = branch.arena.calculate_clone_cost(&payload, 1);
            branch.consume_budget(cost)?;
            let metadata = branch
                .arena
                .metadata
                .get(src.0 as usize)
                .and_then(|m| m.clone());
            (payload, metadata)
        };
        let branch = self.get_branch_mut(branch_id)?;
        if let Some(meta) = metadata {
            branch.arena.insert_with_metadata(
                dest.0,
                EntropicState::Valid(val),
                meta,
            )?;
        } else {
            branch.arena.insert(dest.0, EntropicState::Valid(val))?;
        }
        Ok(())
    }

    pub(crate) fn Entangle(
        &mut self,
        branch_id: &str,
        regs: Vec<Reg>,
    ) -> Result<(), TemporalError> {
        let mut set = HashSet::new();
        for reg in regs {
            set.insert((branch_id.to_string(), reg.0));
        }
        // Check if any of these registers are already entangled
        let mut existing_set_idx = None;
        for (i, entangled_set) in self.entanglements.iter().enumerate() {
            if entangled_set
                .iter()
                .any(|(b, r)| set.contains(&(b.clone(), *r)))
            {
                existing_set_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = existing_set_idx {
            self.entanglements[idx].extend(set);
        } else {
            self.entanglements.push(set);
        }
        Ok(())
    }

    pub(crate) fn Anchor(
        &mut self,
        branch_id: &str,
        name: String,
    ) -> Result<(), TemporalError> {
        let history_index = self.causal_history.len();
        let branch = self.get_branch_mut(branch_id)?;
        if branch.entropy_mode == causm_core::EntropyMode::Chaos {
            return Err(TemporalError::RewindDisabledInChaos);
        }
        let snapshot = AnchorPoint {
            name: name.clone(),
            clock_snapshot: branch.local_clock,
            arena_snapshot: branch.arena.clone(),
            cpu_budget_snapshot: branch.cpu_budget_ms,
            resource_budgets_snapshot: branch.resource_budgets.clone(),
            history_index,
            pc_snapshot: branch.pc,
            instructions_snapshot: branch.instructions.clone(),
            spans_snapshot: branch.spans.clone(),
        };
        branch.anchors.insert(name, snapshot);
        Ok(())
    }

    pub(crate) fn Rewind(
        &mut self,
        branch_id: &str,
        target: String,
        anchor: String,
    ) -> Result<(), TemporalError> {
        let target_id = if target == "self" { branch_id } else { &target };
        {
            let t_branch = self.get_branch_mut(target_id)?;
            if t_branch.entropy_mode == causm_core::EntropyMode::Chaos {
                return Err(TemporalError::RewindDisabledInChaos);
            }
        }
        let anchor_data = {
            let t_branch = self.get_branch_mut(target_id)?;
            t_branch
                .anchors
                .get(&anchor)
                .cloned()
                .ok_or_else(|| TemporalError::AnchorNotFound(anchor.clone()))?
        };

        // Perform causal rollback
        self._causal_rollback(target_id, anchor_data.history_index)?;

        let t_branch = self.get_branch_mut(target_id)?;
        t_branch.arena = anchor_data.arena_snapshot;
        t_branch.local_clock = anchor_data.clock_snapshot;
        t_branch.cpu_budget_ms = anchor_data.cpu_budget_snapshot;
        t_branch.resource_budgets = anchor_data.resource_budgets_snapshot;
        t_branch.pc = anchor_data.pc_snapshot;
        t_branch.instructions = anchor_data.instructions_snapshot;
        t_branch.spans = anchor_data.spans_snapshot;
        t_branch.commit_horizon_passed = false;
        Ok(())
    }

    pub(crate) fn ConsumeFieldDynamic(
        &mut self,
        branch_id: &str,
        target: Reg,
        index: Reg,
    ) -> Result<(), TemporalError> {
        let idx_val = self.peek_reg(branch_id, index.0)?;
        let idx_str = match idx_val {
            causm_core::value::Payload::String(s) => s,
            causm_core::value::Payload::Integer(i) => i.to_string(),
            _ => {
                return Err(TemporalError::EvalError(
                    "Index must be string or integer".into(),
                ))
            }
        };
        self.consume_field_reg(branch_id, target.0, &idx_str)
    }
}
