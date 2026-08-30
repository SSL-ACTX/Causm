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
            active_branches: indexmap::IndexMap::new(),

            capability_handlers: HashMap::new(),
            channels: HashMap::new(),
            pending_channels: HashMap::new(),
            channel_decay_limits: HashMap::new(),
            routines: HashMap::new(),
            decay_handlers: HashMap::new(),
            type_decay_limits: HashMap::new(),
            auto_drop_specs: HashMap::new(),
            struct_extends: HashMap::new(),
            speculation_stack: Vec::new(),
            speculative_commit_mode: SpeculationCommitMode::Selective,
            entanglements: Vec::new(),
            causal_history: Vec::new(),
            causal_trace: Vec::new(),
            debug_mode: false,
            next_payload_id: 0,
            next_call_id: 0,
            trace_entropy: false,
            trace_causal: false,
            _is_decaying: false,
            current_span: None,
            call_depth: 0,
            max_call_depth: 10_000,
            foreign_manager: std::sync::Arc::new(
                crate::vm::ffi::ForeignLibraryManager::new(),
            ),
        }
    }

    pub fn register_capability<F>(&mut self, path: &str, handler: F)
    where
        F: Fn(&HashMap<String, String>) -> Result<Payload, String> + 'static,
    {
        self.capability_handlers
            .insert(path.to_string(), Box::new(handler));
    }

    pub fn set_speculative_commit_mode(&mut self, mode: SpeculationCommitMode) {
        self.speculative_commit_mode = mode;
    }

    pub fn execute_instructions(
        &mut self,
        branch_id: &str,
        instructions: &[causm_ir::Instruction],
    ) -> Result<(), TemporalError> {
        let (saved_pc, saved_instructions, saved_spans) = {
            let branch = self.get_branch_mut(branch_id)?;
            let pc = branch.pc;
            let instrs = std::mem::take(&mut branch.instructions);
            let spans = std::mem::take(&mut branch.spans);
            (pc, instrs, spans)
        };

        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.instructions = instructions.to_vec();
            branch.spans = vec![None; instructions.len()];
            branch.pc = 0;
        }

        while {
            let branch = self.get_branch_mut(branch_id)?;
            branch.pc < branch.instructions.len()
        } {
            {
                let branch = self.get_branch_mut(branch_id)?;
                branch.total_executed_cycles += 1;
                if branch.total_executed_cycles > branch.max_cycles_watchdog {
                    return Err(TemporalError::WatchdogBite(
                        branch_id.to_string(),
                        branch.max_cycles_watchdog,
                    ));
                }
            }
            self.execute_instruction(branch_id)?;
            self.handle_break(branch_id)?;
        }

        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.instructions = saved_instructions;
            branch.spans = saved_spans;
            branch.pc = saved_pc;
        }

        Ok(())
    }

    pub fn check_and_apply_decay(
        &mut self,
        branch_id: &str,
        reg: u32,
    ) -> Result<(), TemporalError> {
        if self._is_decaying {
            return Ok(());
        }

        let has_decay = {
            let branch = self.get_branch(branch_id)?;
            if let Some(meta) = branch.arena.get_metadata(reg) {
                if let Some(decay_after_ms) = meta.decay_after_ms {
                    let current_time = branch.birth_global_time + branch.local_clock;
                    if current_time >= meta.instantiated_at {
                        let elapsed = current_time - meta.instantiated_at;
                        if elapsed >= decay_after_ms {
                            let idx = reg as usize;
                            idx < branch.arena.registers.len()
                                && matches!(
                                    branch.arena.registers[idx],
                                    EntropicState::Valid(_)
                                )
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        if has_decay {
            let type_name = {
                let branch = self.get_branch_mut(branch_id)?;
                let idx = reg as usize;
                let old_state = std::mem::replace(
                    &mut branch.arena.registers[idx],
                    EntropicState::Consumed,
                );
                let decayed_state = old_state.decay_recursive();
                branch.arena.registers[idx] = decayed_state;
                branch.arena.compact_consumed();
                branch
                    .arena
                    .get_metadata(reg)
                    .and_then(|m| m.type_name.clone())
            };

            if let Some(type_name) = type_name {
                if let Some(handler_instrs) =
                    self.decay_handlers.get(&type_name).cloned()
                {
                    self._is_decaying = true;
                    let res = self.execute_instructions(branch_id, &handler_instrs);
                    self._is_decaying = false;
                    res?;
                }
            }
        }

        Ok(())
    }

    pub fn peek_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
    ) -> Result<Payload, TemporalError> {
        let has_meta = {
            let branch = self.get_branch(branch_id)?;
            let idx = reg as usize;
            idx < branch.arena.metadata.len() && branch.arena.metadata[idx].is_some()
        };
        if has_meta {
            self.check_and_apply_decay(branch_id, reg)?;
        }
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
        let has_meta = {
            let branch = self.get_branch(branch_id)?;
            let idx = reg as usize;
            idx < branch.arena.metadata.len() && branch.arena.metadata[idx].is_some()
        };
        if has_meta {
            self.check_and_apply_decay(branch_id, reg)?;
        }
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
        if let Err(e) = branch.arena.insert(reg, state.clone()) {
            match e {
                MemoryError::OutOfMemory(_, _) => {
                    // Check if a saturation policy is configured for OnFull or OnOverflow
                    let policy = branch
                        .saturation_policies
                        .get(&causm_core::PolicyTarget::OnFull)
                        .or_else(|| {
                            branch
                                .saturation_policies
                                .get(&causm_core::PolicyTarget::OnOverflow)
                        })
                        .copied();

                    match policy {
                        Some(causm_core::SaturationPolicy::EvictDecayed) => {
                            branch.arena.evict_decayed();
                            branch.arena.insert(reg, state)?;
                        }
                        Some(causm_core::SaturationPolicy::RingBuffer) => {
                            // Reset transient partition to base watermark and insert
                            branch.arena.reset_to_base_watermark();
                            branch.arena.insert(reg, state)?;
                        }
                        Some(causm_core::SaturationPolicy::FailFast) | None => {
                            return Err(TemporalError::MemoryFault(e));
                        }
                        Some(causm_core::SaturationPolicy::Throttle) => {
                            // Add temporal latency penalty and try inserting
                            branch.local_clock += 10;
                            branch.arena.evict_decayed();
                            branch.arena.insert(reg, state)?;
                        }
                    }
                }
                other => return Err(TemporalError::MemoryFault(other)),
            }
        }
        Ok(())
    }

    pub(crate) fn insert_reg_with_metadata(
        &mut self,
        branch_id: &str,
        reg: u32,
        state: EntropicState,
        meta: Option<ValueMetadata>,
    ) -> Result<(), TemporalError> {
        self.insert_reg(branch_id, reg, state)?;
        if let Some(m) = meta {
            let branch = self.get_branch_mut(branch_id)?;
            branch.arena.set_metadata(reg, Some(m));
        }
        Ok(())
    }

    pub fn execute_program(
        &mut self,
        program: &causm_ir::IrProgram,
    ) -> Result<(), TemporalError> {
        self.symbols = program.symbols.clone();
        self.type_decay_limits = program.type_decay_limits.clone();
        self.auto_drop_specs = program.auto_drop_specs.clone();
        self.struct_extends = program.struct_extends.clone();
        self.decay_handlers = program.decay_handlers.clone();
        // Register routines
        for (name, ir_routine) in &program.routines {
            let routine = Routine {
                params: ir_routine.params.clone(),
                return_type: ir_routine.return_type.clone(),
                taking_ms: ir_routine.taking_ms,
                foreign_binding: ir_routine.foreign_binding.clone(),
                instructions: ir_routine.instructions.clone(),
                spans: ir_routine.spans.clone(),
            };
            self.routines.insert(name.clone(), routine);
        }

        for block in &program.blocks {
            let branch_id = match &block.time {
                TimeCoordinate::Global(_) => "main",
                TimeCoordinate::Relative(_) => "main",
                TimeCoordinate::Branch(name) => name.as_str(),
                TimeCoordinate::Periodic(_) => "main",
            };

            if matches!(&block.time, TimeCoordinate::Periodic(_)) {
                let branch = self.get_branch_mut(branch_id)?;
                branch.arena.freeze_base_watermark();
            }

            {
                let branch = self.get_branch_mut(branch_id)?;
                if let Some(mode) = block.entropy_mode {
                    branch.entropy_mode = mode;
                }
                branch.instructions = block.instructions.clone();
                branch.spans = block.spans.clone();
                branch.pc = 0;
            }

            loop {
                let (pc, len) = {
                    let branch = self.get_branch_mut(branch_id)?;
                    branch.total_executed_cycles += 1;
                    if branch.total_executed_cycles > branch.max_cycles_watchdog {
                        return Err(TemporalError::WatchdogBite(
                            branch_id.to_string(),
                            branch.max_cycles_watchdog,
                        ));
                    }
                    (branch.pc, branch.instructions.len())
                };
                if pc >= len {
                    break;
                }

                self.execute_instruction(branch_id)?;
                self.handle_break(branch_id)?;
            }

            if let TimeCoordinate::Periodic(interval_ms) = &block.time {
                let branch = self.get_branch_mut(branch_id)?;
                branch.arena.reset_to_base_watermark();
                if branch.local_clock < *interval_ms {
                    let padding = *interval_ms - branch.local_clock;
                    branch.local_clock = *interval_ms;
                    branch.consume_budget(padding)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn handle_break(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        let b = self.get_branch_mut(branch_id)?;
        if !b.break_requested {
            return Ok(());
        }
        let target_depth = b.loop_depth.saturating_sub(1);
        b.break_requested = false;

        while {
            let b = self.get_branch_mut(branch_id)?;
            b.pc < b.instructions.len()
        } {
            let instr = {
                let b = self.get_branch_mut(branch_id)?;
                b.instructions[b.pc].clone()
            };
            match instr {
                causm_ir::Instruction::Loop { .. }
                | causm_ir::Instruction::LoopTick
                | causm_ir::Instruction::LoopTickOn { .. }
                | causm_ir::Instruction::While { .. }
                | causm_ir::Instruction::For { .. }
                | causm_ir::Instruction::ForStep { .. } => {
                    let b = self.get_branch_mut(branch_id)?;
                    b.loop_depth += 1;
                }
                causm_ir::Instruction::EndFor => {
                    let b = self.get_branch_mut(branch_id)?;
                    b.loop_depth = b.loop_depth.saturating_sub(1);
                    if b.loop_depth <= target_depth {
                        let b = self.get_branch_mut(branch_id)?;
                        b.loop_depth = target_depth;
                        b.flat_loops.pop();
                        b.pc += 1;
                        if b.pc < b.instructions.len() {
                            if let causm_ir::Instruction::Jump { .. } =
                                b.instructions[b.pc]
                            {
                                b.pc += 1;
                            }
                        }
                        return Ok(());
                    }
                }
                causm_ir::Instruction::EndForStep => {
                    let b = self.get_branch_mut(branch_id)?;
                    b.loop_depth = b.loop_depth.saturating_sub(1);
                    if b.loop_depth <= target_depth {
                        let b = self.get_branch_mut(branch_id)?;
                        b.loop_depth = target_depth;
                        b.flat_loops.pop();
                        b.pc += 1;
                        if b.pc < b.instructions.len() {
                            if let causm_ir::Instruction::Jump { .. } =
                                b.instructions[b.pc]
                            {
                                b.pc += 1;
                            }
                        }
                        return Ok(());
                    }
                }
                causm_ir::Instruction::EndLoop { max_ms } => {
                    let b = self.get_branch_mut(branch_id)?;
                    b.loop_depth = b.loop_depth.saturating_sub(1);
                    if b.loop_depth <= target_depth {
                        let max_ms_val = max_ms;
                        self.EndLoop(branch_id, max_ms_val)?;
                        let b = self.get_branch_mut(branch_id)?;
                        b.pc += 1;
                        // Skip the following Jump if present
                        if b.pc < b.instructions.len() {
                            if let causm_ir::Instruction::Jump { .. } =
                                b.instructions[b.pc]
                            {
                                b.pc += 1;
                            }
                        }
                        return Ok(());
                    }
                }
                causm_ir::Instruction::EndWhile { max_ms } => {
                    let b = self.get_branch_mut(branch_id)?;
                    b.loop_depth = b.loop_depth.saturating_sub(1);
                    if b.loop_depth <= target_depth {
                        let max_ms_val = max_ms;
                        self.EndWhile(branch_id, max_ms_val)?;
                        let b = self.get_branch_mut(branch_id)?;
                        b.pc += 1;
                        // Skip the following Jump if present
                        if b.pc < b.instructions.len() {
                            if let causm_ir::Instruction::Jump { .. } =
                                b.instructions[b.pc]
                            {
                                b.pc += 1;
                            }
                        }
                        return Ok(());
                    }
                }
                causm_ir::Instruction::EndLoopTick => {
                    let b = self.get_branch_mut(branch_id)?;
                    b.loop_depth = b.loop_depth.saturating_sub(1);
                    if b.loop_depth <= target_depth {
                        self.EndLoopTick(branch_id)?;
                        let b = self.get_branch_mut(branch_id)?;
                        b.pc += 1;
                        // Skip the following Jump if present
                        if b.pc < b.instructions.len() {
                            if let causm_ir::Instruction::Jump { .. } =
                                b.instructions[b.pc]
                            {
                                b.pc += 1;
                            }
                        }
                        return Ok(());
                    }
                }
                _ => {}
            }
            let b = self.get_branch_mut(branch_id)?;
            b.pc += 1;
        }
        Ok(())
    }

    pub fn execute_instruction(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        if self.debug_mode || self.trace_causal {
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

        let (instr, span) = {
            let branch = self.get_branch_mut(branch_id)?;
            if branch.pc >= branch.instructions.len() {
                return Ok(());
            }
            (
                branch.instructions[branch.pc].clone(),
                branch.spans.get(branch.pc).cloned().flatten(),
            )
        };
        self.current_span = span;

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
                        causm_ir::Instruction::$name $({ $($field),* })? => {
                            if let Err(e) = self.$name(branch_id, $($($field),*)?) {
                                if self.debug_mode {
                                    let span_str = self
                                        .current_span
                                        .as_ref()
                                        .map(|s| format!("span {}-{}", s.start, s.end))
                                        .unwrap_or_else(|| "unknown span".to_string());
                                    let clock = self
                                        .get_branch(branch_id)
                                        .map(|b| b.local_clock)
                                        .unwrap_or(0);
                                    eprintln!(
                                        "[TVM FAULT] [{}] @{}ms ({}) | instruction: {} -> {}",
                                        branch_id,
                                        clock,
                                        span_str,
                                        stringify!($name),
                                        e
                                    );
                                }
                                return Err(e);
                            }
                        },
                    )*
                }
            };
        }

        causm_ir::instructions!(dispatch_instruction);

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

    pub fn format_diagnostic(
        &self,
        branch_id: &str,
        error: &TemporalError,
    ) -> String {
        let span_info = if let Some(span) = &self.current_span {
            format!(" at span {}-{}", span.start, span.end)
        } else {
            String::new()
        };
        let branch_clock = self
            .get_branch(branch_id)
            .map(|b| b.local_clock)
            .unwrap_or(0);
        format!(
            "Runtime Error{} [Branch: '{}' @ {}ms]: {}",
            span_info, branch_id, branch_clock, error
        )
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
    ) -> Result<Payload, TemporalError> {
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
            handler(&resolved_params).map_err(TemporalError::CapabilityViolation)
        } else if cap.path == "System.Entropy" {
            // System.Entropy is a built-in mode that does not require host handler.
            Ok(Payload::Null)
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
            flat_loops: Vec::new(),
            total_executed_cycles: 0,
            max_cycles_watchdog: 500_000, // 500,000 instruction cycle ceiling per branch segment
            call_depth: 0,
            saturation_policies: HashMap::new(),
            pc: 0,
            instructions: Vec::new(),
            spans: Vec::new(),
            return_value: None,
            call_stack: Vec::new(),
        }
    }

    pub fn fork_from(id: String, parent: &Timeline, birth_time: u64) -> Self {
        let mut child = Self::new(id, parent.arena.capacity, birth_time);
        child.arena = parent.arena.clone();
        child.cpu_budget_ms = parent.cpu_budget_ms;
        child.slice_ms = parent.slice_ms;
        child.resource_budgets = parent.resource_budgets.clone();
        child.entropy_mode = parent.entropy_mode;
        child
    }

    pub fn consume_budget(&mut self, amount: u64) -> Result<(), TemporalError> {
        if self.cpu_budget_ms < amount {
            return Err(TemporalError::BudgetExhausted);
        }
        self.cpu_budget_ms -= amount;
        Ok(())
    }
}
