use crate::vm::error::TemporalError;
use crate::vm::state::{Routine, Vm};
use causm_frontend::ir::{IrSelectCase, Reg};
use std::collections::HashMap;

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
        let is_true = match val {
            causm_core::value::Payload::Bool(b) => b,
            causm_core::value::Payload::Integer(i) => i != 0,
            _ => false,
        };

        if is_true {
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
        let is_true = match val {
            causm_core::value::Payload::Bool(b) => b,
            causm_core::value::Payload::Integer(i) => i != 0,
            _ => false,
        };

        if !is_true {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc = target;
        }
        Ok(())
    }

    pub(crate) fn Call(
        &mut self,
        branch_id: &str,
        routine: String,
        args: Vec<Reg>,
        dest: Reg,
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

        // SpeedMicro JIT Check
        if routine_def.taking_cycles.is_some() {
            let mut code_ptr_opt = self.jit_cache.get(&routine).copied();

            if code_ptr_opt.is_none() {
                if let Some(jit) = &mut self.jit {
                    let routine_ir = causm_frontend::ir::IrRoutine {
                        params: routine_def.params.clone(),
                        return_type: routine_def.return_type.clone(),
                        taking_ms: routine_def.taking_ms,
                        taking_cycles: routine_def.taking_cycles,
                        instructions: routine_def.instructions.clone(),
                    };
                    if let Ok(code_ptr) = jit.compile_routine(&routine, &routine_ir)
                    {
                        self.jit_cache.insert(routine.clone(), code_ptr);
                        code_ptr_opt = Some(code_ptr);
                    }
                }
            }

            if let Some(code_ptr) = code_ptr_opt {
                // Consume arguments
                for (i, reg) in args.iter().enumerate() {
                    let (mode, _, _) = &params[i];
                    if let causm_core::ParamMode::Consume = mode {
                        self.consume_reg(branch_id, reg.0)?;
                    }
                }

                let mut i64_args = Vec::new();
                for val in &arg_values {
                    match val {
                        causm_core::value::Payload::Integer(i) => i64_args.push(*i),
                        causm_core::value::Payload::Float(bits) => {
                            i64_args.push(*bits as i64)
                        }
                        _ => i64_args.push(0),
                    }
                }

                let mut child = crate::vm::state::Timeline::new(
                    format!("__jit_{}", routine),
                    1024 * 1024,
                    self.global_clock,
                );

                // SpeedMicro: Capture TSC immediately before call
                let start_tsc = crate::vm::jit::hw_timing::read_tsc();

                let res_i64 = if i64_args.len() == 2 {
                    type RoutineFn2 = extern "C" fn(
                        *mut Vm,
                        *mut crate::vm::state::Timeline,
                        i64, // start_tsc
                        i64,
                        i64,
                    ) -> i64;
                    let func: RoutineFn2 = unsafe { std::mem::transmute(code_ptr) };
                    func(
                        self as *mut Vm,
                        &mut child as *mut crate::vm::state::Timeline,
                        start_tsc as i64,
                        i64_args[0],
                        i64_args[1],
                    )
                } else if i64_args.len() == 1 {
                    type RoutineFn1 = extern "C" fn(
                        *mut Vm,
                        *mut crate::vm::state::Timeline,
                        i64, // start_tsc
                        i64,
                    ) -> i64;
                    let func: RoutineFn1 = unsafe { std::mem::transmute(code_ptr) };
                    func(
                        self as *mut Vm,
                        &mut child as *mut crate::vm::state::Timeline,
                        start_tsc as i64,
                        i64_args[0],
                    )
                } else {
                    type RoutineFn0 = extern "C" fn(
                        *mut Vm,
                        *mut crate::vm::state::Timeline,
                        i64, // start_tsc
                    ) -> i64;
                    let func: RoutineFn0 = unsafe { std::mem::transmute(code_ptr) };
                    func(
                        self as *mut Vm,
                        &mut child as *mut crate::vm::state::Timeline,
                        start_tsc as i64,
                    )
                };

                // SpeedMicro: Synchronize logical clock with cycle contract
                if let Some(cycles) = routine_def.taking_cycles {
                    let branch = self.get_branch_mut(branch_id)?;
                    // We just jumped forward by the contract.
                    // execute_instruction already added +1 to local_clock.
                    // So we add (cycles - 1) to land exactly on 'cycles'.
                    let adjusted_cycles = cycles.saturating_sub(1);
                    branch.local_clock =
                        branch.local_clock.saturating_add(adjusted_cycles);
                    // Also consume budget
                    branch.consume_budget(adjusted_cycles / 1000)?;
                }

                return self.insert_reg(
                    branch_id,
                    dest.0,
                    causm_core::value::EntropicState::Valid(
                        causm_core::value::Payload::Integer(res_i64),
                    ),
                );
            }
        }

        // Interpreter Path
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
        // SpeedMicro: Ensure child starts at 0 logical time
        child.local_clock = 0;
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

    pub(crate) fn MatchEntropy(
        &mut self,
        _branch_id: &str,
        _target: Reg,
        _valid_target: Option<usize>,
        _decayed_target: Option<usize>,
        _pending_target: Option<usize>,
        _consumed_target: Option<usize>,
    ) -> Result<(), TemporalError> {
        // TODO: Implement MatchEntropy
        Ok(())
    }

    pub(crate) fn Select(
        &mut self,
        _branch_id: &str,
        _max_ms: u64,
        _cases: Vec<causm_frontend::ir::IrSelectCase>,
        _timeout_target: Option<usize>,
    ) -> Result<(), TemporalError> {
        // TODO: Implement Select in interpreter
        Err(TemporalError::EvalError(
            "Select not implemented in interpreter".to_string(),
        ))
    }
}
