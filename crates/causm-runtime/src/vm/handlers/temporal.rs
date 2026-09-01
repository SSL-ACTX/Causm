use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::{Manifest, MergeResolution};
use causm_ir::Reg;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn SetEntropyMode(
        &mut self,
        branch_id: &str,
        mode: causm_core::EntropyMode,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.entropy_mode = mode;
        Ok(())
    }

    pub(crate) fn Isolate(
        &mut self,
        branch_id: &str,
        _name: String,
        manifest: Manifest,
    ) -> Result<(), TemporalError> {
        let (cpu_req, slice_req) = {
            let branch = self.get_branch_mut(branch_id)?;
            if let Some(limit_bytes) = manifest.memory_budget_bytes {
                branch.arena.capacity = limit_bytes;
            }
            if let Some(mode) = manifest.mode {
                branch.entropy_mode = mode;
            }
            // Apply resource budgets
            for (res, amount) in &manifest.resource_budgets {
                branch.resource_budgets.insert(res.clone(), *amount);
            }
            branch.manifest_stack.push(manifest.clone());
            (manifest.cpu_budget_ms, manifest.slice_ms)
        };

        if let Some(cpu) = cpu_req {
            let branch = self.get_branch_mut(branch_id)?;
            if cpu > branch.cpu_budget_ms {
                return Err(TemporalError::BudgetExhausted);
            }
            branch.cpu_budget_ms = cpu;
        }
        if let Some(slice) = slice_req {
            let branch = self.get_branch_mut(branch_id)?;
            branch.slice_ms = Some(slice);
        }
        Ok(())
    }

    pub(crate) fn EndIsolate(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.manifest_stack.pop();
        Ok(())
    }

    pub(crate) fn Split(
        &mut self,
        branch_id: &str,
        _parent: String,
        branches: Vec<String>,
    ) -> Result<(), TemporalError> {
        let branch_names: Vec<&str> = branches.iter().map(|s| s.as_str()).collect();
        self.split_timeline(branch_id, branch_names)
    }

    pub(crate) fn Merge(
        &mut self,
        _branch_id: &str,
        branches: Vec<String>,
        target: String,
        resolution: MergeResolution,
    ) -> Result<(), TemporalError> {
        let branch_names: Vec<&str> = branches.iter().map(|s| s.as_str()).collect();
        self.merge_timelines(branch_names, &target, &resolution)
    }

    pub(crate) fn Slice(
        &mut self,
        branch_id: &str,
        ms: u64,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.slice_ms = Some(ms);
        Ok(())
    }

    pub(crate) fn Break(&mut self, branch_id: &str) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.break_requested = true;
        Ok(())
    }

    pub(crate) fn AssertTime(
        &mut self,
        branch_id: &str,
        op: causm_core::BinaryOperator,
        limit_ms: u64,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        let elapsed = branch.local_clock;
        let condition = match op {
            causm_core::BinaryOperator::Lt => elapsed < limit_ms,
            causm_core::BinaryOperator::Gt => elapsed > limit_ms,
            causm_core::BinaryOperator::Le => elapsed <= limit_ms,
            causm_core::BinaryOperator::Ge => elapsed >= limit_ms,
            causm_core::BinaryOperator::Eq => elapsed == limit_ms,
            causm_core::BinaryOperator::Neq => elapsed != limit_ms,
            _ => false,
        };
        if !condition {
            return Err(TemporalError::AssertionFailed(format!(
                "Temporal assertion failed: elapsed {}ms {:?} {}ms",
                elapsed, op, limit_ms
            )));
        }
        Ok(())
    }

    pub(crate) fn Watchdog(
        &mut self,
        branch_id: &str,
        target: String,
        timeout_ms: u64,
        recovery_jump: Option<usize>,
    ) -> Result<(), TemporalError> {
        let (target_clock, is_active) = {
            if let Ok(t) = self.get_branch_mut(&target) {
                (t.local_clock, true)
            } else {
                (0, false)
            }
        };

        if is_active && target_clock > timeout_ms {
            if let Some(jump) = recovery_jump {
                let branch = self.get_branch_mut(branch_id)?;
                branch.pc = jump;
            }
        }
        Ok(())
    }

    pub(crate) fn RelativisticBlock(
        &mut self,
        branch_id: &str,
        target: String,
        block_pc: usize,
        block_len: usize,
    ) -> Result<(), TemporalError> {
        // println!(
        //     "[VM] RelativisticBlock: target={}, pc={}, len={}",
        //     target, block_pc, block_len
        // );
        let (target_id, old_pc, old_instrs) = {
            let t = self.get_branch_mut(&target)?;
            let old_pc = t.pc;
            let old_instrs = t.instructions.clone();
            (target.clone(), old_pc, old_instrs)
        };

        {
            let current_instrs =
                self.get_branch_mut(branch_id)?.instructions.clone();
            let t = self.get_branch_mut(&target_id)?;
            t.instructions = current_instrs;
            t.pc = block_pc;
        }

        // DEV NOTE (Relativistic Execution & Temporal Control Flow):
        // When executing a RelativisticBlock on a branch, called routines (e.g. SyncChannel.send,
        // Mutex.try_lock) switch `t.pc` to 0 and push a frame onto `t.call_stack`. We must guard
        // against premature block loop exit (`!in_call`) so nested routine execution completes and
        // returns to the block before checking whether the block's outer PC range has elapsed.
        loop {
            let (pc, in_call, instrs_len) = {
                let t = self.get_branch_mut(&target_id)?;
                (t.pc, !t.call_stack.is_empty(), t.instructions.len())
            };
            if !in_call
                && (pc < block_pc || pc >= block_pc + block_len || pc >= instrs_len)
            {
                break;
            }
            self.execute_instruction(&target_id)?;

            let b = self.get_branch_mut(&target_id)?;
            if b.break_requested {
                let target_depth = b.loop_depth;
                b.break_requested = false;

                while {
                    let b = self.get_branch_mut(&target_id)?;
                    b.pc < b.instructions.len() && b.pc < block_pc + block_len
                } {
                    let instr = {
                        let b = self.get_branch_mut(&target_id)?;
                        b.instructions[b.pc].clone()
                    };
                    match instr {
                        causm_ir::Instruction::Loop { .. }
                        | causm_ir::Instruction::LoopTick => {
                            let b = self.get_branch_mut(&target_id)?;
                            b.loop_depth += 1;
                        }
                        causm_ir::Instruction::EndLoop { max_ms } => {
                            let b = self.get_branch_mut(&target_id)?;
                            b.loop_depth -= 1;
                            if b.loop_depth < target_depth {
                                let max_ms_val = max_ms;
                                self.EndLoop(&target_id, max_ms_val)?;
                                let b = self.get_branch_mut(&target_id)?;
                                b.pc += 1;
                                // Skip the following Jump if present
                                if b.pc < b.instructions.len() {
                                    if let causm_ir::Instruction::Jump { .. } =
                                        b.instructions[b.pc]
                                    {
                                        b.pc += 1;
                                    }
                                }
                                break;
                            }
                        }
                        causm_ir::Instruction::EndLoopTick => {
                            let b = self.get_branch_mut(&target_id)?;
                            b.loop_depth -= 1;
                            if b.loop_depth < target_depth {
                                self.EndLoopTick(&target_id)?;
                                let b = self.get_branch_mut(&target_id)?;
                                b.pc += 1;
                                // Skip the following Jump if present
                                if b.pc < b.instructions.len() {
                                    if let causm_ir::Instruction::Jump { .. } =
                                        b.instructions[b.pc]
                                    {
                                        b.pc += 1;
                                    }
                                }
                                break;
                            }
                        }
                        _ => {}
                    }
                    let b = self.get_branch_mut(&target_id)?;
                    b.pc += 1;
                }
            }
        }

        // DEV NOTE (Cross-Timeline Scope & Arena Sync):
        // Relativistic blocks execute inside the target branch's timeline, modifying registers
        // in `target.arena`. To allow consecutive blocks on the parent timeline or subsequent
        // merges to access newly materialized values (e.g., intermediate structs or channel payloads),
        // we synchronize active (non-Consumed) registers back into the calling timeline arena.
        {
            let target_registers = {
                let t = self.get_branch_mut(&target_id)?;
                t.instructions = old_instrs;
                if branch_id != target_id {
                    t.pc = old_pc;
                }
                t.arena.registers.clone()
            };
            let caller = self.get_branch_mut(branch_id)?;
            if caller.arena.registers.len() < target_registers.len() {
                caller.arena.registers.resize(
                    target_registers.len(),
                    causm_core::value::EntropicState::Consumed,
                );
            }
            for (idx, state) in target_registers.iter().enumerate() {
                if !matches!(state, causm_core::value::EntropicState::Consumed) {
                    caller.arena.registers[idx] = state.clone();
                }
            }
        }
        // println!("[VM] RelativisticBlock finished: {}", target_id);
        Ok(())
    }

    pub(crate) fn Defer(
        &mut self,
        branch_id: &str,
        dest: Reg,
        cap: causm_core::Capability,
        deadline_ms: u64,
    ) -> Result<(), TemporalError> {
        let requested_at = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.local_clock
        };
        let latency = cap
            .parameters
            .get("latency")
            .and_then(|l| {
                l.trim_end_matches("ms")
                    .trim_end_matches("us")
                    .trim_end_matches("ns")
                    .trim_end_matches('s')
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or(10);

        let promise = causm_core::value::PendingPromise {
            capability: cap.path.clone(),
            params: cap.parameters.clone(),
            requested_at,
            ready_at: requested_at + latency,
            deadline_at: requested_at + deadline_ms,
        };
        self.insert_reg(
            branch_id,
            dest.0,
            causm_core::value::EntropicState::Pending(promise),
        )
    }

    pub(crate) fn Await(
        &mut self,
        branch_id: &str,
        target: Reg,
    ) -> Result<(), TemporalError> {
        let promise = {
            let state = self.peek_state(branch_id, target.0)?;
            match state {
                causm_core::value::EntropicState::Pending(p) => p,
                _ => {
                    return Err(TemporalError::EvalError(
                        "await target must be a pending promise".into(),
                    ))
                }
            }
        };

        {
            let branch = self.get_branch_mut(branch_id)?;
            if branch.local_clock < promise.ready_at {
                let wait = promise.ready_at - branch.local_clock;
                branch.local_clock = promise.ready_at;
                branch.consume_budget(wait)?;
            }

            if branch.local_clock > promise.deadline_at {
                // Timeout!
                branch
                    .arena
                    .insert(target.0, causm_core::value::EntropicState::Consumed)?;
                return Ok(());
            }
        }

        // Execute the capability now that it's ready
        let cap = causm_core::Capability {
            path: promise.capability,
            parameters: promise.params,
        };
        match self._execute_capability(branch_id, &cap) {
            Ok(payload) => self.insert_reg(
                branch_id,
                target.0,
                causm_core::value::EntropicState::Valid(payload),
            ),
            Err(_) => self.insert_reg(
                branch_id,
                target.0,
                causm_core::value::EntropicState::Consumed,
            ),
        }
    }

    pub(crate) fn Lease(
        &mut self,
        branch_id: &str,
        target_reg: Reg,
        source_reg: Reg,
        duration_ms: u64,
    ) -> Result<(), TemporalError> {
        let source_state = self.peek_state(branch_id, source_reg.0)?;

        if matches!(
            source_state,
            causm_core::value::EntropicState::Leased { .. }
        ) {
            return Err(TemporalError::LeaseViolation(format!(
                "register R{} is already leased",
                source_reg.0
            )));
        }

        if matches!(source_state, causm_core::value::EntropicState::Consumed) {
            return Err(TemporalError::MemoryFault(
                causm_core::value::MemoryError::AlreadyConsumed,
            ));
        }

        let expiration_ms = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.local_clock + duration_ms
        };

        let metadata = {
            let branch = self.get_branch_mut(branch_id)?;
            branch
                .arena
                .metadata
                .get(source_reg.0 as usize)
                .and_then(|m| m.clone())
        };

        // Clone the original state to the target register (the lease view)
        let lease_view = source_state.clone();
        if let Some(ref meta) = metadata {
            let branch = self.get_branch_mut(branch_id)?;
            branch.arena.insert_with_metadata(
                target_reg.0,
                lease_view,
                meta.clone(),
            )?;
        } else {
            self.insert_reg(branch_id, target_reg.0, lease_view)?;
        }

        // Transition source to Leased state while preserving its metadata
        let original = Box::new(source_state);
        if let Some(meta) = metadata {
            let branch = self.get_branch_mut(branch_id)?;
            branch.arena.insert_with_metadata(
                source_reg.0,
                causm_core::value::EntropicState::Leased {
                    original,
                    expiration_ms,
                },
                meta,
            )?;
        } else {
            self.insert_reg(
                branch_id,
                source_reg.0,
                causm_core::value::EntropicState::Leased {
                    original,
                    expiration_ms,
                },
            )?;
        }

        Ok(())
    }

    pub(crate) fn EndLease(
        &mut self,
        branch_id: &str,
        source_reg: Reg,
        _duration_ms: u64,
    ) -> Result<(), TemporalError> {
        let current_clock = {
            let branch = self.get_branch_mut(branch_id)?;
            branch.local_clock
        };

        let metadata = {
            let branch = self.get_branch_mut(branch_id)?;
            branch
                .arena
                .metadata
                .get(source_reg.0 as usize)
                .and_then(|m| m.clone())
        };

        let (original, expiration_ms) = {
            let state = self.peek_state(branch_id, source_reg.0)?;
            match state {
                causm_core::value::EntropicState::Leased {
                    original,
                    expiration_ms,
                } => (*original, expiration_ms),
                _ => {
                    return Err(TemporalError::LeaseViolation(format!(
                        "register R{} is not leased",
                        source_reg.0
                    )))
                }
            }
        };

        if current_clock > expiration_ms {
            return Err(TemporalError::LeaseViolation(format!(
                "Lease on R{} exceeded duration by {}ms",
                source_reg.0,
                current_clock - expiration_ms
            )));
        }

        // Apply padding
        if current_clock < expiration_ms {
            let padding = expiration_ms - current_clock;
            let branch = self.get_branch_mut(branch_id)?;
            branch.local_clock = expiration_ms;
            branch.consume_budget(padding)?;
        }

        // Restore original state and preserve metadata
        if let Some(meta) = metadata {
            let branch = self.get_branch_mut(branch_id)?;
            branch
                .arena
                .insert_with_metadata(source_reg.0, original, meta)?;
        } else {
            self.insert_reg(branch_id, source_reg.0, original)?;
        }

        Ok(())
    }

    pub(crate) fn PeriodicEpoch(
        &mut self,
        branch_id: &str,
        _interval_ms: u64,
        _block_pc: usize,
        _block_len: usize,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.arena.freeze_base_watermark();
        Ok(())
    }

    pub(crate) fn EndPeriodicEpoch(
        &mut self,
        branch_id: &str,
        interval_ms: u64,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.arena.reset_to_base_watermark();
        if branch.local_clock < interval_ms {
            let padding = interval_ms - branch.local_clock;
            branch.local_clock = interval_ms;
            branch.consume_budget(padding)?;
        }
        Ok(())
    }

    pub(crate) fn FreezeBaseWatermark(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.arena.freeze_base_watermark();
        Ok(())
    }

    pub(crate) fn ResetBaseWatermark(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.arena.reset_to_base_watermark();
        Ok(())
    }

    pub(crate) fn SetSaturationPolicy(
        &mut self,
        branch_id: &str,
        target: causm_core::PolicyTarget,
        policy: causm_core::SaturationPolicy,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.saturation_policies.insert(target, policy);
        Ok(())
    }

    pub(crate) fn ArenaIntrospect(
        &mut self,
        branch_id: &str,
        dest: Reg,
        kind: causm_core::ArenaIntrospect,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch(branch_id)?;
        let val = match kind {
            causm_core::ArenaIntrospect::Remaining => {
                branch.arena.remaining() as i64
            }
            causm_core::ArenaIntrospect::UsedBytes => {
                branch.arena.used_bytes() as i64
            }
            causm_core::ArenaIntrospect::Capacity => branch.arena.capacity as i64,
        };
        self.insert_reg(
            branch_id,
            dest.0,
            causm_core::value::EntropicState::Valid(
                causm_core::value::Payload::Integer(val),
            ),
        )?;
        Ok(())
    }
}
