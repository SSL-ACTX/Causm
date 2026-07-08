use crate::gc::GarbageCollector;
use crate::vm::error::TemporalError;
use crate::vm::state::{AnchorPoint, Routine, SpeculationContext, Timeline, Vm};
use causm_core::value::{Arena, EntropicState, MemoryError, Payload, ValueMetadata};
use causm_core::{
    BinaryOperator, Capability, EntropyMode, Expression, MergeResolution, ParamMode,
    ResolutionStrategy, SpeculationCommitMode, Statement, TimeCoordinate,
};

use std::collections::{HashMap, VecDeque};

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            global_clock: 0,
            root_timeline: Timeline::new("main".to_string(), 1024 * 1024, 0),
            active_branches: HashMap::new(),
            capability_handlers: HashMap::new(),
            channels: HashMap::new(),
            pending_channels: HashMap::new(),
            routines: HashMap::new(),
            decay_handlers: HashMap::new(),
            type_decay_limits: HashMap::new(),
            speculation_stack: Vec::new(),
            speculative_commit_mode: SpeculationCommitMode::Selective,
            entanglements: Vec::new(),
            causal_history: Vec::new(),
            causal_trace: Vec::new(),
            debug_mode: false,
            next_payload_id: 0,
            trace_entropy: false,
            _is_decaying: false,
        }
    }

    pub fn register_capability<F>(&mut self, path: &str, handler: F)
    where
        F: Fn(&HashMap<String, String>) -> Result<(), String> + 'static,
    {
        self.capability_handlers
            .insert(path.to_string(), Box::new(handler));
    }

    pub fn set_speculative_commit_mode(&mut self, mode: SpeculationCommitMode) {
        self.speculative_commit_mode = mode;
    }

    pub fn peek_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
    ) -> Result<Payload, TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch
            .arena
            .peek(reg)
            .ok_or(TemporalError::MemoryFault(MemoryError::AlreadyConsumed))
    }

    pub fn peek_state(
        &mut self,
        branch_id: &str,
        reg: u32,
    ) -> Result<EntropicState, TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        Ok(branch
            .arena
            .registers
            .get(reg as usize)
            .cloned()
            .unwrap_or(EntropicState::Consumed))
    }

    pub(crate) fn insert_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
        state: EntropicState,
    ) -> Result<(), TemporalError> {
        let branch = self.get_branch_mut(branch_id)?;
        branch.arena.insert(reg, state)?;
        Ok(())
    }

    pub fn execute_program(
        &mut self,
        program: &causm_frontend::ir::IrProgram,
    ) -> Result<(), TemporalError> {
        self.symbols = program.symbols.clone();
        self.type_decay_limits = program.type_decay_limits.clone();
        // Register routines
        for (name, ir_routine) in &program.routines {
            let routine = Routine {
                params: ir_routine.params.clone(),
                return_type: ir_routine.return_type.clone(),
                taking_ms: ir_routine.taking_ms,
                instructions: ir_routine.instructions.clone(),
            };
            self.routines.insert(name.clone(), routine);
        }

        for block in &program.blocks {
            let branch_id = match &block.time {
                TimeCoordinate::Global(_) => "main",
                TimeCoordinate::Relative(_) => "main",
                TimeCoordinate::Branch(name) => name.as_str(),
            };

            {
                let branch = self.get_branch_mut(branch_id)?;
                branch.instructions = block.instructions.clone();
                branch.pc = 0;
            }

            loop {
                let (pc, len) = {
                    let branch = self.get_branch_mut(branch_id)?;
                    (branch.pc, branch.instructions.len())
                };
                if pc >= len {
                    break;
                }

                self.execute_instruction(branch_id)?;

                let b = self.get_branch_mut(branch_id)?;
                if b.break_requested {
                    let target_depth = b.loop_depth;
                    b.break_requested = false;
                    let _ = b;

                    while {
                        let b = self.get_branch_mut(branch_id)?;
                        b.pc < b.instructions.len()
                    } {
                        let instr = {
                            let b = self.get_branch_mut(branch_id)?;
                            b.instructions[b.pc].clone()
                        };
                        match instr {
                            causm_frontend::ir::Instruction::Loop { .. }
                            | causm_frontend::ir::Instruction::LoopTick => {
                                let b = self.get_branch_mut(branch_id)?;
                                b.loop_depth += 1;
                            }
                            causm_frontend::ir::Instruction::EndLoop { max_ms } => {
                                let b = self.get_branch_mut(branch_id)?;
                                b.loop_depth -= 1;
                                if b.loop_depth < target_depth {
                                    let max_ms_val = max_ms;
                                    self.EndLoop(branch_id, max_ms_val)?;
                                    let b = self.get_branch_mut(branch_id)?;
                                    b.pc += 1;
                                    // Skip the following Jump if present
                                    if b.pc < b.instructions.len() {
                                        if let causm_frontend::ir::Instruction::Jump { .. } =
                                            b.instructions[b.pc]
                                        {
                                            b.pc += 1;
                                        }
                                    }
                                    break;
                                }
                            }
                            causm_frontend::ir::Instruction::EndLoopTick => {
                                let b = self.get_branch_mut(branch_id)?;
                                b.loop_depth -= 1;
                                if b.loop_depth < target_depth {
                                    self.EndLoopTick(branch_id)?;
                                    let b = self.get_branch_mut(branch_id)?;
                                    b.pc += 1;
                                    // Skip the following Jump if present
                                    if b.pc < b.instructions.len() {
                                        if let causm_frontend::ir::Instruction::Jump { .. } =
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
                        let b = self.get_branch_mut(branch_id)?;
                        b.pc += 1;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn execute_instruction(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        if self.debug_mode {
            let branch = self.get_branch(branch_id)?;
            let snapshot = branch.clone();
            self.causal_trace.push((branch_id.to_string(), snapshot));
        }

        // Deterministic instruction cost
        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.local_clock += 1;
            branch.consume_budget(1)?;
        }

        let instr = {
            let branch = self.get_branch_mut(branch_id)?;
            if branch.pc >= branch.instructions.len() {
                return Ok(());
            }
            branch.instructions[branch.pc].clone()
        };

        // Advance PC before execution to handle jumps correctly
        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc += 1;
        }

        #[allow(non_snake_case)]
        macro_rules! dispatch_instruction {
            ($($name:ident $({ $($field:ident: $type:ty),* })?),*) => {
                match instr {
                    $(
                        causm_frontend::ir::Instruction::$name $({ $($field),* })? => {
                            self.$name(branch_id, $($($field),*)?)?
                        },
                    )*
                }
            };
        }

        causm_frontend::instructions!(dispatch_instruction);

        if self.trace_entropy {
            println!("\x1b[1;30m--- Entropy Trace [{}] ---\x1b[0m", branch_id);
            let branch = self.get_branch_mut(branch_id)?;
            for (i, state) in branch.arena.registers.iter().enumerate() {
                if !matches!(state, EntropicState::Consumed) {
                    println!(
                        "  \x1b[1;33mR{: <10}\x1b[0m: {}",
                        i,
                        state.render_decay(1)
                    );
                }
            }
        }

        Ok(())
    }

    pub fn commit_tick_buffers(&mut self) {
        for (name, pending) in self.pending_channels.iter_mut() {
            if let Some(chan) = self.channels.get_mut(name) {
                chan.append(pending);
            }
        }
    }

    pub(crate) fn get_branch(&self, id: &str) -> Result<&Timeline, TemporalError> {
        if id == "main" {
            Ok(&self.root_timeline)
        } else {
            self.active_branches
                .get(id)
                .ok_or_else(|| TemporalError::BranchNotFound(id.to_string()))
        }
    }

    pub fn get_branch_mut(
        &mut self,
        id: &str,
    ) -> Result<&mut Timeline, TemporalError> {
        if id == "main" {
            Ok(&mut self.root_timeline)
        } else {
            self.active_branches
                .get_mut(id)
                .ok_or_else(|| TemporalError::BranchNotFound(id.to_string()))
        }
    }

    pub(crate) fn _execute_capability(
        &mut self,
        branch_id: &str,
        cap: &Capability,
    ) -> Result<(), TemporalError> {
        let mut resolved_params = cap.parameters.clone();
        for (k, v) in &cap.parameters {
            let reg_opt = self.symbols.get(v).copied();
            if let Some(reg) = reg_opt {
                if let Ok(reg_val) = self.peek_reg(branch_id, reg.0) {
                    resolved_params.insert(k.clone(), reg_val.to_string());
                }
            }
        }

        // Enforce resource budgets
        let res_name = cap.path.replace(".", "_").to_lowercase();
        {
            let branch = self.get_branch_mut(branch_id)?;
            if let Some(budget) = branch.resource_budgets.get_mut(&res_name) {
                if *budget == 0 {
                    return Err(TemporalError::CapabilityViolation(format!(
                        "Capability budget exhausted: {}",
                        cap.path
                    )));
                }
                *budget -= 1;
            }
        }

        if cap.path == "System.Entropy"
            && resolved_params.get("mode") == Some(&"chaos".to_string())
        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.entropy_mode = EntropyMode::Chaos;
        }

        if let Some(handler) = self.capability_handlers.get(&cap.path) {
            handler(&resolved_params).map_err(TemporalError::CapabilityViolation)?;
            Ok(())
        } else if cap.path == "System.Entropy" {
            // System.Entropy is a built-in mode that does not require host handler.
            Ok(())
        } else {
            Err(TemporalError::MissingCapability(cap.path.clone()))
        }
    }

    pub(crate) fn _causal_rollback(
        &mut self,
        branch_id: &str,
        start_index: usize,
    ) -> Result<(), TemporalError> {
        for i in (start_index..self.causal_history.len()).rev() {
            let event = self.causal_history[i].clone();
            match event {
                crate::vm::state::CausalEvent::ChannelSend {
                    branch_id: b_id,
                    channel_id,
                    payload_id,
                } if b_id == branch_id => {
                    let payload_id_val = payload_id;
                    let channel_id_val = channel_id.clone();

                    let was_received = self.causal_history.iter().skip(i + 1).any(|e| {
                        match e {
                            crate::vm::state::CausalEvent::ChannelRecv { channel_id: c_id, message, .. } => {
                                let match_found = c_id == &channel_id_val && message.id == payload_id_val;
                                if match_found {
                                    println!("[VM] Paradox detected: message {} on chan {} was received by {} after send was rolled back", payload_id_val, channel_id_val, b_id);
                                }
                                match_found
                            }
                            _ => false,
                        }
                    });

                    if was_received {
                        return Err(TemporalError::Paradox);
                    }

                    let mut found = false;
                    if let Some(chan) = self.channels.get_mut(&channel_id_val) {
                        if let Some(pos) =
                            chan.iter().position(|m| m.id == payload_id_val)
                        {
                            chan.remove(pos);
                            found = true;
                        }
                    }
                    if !found {
                        if let Some(pending) =
                            self.pending_channels.get_mut(&channel_id_val)
                        {
                            if let Some(pos) =
                                pending.iter().position(|m| m.id == payload_id_val)
                            {
                                pending.remove(pos);
                                found = true;
                            }
                        }
                    }

                    if !found {
                        return Err(TemporalError::Paradox);
                    }
                }
                crate::vm::state::CausalEvent::ChannelRecv {
                    branch_id: b_id,
                    channel_id,
                    message,
                } if b_id == branch_id => {
                    if let Some(chan) = self.channels.get_mut(&channel_id) {
                        chan.push_front(message.clone());
                    } else {
                        return Err(TemporalError::Paradox);
                    }
                }
                crate::vm::state::CausalEvent::InterBranchMove {
                    source_branch,
                    target_branch,
                    reg,
                    message: _,
                } if source_branch == branch_id => {
                    let target = self.get_branch_mut(&target_branch)?;
                    match target.arena.registers.get(reg as usize) {
                        Some(EntropicState::Valid(_)) => {
                            target.arena.registers[reg as usize] =
                                EntropicState::Consumed;
                        }
                        _ => {
                            return Err(TemporalError::Paradox);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn _set_branch_state(&mut self, id: &str, state: Timeline) {
        if id == "main" {
            self.root_timeline = state;
        } else {
            self.active_branches.insert(id.to_string(), state);
        }
    }
}

impl Timeline {
    pub fn new(id: String, memory_capacity: u64, birth_time: u64) -> Self {
        Self {
            id,
            birth_global_time: birth_time,
            local_clock: 0,
            arena: Arena::new(memory_capacity),
            cpu_budget_ms: u64::MAX,
            slice_ms: None,
            anchors: HashMap::new(),
            commit_horizon_passed: false,
            manifest_stack: Vec::new(),
            resource_budgets: HashMap::new(),
            entropy_mode: EntropyMode::Deterministic,
            break_requested: false,
            loop_depth: 0,
            loop_stack: Vec::new(),
            pc: 0,
            instructions: Vec::new(),
        }
    }

    pub fn consume_budget(&mut self, amount: u64) -> Result<(), TemporalError> {
        if self.cpu_budget_ms < amount {
            return Err(TemporalError::BudgetExhausted);
        }
        self.cpu_budget_ms -= amount;
        Ok(())
    }
}
