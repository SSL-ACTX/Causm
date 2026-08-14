use crate::vm::error::TemporalError;
use crate::vm::state::Vm;

impl Vm {
    pub fn propagate_entanglement(
        &mut self,
        source_branch: &str,
        reg: u32,
    ) -> Result<(), TemporalError> {
        let mut groups_to_propagate = Vec::new();
        for (i, group) in self.entanglements.iter().enumerate() {
            if group.contains(&(source_branch.to_string(), reg)) {
                groups_to_propagate.push(i);
            }
        }

        for idx in groups_to_propagate {
            let group = self.entanglements[idx].clone();
            for (target_branch, target_reg) in group {
                if target_branch == source_branch && target_reg == reg {
                    continue;
                }
                if let Ok(branch) = self.get_branch_mut(&target_branch) {
                    branch.arena.set_consumed(target_reg).ok();
                }
            }
        }
        Ok(())
    }

    pub fn propagate_field_decay(
        &mut self,
        source_branch: &str,
        reg: u32,
        field_name: &str,
    ) -> Result<(), TemporalError> {
        let mut groups_to_propagate = Vec::new();
        for (i, group) in self.entanglements.iter().enumerate() {
            if group.contains(&(source_branch.to_string(), reg)) {
                groups_to_propagate.push(i);
            }
        }

        for idx in groups_to_propagate {
            let group = self.entanglements[idx].clone();
            for (target_branch, target_reg) in group {
                if target_branch == source_branch && target_reg == reg {
                    continue;
                }
                if let Ok(branch) = self.get_branch_mut(&target_branch) {
                    branch.arena.consume_field(target_reg, field_name).ok();
                }
            }
        }
        Ok(())
    }

    pub fn consume_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
    ) -> Result<(), TemporalError> {
        let mut to_consume = Vec::new();
        to_consume.push((branch_id.to_string(), reg));

        let mut entangled_found = true;
        while entangled_found {
            entangled_found = false;
            let current_to_consume = to_consume.clone();
            for set in &self.entanglements {
                if current_to_consume.iter().any(|item| set.contains(item)) {
                    for entangled in set {
                        if !to_consume.contains(entangled) {
                            to_consume.push(entangled.clone());
                            entangled_found = true;
                        }
                    }
                }
            }
        }

        for (b_id, r_id) in to_consume {
            self.trigger_auto_drop(&b_id, r_id);
            if let Ok(branch) = self.get_branch_mut(&b_id) {
                branch.arena.consume(r_id).ok();
            }
        }
        Ok(())
    }

    pub fn trigger_auto_drop(&mut self, branch_id: &str, reg: u32) {
        if let Ok(causm_core::value::Payload::Struct(fields)) =
            self.peek_reg(branch_id, reg)
        {
            for spec in self.auto_drop_specs.values() {
                if let Some(causm_core::value::EntropicState::Valid(ref field_val)) =
                    fields.get(&spec.field_name)
                {
                    if let Ok(sym_ptr) = self
                        .foreign_manager
                        .get_or_load_symbol(&spec.lib_name, &spec.routine_name)
                    {
                        unsafe {
                            let mut args = [field_val.clone()];
                            let _ = crate::vm::ffi::invoke_foreign_symbol(
                                sym_ptr,
                                &mut args,
                                &causm_core::types::Type::I32,
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn consume_field_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
        field: &str,
    ) -> Result<(), TemporalError> {
        let mut to_consume = Vec::new();
        to_consume.push((branch_id.to_string(), reg));

        let mut entangled_found = true;
        while entangled_found {
            entangled_found = false;
            let current_to_consume = to_consume.clone();
            for set in &self.entanglements {
                if current_to_consume.iter().any(|item| set.contains(item)) {
                    for entangled in set {
                        if !to_consume.contains(entangled) {
                            to_consume.push(entangled.clone());
                            entangled_found = true;
                        }
                    }
                }
            }
        }

        for (b_id, r_id) in to_consume {
            if let Ok(branch) = self.get_branch_mut(&b_id) {
                branch.arena.consume_field(r_id, field).ok();
            }
        }
        Ok(())
    }
}
