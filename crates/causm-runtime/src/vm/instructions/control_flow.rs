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
        if self.call_depth >= self.max_call_depth {
            return Err(TemporalError::EvalError(format!(
                "Call stack overflow: maximum recursion depth {} exceeded in routine '{}'",
                self.max_call_depth, routine
            )));
        }
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
        if self.call_depth >= self.max_call_depth {
            return Err(TemporalError::EvalError(format!(
                "Call stack overflow: maximum recursion depth {} exceeded in routine '{}'",
                self.max_call_depth, routine
            )));
        }
        self.call_depth += 1;
        let res = self.execute_call_inner(branch_id, routine, args, dest, budget);
        self.call_depth -= 1;
        res
    }

    fn execute_call_inner(
        &mut self,
        branch_id: &str,
        routine: String,
        args: Vec<Reg>,
        dest: Reg,
        budget: Option<u64>,
    ) -> Result<(), TemporalError> {
        let mut resolved_routine = routine.clone();
        if !self.routines.contains_key(&resolved_routine)
            && !self.is_intrinsic(&resolved_routine)
        {
            if let Some(angle_idx) = routine.find('<') {
                if let Some(dot_idx) = routine.find('.') {
                    let base_struct = &routine[..angle_idx];
                    let method_name = &routine[dot_idx..];
                    let base_routine = format!("{}{}", base_struct, method_name);
                    if self.routines.contains_key(&base_routine) {
                        resolved_routine = base_routine;
                    }
                }
            }

            if !self.routines.contains_key(&resolved_routine) {
                if let Some(dot_idx) = routine.find('.') {
                    let struct_name = &routine[..dot_idx];
                    let method_name = &routine[dot_idx + 1..];
                    let mut current = struct_name.to_string();
                    while let Some(parent) = self.struct_extends.get(&current) {
                        let parent_routine = format!("{}.{}", parent, method_name);
                        if self.routines.contains_key(&parent_routine) {
                            resolved_routine = parent_routine;
                            break;
                        }
                        current = parent.clone();
                    }
                }
            }
        }
        let routine = resolved_routine;

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
        let mut arg_metas = Vec::new();
        {
            let branch = self.get_branch_mut(branch_id)?;
            for reg in &args {
                let val =
                    branch.arena.peek(reg.0).ok_or(TemporalError::MemoryFault(
                        causm_core::value::MemoryError::AlreadyConsumed,
                    ))?;
                let meta = branch
                    .arena
                    .metadata
                    .get(reg.0 as usize)
                    .and_then(|m| m.clone());
                arg_values.push(val);
                arg_metas.push(meta);
            }
        }

        // Check if this routine is a dynamic foreign FFI binding
        if let Some(ref binding) = routine_def.foreign_binding {
            let sym_ptr = self
                .foreign_manager
                .get_or_load_symbol(&binding.lib_name, &binding.symbol)?;
            let result_payload = unsafe {
                crate::vm::ffi::invoke_foreign_symbol(
                    sym_ptr,
                    &mut arg_values,
                    &routine_def.return_type,
                )?
            };

            // Write back any modified struct or array argument payloads into registers
            for (i, reg) in args.iter().enumerate() {
                if matches!(
                    &arg_values[i],
                    causm_core::value::Payload::Struct(_)
                        | causm_core::value::Payload::Array(_)
                ) {
                    self.insert_reg(
                        branch_id,
                        reg.0,
                        causm_core::value::EntropicState::Valid(
                            arg_values[i].clone(),
                        ),
                    )?;
                }
            }

            // Consume arguments if needed
            for (i, reg) in args.iter().enumerate() {
                let (mode, _, _) = &params[i];
                if let causm_core::ParamMode::Consume = mode {
                    self.consume_reg(branch_id, reg.0)?;
                }
            }

            if let Some(cost) = routine_def.taking_ms {
                let branch = self.get_branch_mut(branch_id)?;
                branch.local_clock += cost;
                branch.consume_budget(cost)?;
            }

            return self.insert_reg(
                branch_id,
                dest.0,
                causm_core::value::EntropicState::Valid(result_payload),
            );
        }

        // Consume arguments if needed
        for (i, reg) in args.iter().enumerate() {
            let (mode, _, _) = &params[i];
            if let causm_core::ParamMode::Consume = mode {
                self.consume_reg(branch_id, reg.0)?;
            }
        }

        let caller_call_depth = self.get_branch(branch_id)?.call_depth;

        let caller_manifest_stack = self
            .get_branch(branch_id)
            .map(|b| b.manifest_stack.clone())
            .unwrap_or_default();
        let call_idx = self.next_call_id;
        self.next_call_id += 1;
        let child_id =
            format!("__routine_{}_{}_{}", routine, self.global_clock, call_idx);
        let mut child = crate::vm::state::Timeline::new(
            child_id.clone(),
            1024 * 1024,
            self.global_clock,
        );
        child.instructions = routine_def.instructions.clone();
        child.manifest_stack = caller_manifest_stack;
        child.call_depth = caller_call_depth + 1;

        for (i, (mode, _name, _)) in params.iter().enumerate() {
            let val = arg_values[i].clone();
            let meta = arg_metas.get(i).and_then(|m| m.clone());
            match mode {
                causm_core::ParamMode::Consume
                | causm_core::ParamMode::Clone
                | causm_core::ParamMode::Peek
                | causm_core::ParamMode::Lease => {
                    if let Some(m) = meta {
                        child.arena.insert_with_metadata(
                            i as u32,
                            causm_core::value::EntropicState::Valid(val),
                            m,
                        )?;
                    } else {
                        child.arena.insert(
                            i as u32,
                            causm_core::value::EntropicState::Valid(val),
                        )?;
                    }
                }
                causm_core::ParamMode::Decay => {
                    if let Some(m) = meta {
                        child.arena.insert_with_metadata(
                            i as u32,
                            causm_core::value::EntropicState::Valid(val),
                            m,
                        )?;
                    } else {
                        child.arena.insert(
                            i as u32,
                            causm_core::value::EntropicState::Valid(val),
                        )?;
                    }
                }
            }
        }

        self.active_branches.insert(child_id.clone(), child);

        let exec_res = (|| -> Result<(), TemporalError> {
            while {
                let b = self.get_branch_mut(&child_id)?;
                b.pc < b.instructions.len()
            } {
                {
                    let b = self.get_branch_mut(&child_id)?;
                    b.total_executed_cycles += 1;
                    if b.total_executed_cycles > b.max_cycles_watchdog {
                        return Err(TemporalError::WatchdogBite(
                            child_id.clone(),
                            b.max_cycles_watchdog,
                        ));
                    }
                }
                self.execute_instruction(&child_id)?;
                self.handle_break(&child_id)?;
            }
            Ok(())
        })();

        let child_branch = self.active_branches.remove(&child_id);
        exec_res?;
        let child_branch = child_branch
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

        // Write back any modified lease or FFI pointer buffer argument payloads from child arena to caller registers
        for (i, (mode, _, expected_type)) in params.iter().enumerate() {
            let is_ffi_buf = matches!(mode, causm_core::ParamMode::Peek)
                && matches!(
                    expected_type,
                    causm_core::types::Type::I64
                        | causm_core::types::Type::I32
                        | causm_core::types::Type::U64
                        | causm_core::types::Type::Integer
                );
            if matches!(mode, causm_core::ParamMode::Lease) || is_ffi_buf {
                if let Some(child_val) = child_branch.arena.peek(i as u32) {
                    if matches!(
                        child_val,
                        causm_core::value::Payload::Array(_)
                            | causm_core::value::Payload::Struct(_)
                    ) {
                        let caller_val = self.peek_reg(branch_id, args[i].0);
                        if let Ok(cval) = caller_val {
                            if !matches!(cval, causm_core::value::Payload::String(_))
                            {
                                self.insert_reg(
                                    branch_id,
                                    args[i].0,
                                    causm_core::value::EntropicState::Valid(
                                        child_val,
                                    ),
                                )?;
                            }
                        }
                    }
                }
            }
        }

        let result = child_branch
            .return_value
            .or_else(|| {
                if params.is_empty() {
                    child_branch.arena.peek(0)
                } else {
                    None
                }
            })
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
        branch.return_value = Some(val.clone());
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
                    causm_core::value::EntropicState::Leased { .. } => valid_target,
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
