use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_core::{Manifest, MergeResolution};
use causm_frontend::ir::Reg;

#[allow(non_snake_case, dead_code)]
impl Vm {
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

    pub(crate) fn Break(
        &mut self,
        branch_id: &str,
        _target: usize,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.break_requested = true;
        Ok(())
    }

    pub(crate) fn Reset(
        &mut self,
        branch_id: &str,
        target: String,
        anchor_name: String,
    ) -> Result<(), TemporalError> {
        self.Rewind(branch_id, target, anchor_name)
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
        println!(
            "[VM] RelativisticBlock: target={}, pc={}, len={}",
            target, block_pc, block_len
        );
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

        for i in 0..block_len {
            let (pc, instrs_len) = {
                let t = self.get_branch_mut(&target_id)?;
                (t.pc, t.instructions.len())
            };
            if pc < block_pc || pc >= block_pc + block_len || pc >= instrs_len {
                println!(
                    "[VM] RelativisticBlock: PC {} out of bounds [{}, {}), stopping.",
                    pc,
                    block_pc,
                    block_pc + block_len
                );
                break;
            }
            println!(
                "[VM] RelativisticBlock execution: step {}/{} on {} at PC {}",
                i + 1,
                block_len,
                target_id,
                pc
            );
            self.execute_instruction(&target_id)?;

            // Handle break within the relativistic block
            let b = self.get_branch_mut(&target_id)?;
            if b.break_requested {
                let target_depth = b.loop_depth;
                b.break_requested = false;
                let _ = b;

                while {
                    let b = self.get_branch_mut(&target_id)?;
                    b.pc < b.instructions.len() && b.pc < block_pc + block_len
                } {
                    let instr = {
                        let b = self.get_branch_mut(&target_id)?;
                        b.instructions[b.pc].clone()
                    };
                    match instr {
                        causm_frontend::ir::Instruction::Loop { .. }
                        | causm_frontend::ir::Instruction::LoopTick => {
                            let b = self.get_branch_mut(&target_id)?;
                            b.loop_depth += 1;
                        }
                        causm_frontend::ir::Instruction::EndLoop { max_ms } => {
                            let b = self.get_branch_mut(&target_id)?;
                            b.loop_depth -= 1;
                            if b.loop_depth < target_depth {
                                let max_ms_val = max_ms;
                                self.EndLoop(&target_id, max_ms_val)?;
                                let b = self.get_branch_mut(&target_id)?;
                                b.pc += 1;
                                // Skip the following Jump if present
                                if b.pc < b.instructions.len() {
                                    if let causm_frontend::ir::Instruction::Jump {
                                        ..
                                    } = b.instructions[b.pc]
                                    {
                                        b.pc += 1;
                                    }
                                }
                                break;
                            }
                        }
                        causm_frontend::ir::Instruction::EndLoopTick => {
                            let b = self.get_branch_mut(&target_id)?;
                            b.loop_depth -= 1;
                            if b.loop_depth < target_depth {
                                self.EndLoopTick(&target_id)?;
                                let b = self.get_branch_mut(&target_id)?;
                                b.pc += 1;
                                // Skip the following Jump if present
                                if b.pc < b.instructions.len() {
                                    if let causm_frontend::ir::Instruction::Jump {
                                        ..
                                    } = b.instructions[b.pc]
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

        let t = self.get_branch_mut(&target_id)?;
        t.instructions = old_instrs;
        t.pc = old_pc;
        println!("[VM] RelativisticBlock finished: {}", target_id);
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
            .and_then(|l| l.parse::<u64>().ok())
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
        self._execute_capability(branch_id, &cap)?;

        // For now, let's say it returns Null
        self.insert_reg(
            branch_id,
            target.0,
            causm_core::value::EntropicState::Valid(
                causm_core::value::Payload::Null,
            ),
        )
    }

    pub(crate) fn YieldPad(
        &mut self,
        _branch_id: &str,
    ) -> Result<(), TemporalError> {
        // In the interpreter, yield_pad is a no-op as we don't have cycle-accurate timing here.
        // It's primarily a hint for the JIT.
        Ok(())
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

        // Clone the original state to the target register (the lease view)
        let lease_view = source_state.clone();
        self.insert_reg(branch_id, target_reg.0, lease_view)?;

        // Transition source to Leased state
        let original = Box::new(source_state);
        self.insert_reg(
            branch_id,
            source_reg.0,
            causm_core::value::EntropicState::Leased {
                original,
                expiration_ms,
            },
        )?;

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

        // Restore original state
        self.insert_reg(branch_id, source_reg.0, original)?;

        Ok(())
    }
}
