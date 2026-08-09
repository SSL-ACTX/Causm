use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::value::{EntropicState, Payload, ValueMetadata};
use causm_ir::Reg;
use std::collections::HashMap;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn StructLit(
        &mut self,
        branch_id: &str,
        dest: Reg,
        fields: HashMap<String, Reg>,
        type_name: Option<String>,
    ) -> Result<(), TemporalError> {
        let mut evaluated_fields = HashMap::new();
        for (name, reg) in fields {
            let val = self.peek_reg(branch_id, reg.0)?;
            evaluated_fields.insert(name.clone(), EntropicState::Valid(val));
        }

        let decay_after_ms = type_name
            .as_ref()
            .and_then(|name| self.type_decay_limits.get(name))
            .cloned();
        let global_time = self.global_clock;

        let branch = self.get_branch_mut(branch_id)?;
        let meta = ValueMetadata {
            instantiated_at: global_time + branch.local_clock,
            type_name: type_name.clone(),
            decay_after_ms,
        };

        branch.arena.insert_with_metadata(
            dest.0,
            EntropicState::Valid(Payload::Struct(evaluated_fields)),
            meta,
        )?;
        Ok(())
    }

    pub(crate) fn TypeAssert(
        &mut self,
        branch_id: &str,
        dest: Reg,
        src: Reg,
        type_name: String,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let meta = {
            let branch = self.get_branch_mut(branch_id)?;
            branch
                .arena
                .metadata
                .get(src.0 as usize)
                .and_then(|m| m.as_ref())
                .cloned()
        };

        let matches = match &meta {
            Some(m) => m.type_name.as_ref() == Some(&type_name),
            None => false,
        };

        if !matches {
            return Err(TemporalError::EvalError(format!(
                "Type assertion failed: expected {}, got {:?}",
                type_name,
                meta.and_then(|m| m.type_name)
            )));
        }

        self.insert_reg(
            branch_id,
            dest.0,
            causm_core::value::EntropicState::Valid(val),
        )?;
        if let Some(m) = meta {
            let branch = self.get_branch_mut(branch_id)?;
            branch.arena.metadata[dest.0 as usize] = Some(m);
        }
        Ok(())
    }

    pub(crate) fn TypeCast(
        &mut self,
        branch_id: &str,
        dest: Reg,
        src: Reg,
        target_type: causm_core::TypeName,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let target_ty = causm_core::types::Type::from_typename(&target_type);

        fn cast_payload(
            p: Payload,
            target_ty: &causm_core::types::Type,
        ) -> Result<Payload, TemporalError> {
            match (p, target_ty) {
                (
                    Payload::Integer(i),
                    causm_core::types::Type::Float
                    | causm_core::types::Type::F32
                    | causm_core::types::Type::F64,
                ) => Ok(Payload::Float((i as f64).to_bits())),
                (
                    Payload::Float(bits),
                    causm_core::types::Type::Integer
                    | causm_core::types::Type::I8
                    | causm_core::types::Type::I16
                    | causm_core::types::Type::I32
                    | causm_core::types::Type::I64
                    | causm_core::types::Type::U8
                    | causm_core::types::Type::U16
                    | causm_core::types::Type::U32
                    | causm_core::types::Type::U64,
                ) => {
                    let f = f64::from_bits(bits);
                    Ok(Payload::Integer(f as i64))
                }
                (Payload::Integer(i), causm_core::types::Type::Bool) => {
                    Ok(Payload::Bool(i != 0))
                }
                (Payload::Bool(b), causm_core::types::Type::Integer) => {
                    Ok(Payload::Integer(if b { 1 } else { 0 }))
                }
                (Payload::Array(vec), causm_core::types::Type::Array(inner)) => {
                    let mut casted = Vec::new();
                    for elem in vec {
                        casted.push(cast_payload(elem, inner)?);
                    }
                    Ok(Payload::Array(casted))
                }
                (other, _) => Ok(other),
            }
        }

        let res = cast_payload(val, &target_ty)?;
        self.insert_reg(branch_id, dest.0, EntropicState::Valid(res))
    }

    pub(crate) fn AssertState(
        &mut self,
        branch_id: &str,
        src: Reg,
        state: String,
    ) -> Result<(), TemporalError> {
        let entropic_state = self.peek_state(branch_id, src.0)?;
        let state_name = match entropic_state {
            causm_core::value::EntropicState::Valid(_) => "Valid",
            causm_core::value::EntropicState::Leased { .. } => "Leased",
            causm_core::value::EntropicState::Decayed(_) => "Decayed",
            causm_core::value::EntropicState::Pending(_) => "Pending",
            causm_core::value::EntropicState::Consumed => "Consumed",
        };
        if state_name != state {
            return Err(TemporalError::EvalError(format!(
                "State constraint violated: expected receiver to be in state '{}', but was in state '{}'",
                state, state_name
            )));
        }
        Ok(())
    }

    pub(crate) fn TryTypeAssert(
        &mut self,
        branch_id: &str,
        dest: Reg,
        src: Reg,
        type_name: String,
        success: Reg,
    ) -> Result<(), TemporalError> {
        let meta = {
            let branch = self.get_branch_mut(branch_id)?;
            branch
                .arena
                .metadata
                .get(src.0 as usize)
                .and_then(|m| m.as_ref())
                .cloned()
        };

        let matches = match &meta {
            Some(m) => m.type_name.as_ref() == Some(&type_name),
            None => false,
        };

        if matches {
            let val = self.peek_reg(branch_id, src.0)?;
            self.insert_reg(
                branch_id,
                dest.0,
                causm_core::value::EntropicState::Valid(val),
            )?;
            if let Some(m) = meta {
                let branch = self.get_branch_mut(branch_id)?;
                branch.arena.metadata[dest.0 as usize] = Some(m);
            }
            self.insert_reg(
                branch_id,
                success.0,
                causm_core::value::EntropicState::Valid(
                    causm_core::value::Payload::Bool(true),
                ),
            )?;
        } else {
            self.insert_reg(
                branch_id,
                success.0,
                causm_core::value::EntropicState::Valid(
                    causm_core::value::Payload::Bool(false),
                ),
            )?;
        }
        Ok(())
    }

    pub(crate) fn TopologyLit(
        &mut self,
        branch_id: &str,
        dest: Reg,
        fields: HashMap<String, Reg>,
    ) -> Result<(), TemporalError> {
        let mut evaluated_fields = HashMap::new();
        for (name, reg) in fields {
            let val = self.peek_reg(branch_id, reg.0)?;
            evaluated_fields.insert(name.clone(), EntropicState::Valid(val));
        }
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Topology(evaluated_fields)),
        )
    }

    pub(crate) fn ArrayLit(
        &mut self,
        branch_id: &str,
        dest: Reg,
        elements: Vec<Reg>,
    ) -> Result<(), TemporalError> {
        let mut values = Vec::new();
        for reg in elements {
            let val = self.peek_reg(branch_id, reg.0)?;
            values.push(val);
        }
        self.insert_reg(
            branch_id,
            dest.0,
            EntropicState::Valid(Payload::Array(values)),
        )
    }

    pub(crate) fn FieldAccess(
        &mut self,
        branch_id: &str,
        dest: Reg,
        target: Reg,
        field: String,
    ) -> Result<(), TemporalError> {
        self.check_and_apply_decay(branch_id, target.0)?;
        let field_state = {
            let branch = self.get_branch_mut(branch_id)?;
            branch
                .arena
                .consume_field_entropic(target.0, &field)
                .map_err(TemporalError::MemoryFault)?
        };

        let time = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.birth_global_time + branch.local_clock
        };

        self.causal_history
            .push(crate::vm::state::CausalEvent::Decay {
                branch_id: branch_id.to_string(),
                reg: target.0,
                field: field.clone(),
                time,
            });

        self.insert_reg(branch_id, dest.0, field_state)?;
        self.propagate_field_decay(branch_id, target.0, &field)
    }

    pub(crate) fn FieldUpdate(
        &mut self,
        branch_id: &str,
        target: Reg,
        field: String,
        src: Reg,
    ) -> Result<(), TemporalError> {
        self.check_and_apply_decay(branch_id, target.0)?;
        let val = self.peek_reg(branch_id, src.0)?;
        let branch = self.get_branch_mut(branch_id)?;
        branch.arena.update_field(target.0, &field, val)?;
        Ok(())
    }

    pub(crate) fn IndexAccess(
        &mut self,
        branch_id: &str,
        dest: Reg,
        target: Reg,
        index: Reg,
    ) -> Result<(), TemporalError> {
        let target_val = self.peek_reg(branch_id, target.0)?;
        let idx_val = self.peek_reg(branch_id, index.0)?;
        let idx_str = match idx_val {
            Payload::String(s) => s,
            Payload::Integer(i) => i.to_string(),
            _ => {
                return Err(TemporalError::EvalError(
                    "Index must be string or integer".into(),
                ))
            }
        };
        let state = match target_val {
            Payload::Struct(fields) | Payload::Topology(fields) => fields
                .get(&idx_str)
                .cloned()
                .unwrap_or(EntropicState::Consumed),
            _ => EntropicState::Consumed,
        };
        self.insert_reg(branch_id, dest.0, state)
    }

    pub(crate) fn IndexFieldUpdate(
        &mut self,
        branch_id: &str,
        target: Reg,
        index: Reg,
        field: String,
        src: Reg,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, src.0)?;
        let idx_val = self.peek_reg(branch_id, index.0)?;
        let idx_str = match idx_val {
            Payload::String(s) => s,
            Payload::Integer(i) => i.to_string(),
            _ => {
                return Err(TemporalError::EvalError(
                    "Index must be string or integer".into(),
                ))
            }
        };

        let branch = self.get_branch_mut(branch_id)?;
        branch
            .arena
            .update_index_field(target.0, &idx_str, &field, val)?;
        Ok(())
    }
}
