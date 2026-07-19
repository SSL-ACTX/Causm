use crate::vm::error::TemporalError;
use crate::vm::state::{Timeline, Vm};
use causm_core::value::{EntropicState, Payload};
use causm_core::{MergeResolution, ParamMode};
use causm_ir::{Instruction, Reg};
use std::collections::HashMap;

#[allow(non_snake_case)]
impl Vm {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn For(
        &mut self,
        branch_id: &str,
        item_name: String,
        mode: ParamMode,
        source: Reg,
        body: Vec<Instruction>,
        pacing_ms: Option<u64>,
        max_ms: Option<u64>,
    ) -> Result<(), TemporalError> {
        let source_payload = match mode {
            ParamMode::Consume | ParamMode::Decay => {
                let payload = self.peek_reg(branch_id, source.0)?;
                self.consume_reg(branch_id, source.0)?;
                payload
            }
            ParamMode::Clone | ParamMode::Peek => {
                self.peek_reg(branch_id, source.0)?
            }
        };

        let elements = match source_payload {
            Payload::Array(vec) => vec,
            Payload::Struct(map) | Payload::Topology(map) => {
                let mut vec = Vec::new();
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                for k in keys {
                    if let Some(EntropicState::Valid(p)) = map.get(k) {
                        let mut fields = HashMap::new();
                        fields.insert(
                            "key".to_string(),
                            EntropicState::Valid(Payload::String(k.clone())),
                        );
                        fields.insert(
                            "value".to_string(),
                            EntropicState::Valid(p.clone()),
                        );
                        vec.push(Payload::Struct(fields));
                    }
                }
                vec
            }
            _ => {
                return Err(TemporalError::EvalError(
                    "for-source must be array or struct".into(),
                ))
            }
        };

        let start_local_clock = self.get_branch(branch_id)?.local_clock;
        let mut elapsed = 0;
        let max_allowed = max_ms.unwrap_or(u64::MAX);

        let item_reg = self.symbols.get(&item_name).unwrap().0;

        for item_value in elements.into_iter() {
            if elapsed >= max_allowed {
                break;
            }

            self.insert_reg(branch_id, item_reg, EntropicState::Valid(item_value))?;

            let iteration_start = self.get_branch_mut(branch_id)?.local_clock;

            // Execute body
            let (old_pc, old_instrs) = {
                let b = self.get_branch_mut(branch_id)?;
                let pc = b.pc;
                let instrs = b.instructions.clone();
                b.instructions = body.clone();
                b.pc = 0;
                (pc, instrs)
            };

            while {
                let b = self.get_branch_mut(branch_id)?;
                b.pc < b.instructions.len()
            } {
                self.execute_instruction(branch_id)?;
            }

            {
                let b = self.get_branch_mut(branch_id)?;
                b.instructions = old_instrs;
                b.pc = old_pc;
            }

            let body_cost =
                self.get_branch_mut(branch_id)?.local_clock - iteration_start;
            let paced = pacing_ms.unwrap_or(body_cost);

            if body_cost > paced {
                return Err(TemporalError::PacingViolation);
            }

            let pad = paced - body_cost;
            if pad > 0 {
                let branch = self.get_branch_mut(branch_id)?;
                branch.local_clock += pad;
                branch.consume_budget(pad)?;
            }

            elapsed += paced;
        }

        if let Some(max) = max_ms {
            let total_elapsed =
                self.get_branch(branch_id)?.local_clock - start_local_clock;
            if total_elapsed < max {
                let pad = max - total_elapsed;
                let branch = self.get_branch_mut(branch_id)?;
                branch.local_clock += pad;
                branch.consume_budget(pad)?;
            }
        }

        Ok(())
    }

    pub(crate) fn SplitMap(
        &mut self,
        branch_id: &str,
        item_name: String,
        mode: ParamMode,
        source: Reg,
        body: Vec<Instruction>,
        _reconcile: Option<MergeResolution>,
    ) -> Result<(), TemporalError> {
        let source_payload = match mode {
            ParamMode::Consume | ParamMode::Decay => {
                let payload = self.peek_reg(branch_id, source.0)?;
                self.consume_reg(branch_id, source.0)?;
                payload
            }
            ParamMode::Clone | ParamMode::Peek => {
                self.peek_reg(branch_id, source.0)?
            }
        };
        let elements = match source_payload {
            Payload::Array(vec) => vec,
            Payload::Struct(map) | Payload::Topology(map) => {
                let mut vec = Vec::new();
                let mut keys: Vec<_> = map.keys().collect();
                keys.sort();
                for k in keys {
                    if let Some(EntropicState::Valid(p)) = map.get(k) {
                        let mut fields = HashMap::new();
                        fields.insert(
                            "key".to_string(),
                            EntropicState::Valid(Payload::String(k.clone())),
                        );
                        fields.insert(
                            "value".to_string(),
                            EntropicState::Valid(p.clone()),
                        );
                        vec.push(Payload::Struct(fields));
                    }
                }
                vec
            }
            _ => {
                return Err(TemporalError::EvalError(
                    "split_map source must be array or struct".into(),
                ))
            }
        };

        let mut results: Vec<Payload> = Vec::new();
        let item_reg = self.symbols.get(&item_name).unwrap().0;

        for item_value in elements.into_iter() {
            let child_name = format!("splitmap_{}", results.len());
            let child_snapshot = self.get_branch_mut(branch_id)?.clone();

            self.active_branches
                .insert(child_name.clone(), child_snapshot);
            {
                let child_branch = self.get_branch_mut(&child_name)?;
                child_branch
                    .arena
                    .insert(item_reg, EntropicState::Valid(item_value))?;
                child_branch.instructions = body.clone();
                child_branch.pc = 0;
            }

            while {
                let b = self.get_branch_mut(&child_name)?;
                b.pc < b.instructions.len()
            } {
                self.execute_instruction(&child_name)?;
            }

            let child_branch = self
                .active_branches
                .remove(&child_name)
                .ok_or_else(|| TemporalError::BranchNotFound(child_name.clone()))?;

            let yielded = child_branch.arena.peek(0);
            if let Some(p) = yielded {
                results.push(p);
            }
        }

        let out_reg = self
            .symbols
            .get("splitmap_results")
            .expect("splitmap_results not found")
            .0;
        self.insert_reg(
            branch_id,
            out_reg,
            EntropicState::Valid(Payload::Array(results)),
        )
    }

    pub(crate) fn Loop(
        &mut self,
        branch_id: &str,
        max_ms: u64,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.loop_stack.push((branch.local_clock, max_ms));
        branch.loop_depth += 1;
        Ok(())
    }

    pub(crate) fn EndLoop(
        &mut self,
        branch_id: &str,
        _max_ms: u64,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        let (start, limit) = branch.loop_stack.pop().ok_or_else(|| {
            TemporalError::EvalError("Loop stack underflow".into())
        })?;

        let total_elapsed = branch.local_clock - start;
        if limit > 0 && total_elapsed < limit {
            let pad = limit - total_elapsed;
            branch.local_clock += pad;
            branch.consume_budget(pad)?;
        }

        if branch.loop_depth > 0 {
            branch.loop_depth -= 1;
        }
        Ok(())
    }

    pub(crate) fn LoopTick(&mut self, branch_id: &str) -> Result<(), TemporalError> {
        self.commit_tick_buffers();
        let branch = self.get_branch_mut(branch_id)?;
        let slice = branch.slice_ms.ok_or(TemporalError::TickLoopWithoutSlice)?;
        branch.loop_stack.push((branch.local_clock, slice));
        branch.loop_depth += 1;
        Ok(())
    }

    pub(crate) fn EndLoopTick(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        let (start, slice) = branch.loop_stack.pop().ok_or_else(|| {
            TemporalError::EvalError("Loop stack underflow".into())
        })?;
        let elapsed = branch.local_clock - start;
        if elapsed > slice {
            return Err(TemporalError::WatchdogBite(branch_id.to_string(), slice));
        }
        let pad = slice - elapsed;
        branch.local_clock += pad;
        branch.consume_budget(pad)?;

        if branch.loop_depth > 0 {
            branch.loop_depth -= 1;
        }
        Ok(())
    }

    pub(crate) fn LoopTickOn(
        &mut self,
        branch_id: &str,
        chan_id: String,
    ) -> Result<(), TemporalError> {
        self.AwaitChan(branch_id, chan_id)?;
        self.LoopTick(branch_id)
    }

    pub(crate) fn While(
        &mut self,
        branch_id: &str,
        max_ms: u64,
    ) -> Result<(), TemporalError> {
        self.Loop(branch_id, max_ms)
    }

    pub(crate) fn EndWhile(
        &mut self,
        branch_id: &str,
        max_ms: u64,
    ) -> Result<(), TemporalError> {
        self.EndLoop(branch_id, max_ms)
    }

    pub(crate) fn ForStep(
        &mut self,
        branch_id: &str,
        item_name: String,
        source: Reg,
        step_ms: u64,
        body: Vec<Instruction>,
    ) -> Result<(), TemporalError> {
        let source_payload = self.peek_reg(branch_id, source.0)?;

        let elements = match source_payload {
            Payload::Array(vec) => vec,
            _ => {
                return Err(TemporalError::EvalError(
                    "for-step source must be array".into(),
                ))
            }
        };

        let item_reg = self.symbols.get(&item_name).unwrap().0;

        for item_value in elements.into_iter() {
            self.insert_reg(branch_id, item_reg, EntropicState::Valid(item_value))?;

            let iteration_start = self.get_branch_mut(branch_id)?.local_clock;

            let (old_pc, old_instrs) = {
                let b = self.get_branch_mut(branch_id)?;
                let pc = b.pc;
                let instrs = b.instructions.clone();
                b.instructions = body.clone();
                b.pc = 0;
                (pc, instrs)
            };

            while {
                let b = self.get_branch_mut(branch_id)?;
                b.pc < b.instructions.len()
            } {
                self.execute_instruction(branch_id)?;
            }

            {
                let b = self.get_branch_mut(branch_id)?;
                b.instructions = old_instrs;
                b.pc = old_pc;
            }

            let body_cost =
                self.get_branch_mut(branch_id)?.local_clock - iteration_start;

            if body_cost > step_ms {
                return Err(TemporalError::PacingViolation);
            }

            let pad = step_ms - body_cost;
            if pad > 0 {
                let branch = self.get_branch_mut(branch_id)?;
                branch.local_clock += pad;
                branch.consume_budget(pad)?;
            }
        }

        Ok(())
    }
}
