use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_ir::Reg;

#[allow(non_snake_case)]
impl Vm {
    pub(crate) fn Jump(
        &mut self,
        branch_id: &str,
        target: usize,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.pc = target;
        Ok(())
    }

    pub(crate) fn JumpIf(
        &mut self,
        branch_id: &str,
        cond: Reg,
        target: usize,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, cond.0)?;
        if let causm_core::value::Payload::Bool(true) = val {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc = target;
        }
        Ok(())
    }

    pub(crate) fn JumpIfNot(
        &mut self,
        branch_id: &str,
        cond: Reg,
        target: usize,
    ) -> Result<(), TemporalError> {
        let val = self.peek_reg(branch_id, cond.0)?;
        if let causm_core::value::Payload::Bool(false) = val {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc = target;
        }
        Ok(())
    }

    pub(crate) fn DynamicCall(
        &mut self,
        branch_id: &str,
        method: String,
        args: Vec<Reg>,
        dest: Reg,
        budget: Option<u64>,
    ) -> Result<(), TemporalError> {
        if args.is_empty() {
            return Err(TemporalError::EvalError(
                "DynamicCall requires at least one argument (the receiver)"
                    .to_string(),
            ));
        }
        let receiver_reg = args[0].0;
        let type_name = {
            let branch = self.get_branch_mut(branch_id)?;
            let meta = branch
                .arena
                .metadata
                .get(receiver_reg as usize)
                .and_then(|m| m.as_ref())
                .ok_or_else(|| {
                    TemporalError::EvalError("receiver has no metadata".to_string())
                })?;
            meta.type_name.clone().ok_or_else(|| {
                TemporalError::EvalError("receiver has no type name".to_string())
            })?
        };

        let routine_name = format!("{}.{}", type_name, method);
        self.execute_call(branch_id, routine_name, args, dest, budget)
    }

    pub(crate) fn Call(
        &mut self,
        branch_id: &str,
        routine: String,
        args: Vec<Reg>,
        dest: Reg,
    ) -> Result<(), TemporalError> {
        self.execute_call(branch_id, routine, args, dest, None)
    }

    fn execute_call(
        &mut self,
        branch_id: &str,
        routine: String,
        args: Vec<Reg>,
        dest: Reg,
        budget: Option<u64>,
    ) -> Result<(), TemporalError> {
        if self.is_intrinsic(&routine) {
            let mut arg_values = Vec::new();
            for reg in &args {
                arg_values.push(self.peek_reg(branch_id, reg.0)?);
            }
            let res = self.call_intrinsic(&routine, arg_values)?;
            return self.insert_reg(
                branch_id,
                dest.0,
                causm_core::value::EntropicState::Valid(res),
            );
        }

        let routine_def = self
            .routines
            .get(&routine)
            .ok_or_else(|| {
                TemporalError::EvalError(format!("unknown routine {}", routine))
            })?
            .clone();
        let params = routine_def.params.clone();

        if args.len() != params.len() {
            return Err(TemporalError::EvalError(format!(
                "routine call expects {} args, got {}",
                params.len(),
                args.len()
            )));
        }

        let mut arg_values = Vec::new();
        for reg in &args {
            let val = self.peek_reg(branch_id, reg.0)?;
            arg_values.push(val);
        }

        // Consume arguments if needed
        for (i, reg) in args.iter().enumerate() {
            let (mode, _, _) = &params[i];
            if let causm_core::ParamMode::Consume = mode {
                self.consume_reg(branch_id, reg.0)?;
            }
        }

        let child_id = format!("__routine_{}_{}", routine, self.global_clock);
        let mut child = crate::vm::state::Timeline::new(
            child_id.clone(),
            1024 * 1024,
            self.global_clock,
        );
        child.instructions = routine_def.instructions.clone();

        for (i, (mode, _name, _)) in params.iter().enumerate() {
            let val = arg_values[i].clone();
            match mode {
                causm_core::ParamMode::Consume
                | causm_core::ParamMode::Clone
                | causm_core::ParamMode::Peek => {
                    child.arena.insert(
                        i as u32,
                        causm_core::value::EntropicState::Valid(val),
                    )?;
                }
                causm_core::ParamMode::Decay => {
                    child.arena.insert(
                        i as u32,
                        causm_core::value::EntropicState::Valid(val),
                    )?;
                }
            }
        }

        self.active_branches.insert(child_id.clone(), child);

        while {
            let b = self.get_branch_mut(&child_id)?;
            b.pc < b.instructions.len()
        } {
            self.execute_instruction(&child_id)?;
        }

        let child_branch = self
            .active_branches
            .remove(&child_id)
            .ok_or_else(|| TemporalError::BranchNotFound(child_id.clone()))?;

        let elapsed = child_branch.local_clock;
        if let Some(limit) = budget {
            if elapsed > limit {
                return Err(TemporalError::EvalError(format!(
                    "temporal contract violated: concrete implementation of method '{}' took {}ms, exceeding the interface's budget of {}ms",
                    routine, elapsed, limit
                )));
            }
        }

        let result = child_branch
            .arena
            .peek(0)
            .unwrap_or(causm_core::value::Payload::String("void".to_string()));

        self.insert_reg(
            branch_id,
            dest.0,
            causm_core::value::EntropicState::Valid(result),
        )
    }

    pub(crate) fn Return(
        &mut self,
        branch_id: &str,
        src: Option<Reg>,
    ) -> Result<(), TemporalError> {
        let val = if let Some(reg) = src {
            self.peek_reg(branch_id, reg.0)
                .unwrap_or(causm_core::value::Payload::Null)
        } else {
            causm_core::value::Payload::Null
        };
        let branch = self.get_branch_mut(branch_id)?;
        branch
            .arena
            .insert(0, causm_core::value::EntropicState::Valid(val))?;
        branch.pc = branch.instructions.len();
        Ok(())
    }

    pub(crate) fn Select(
        &mut self,
        branch_id: &str,
        max_ms: u64,
        cases: Vec<causm_ir::IrSelectCase>,
        timeout_target: Option<usize>,
    ) -> Result<(), TemporalError> {
        let mut found_case = None;
        for case in &cases {
            let message = {
                if let Some(chan) = self.channels.get_mut(&case.chan_id) {
                    chan.pop_front()
                } else {
                    None
                }
            };

            if let Some(msg) = message {
                found_case = Some((case.clone(), msg));
                break;
            }
        }

        // Apply deterministic temporal padding
        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.local_clock += max_ms;
            branch.consume_budget(max_ms)?;
        }

        if let Some((case, msg)) = found_case {
            self.insert_reg(
                branch_id,
                case.dest.0,
                causm_core::value::EntropicState::Valid(msg.payload),
            )?;
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc = case.target;
        } else if let Some(target) = timeout_target {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc = target;
        }
        Ok(())
    }

    pub(crate) fn MatchEntropy(
        &mut self,
        branch_id: &str,
        target: Reg,
        valid_target: Option<usize>,
        decayed_target: Option<usize>,
        pending_target: Option<usize>,
        consumed_target: Option<usize>,
    ) -> Result<(), TemporalError> {
        let state = self.peek_state(branch_id, target.0)?;
        let maybe_jump = match &state {
            causm_core::value::EntropicState::Valid(_) => valid_target,
            causm_core::value::EntropicState::Leased { original, .. } => {
                match &**original {
                    causm_core::value::EntropicState::Valid(_) => valid_target,
                    causm_core::value::EntropicState::Decayed(_) => decayed_target,
                    causm_core::value::EntropicState::Pending(_) => pending_target,
                    causm_core::value::EntropicState::Consumed => consumed_target,
                    causm_core::value::EntropicState::Leased { .. } => valid_target, // Should not happen due to analysis
                }
            }
            causm_core::value::EntropicState::Decayed(_) => decayed_target,
            causm_core::value::EntropicState::Pending(_) => pending_target,
            causm_core::value::EntropicState::Consumed => consumed_target,
        };

        if let Some(target_pc) = maybe_jump {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc = target_pc;
        }
        Ok(())
    }
}
