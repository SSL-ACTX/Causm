use crate::vm::error::TemporalError;
use crate::vm::state::{FlatLoopState, Timeline, Vm};
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
        dest_cond: Reg,
        item_reg: Reg,
        item_name: String,
        mode: ParamMode,
        source: Reg,
        pacing_ms: Option<u64>,
        max_ms: Option<u64>,
    ) -> Result<(), TemporalError> {
        let (header_pc, loop_exists) = {
            let branch = self.get_branch(branch_id)?;
            let pc = branch.pc;
            let header_pc = pc - 1;
            let loop_exists = branch
                .flat_loops
                .last()
                .map(|l| l.header_pc == header_pc)
                .unwrap_or(false);
            (header_pc, loop_exists)
        };

        if !loop_exists {
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
            self.get_branch_mut(branch_id)?
                .flat_loops
                .push(FlatLoopState {
                    header_pc,
                    end_pc: 0,
                    item_name: item_name.clone(),
                    elements,
                    index: 0,
                    pacing_ms,
                    max_ms,
                    start_local_clock,
                    iteration_start_clock: start_local_clock,
                });
        }

        let (index, elements_len, max_allowed, start_local_clock) = {
            let branch = self.get_branch(branch_id)?;
            let loop_state = branch.flat_loops.last().unwrap();
            (
                loop_state.index,
                loop_state.elements.len(),
                loop_state.max_ms.unwrap_or(u64::MAX),
                loop_state.start_local_clock,
            )
        };

        let local_clock = self.get_branch(branch_id)?.local_clock;

        if index < elements_len && (local_clock - start_local_clock) < max_allowed {
            let item_value = {
                let branch = self.get_branch(branch_id)?;
                branch.flat_loops.last().unwrap().elements[index].clone()
            };
            self.insert_reg(
                branch_id,
                item_reg.0,
                EntropicState::Valid(item_value),
            )?;

            let local_clock = self.get_branch(branch_id)?.local_clock;
            let branch = self.get_branch_mut(branch_id)?;
            branch.flat_loops.last_mut().unwrap().iteration_start_clock =
                local_clock;

            self.insert_reg(
                branch_id,
                dest_cond.0,
                EntropicState::Valid(Payload::Bool(true)),
            )?;
        } else {
            let branch = self.get_branch_mut(branch_id)?;
            branch.flat_loops.pop();

            if let Some(max) = max_ms {
                let total_elapsed = branch.local_clock - start_local_clock;
                if total_elapsed < max {
                    let pad = max - total_elapsed;
                    branch.local_clock += pad;
                    branch.consume_budget(pad)?;
                }
            }

            self.insert_reg(
                branch_id,
                dest_cond.0,
                EntropicState::Valid(Payload::Bool(false)),
            )?;
        }

        Ok(())
    }

    pub(crate) fn EndFor(&mut self, branch_id: &str) -> Result<(), TemporalError> {
        let (paced, body_cost, header_pc) = {
            let branch = self.get_branch(branch_id)?;
            let loop_state = branch.flat_loops.last().ok_or_else(|| {
                TemporalError::EvalError("Loop state underflow on EndFor".into())
            })?;
            let body_cost = branch.local_clock - loop_state.iteration_start_clock;
            let paced = loop_state.pacing_ms.unwrap_or(body_cost);
            (paced, body_cost, loop_state.header_pc)
        };

        if body_cost > paced {
            return Err(TemporalError::PacingViolation);
        }

        let pad = paced - body_cost;
        let branch = self.get_branch_mut(branch_id)?;
        if pad > 0 {
            branch.local_clock += pad;
            branch.consume_budget(pad)?;
        }

        branch.flat_loops.last_mut().unwrap().index += 1;
        branch.pc = header_pc;
        Ok(())
    }

    pub(crate) fn SplitMap(
        &mut self,
        branch_id: &str,
        _item_reg: Reg,
        item_name: String,
        mode: ParamMode,
        source: Reg,
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

        // Find matching EndSplitMap PC
        let (pc, end_pc) = {
            let branch = self.get_branch(branch_id)?;
            let pc = branch.pc;
            let mut depth = 0;
            let mut end_pc = None;
            for i in pc..branch.instructions.len() {
                match &branch.instructions[i] {
                    Instruction::SplitMap { .. } => depth += 1,
                    Instruction::EndSplitMap => {
                        if depth == 0 {
                            end_pc = Some(i);
                            break;
                        } else {
                            depth -= 1;
                        }
                    }
                    _ => {}
                }
            }
            let end_pc = end_pc
                .ok_or(TemporalError::EvalError("Missing EndSplitMap".into()))?;
            (pc, end_pc)
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
                child_branch.pc = pc;
            }

            while {
                let b = self.get_branch_mut(&child_name)?;
                b.pc < end_pc
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

        let branch = self.get_branch_mut(branch_id)?;
        branch.pc = end_pc + 1;

        self.insert_reg(
            branch_id,
            out_reg,
            EntropicState::Valid(Payload::Array(results)),
        )
    }

    pub(crate) fn EndSplitMap(
        &mut self,
        _branch_id: &str,
    ) -> Result<(), TemporalError> {
        Ok(())
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
        dest_cond: Reg,
        item_reg: Reg,
        item_name: String,
        source: Reg,
        step_ms: u64,
    ) -> Result<(), TemporalError> {
        let (header_pc, loop_exists) = {
            let branch = self.get_branch(branch_id)?;
            let pc = branch.pc;
            let header_pc = pc - 1;
            let loop_exists = branch
                .flat_loops
                .last()
                .map(|l| l.header_pc == header_pc)
                .unwrap_or(false);
            (header_pc, loop_exists)
        };

        if !loop_exists {
            let source_payload = self.peek_reg(branch_id, source.0)?;
            let elements = match source_payload {
                Payload::Array(vec) => vec,
                _ => {
                    return Err(TemporalError::EvalError(
                        "for-step source must be array".into(),
                    ))
                }
            };

            let start_local_clock = self.get_branch(branch_id)?.local_clock;
            self.get_branch_mut(branch_id)?
                .flat_loops
                .push(FlatLoopState {
                    header_pc,
                    end_pc: 0,
                    item_name: item_name.clone(),
                    elements,
                    index: 0,
                    pacing_ms: Some(step_ms),
                    max_ms: None,
                    start_local_clock,
                    iteration_start_clock: start_local_clock,
                });
        }

        let (index, elements_len) = {
            let branch = self.get_branch(branch_id)?;
            let loop_state = branch.flat_loops.last().unwrap();
            (loop_state.index, loop_state.elements.len())
        };

        if index < elements_len {
            let item_value = {
                let branch = self.get_branch(branch_id)?;
                branch.flat_loops.last().unwrap().elements[index].clone()
            };
            self.insert_reg(
                branch_id,
                item_reg.0,
                EntropicState::Valid(item_value),
            )?;

            let local_clock = self.get_branch(branch_id)?.local_clock;
            let branch = self.get_branch_mut(branch_id)?;
            branch.flat_loops.last_mut().unwrap().iteration_start_clock =
                local_clock;

            self.insert_reg(
                branch_id,
                dest_cond.0,
                EntropicState::Valid(Payload::Bool(true)),
            )?;
        } else {
            let branch = self.get_branch_mut(branch_id)?;
            branch.flat_loops.pop();

            self.insert_reg(
                branch_id,
                dest_cond.0,
                EntropicState::Valid(Payload::Bool(false)),
            )?;
        }

        Ok(())
    }

    pub(crate) fn EndForStep(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        let (step_ms, body_cost, header_pc) = {
            let branch = self.get_branch(branch_id)?;
            let loop_state = branch.flat_loops.last().ok_or_else(|| {
                TemporalError::EvalError("Loop state underflow on EndForStep".into())
            })?;
            let body_cost = branch.local_clock - loop_state.iteration_start_clock;
            let step_ms = loop_state.pacing_ms.unwrap();
            (step_ms, body_cost, loop_state.header_pc)
        };

        if body_cost > step_ms {
            return Err(TemporalError::PacingViolation);
        }

        let pad = step_ms - body_cost;
        let branch = self.get_branch_mut(branch_id)?;
        if pad > 0 {
            branch.local_clock += pad;
            branch.consume_budget(pad)?;
        }

        branch.flat_loops.last_mut().unwrap().index += 1;
        branch.pc = header_pc;
        Ok(())
    }
}
