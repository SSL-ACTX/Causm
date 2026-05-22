use crate::gc::GarbageCollector;
use crate::vm::error::TemporalError;
use crate::vm::state::{AnchorPoint, Routine, SpeculationContext, Timeline, Vm};
use ictl_core::value::{Arena, EntropicState, MemoryError, Payload, ValueMetadata};
use ictl_core::{
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

    pub(crate) fn peek_reg(
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

    pub(crate) fn peek_state(
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
        program: &ictl_frontend::ir::IrProgram,
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
                            ictl_frontend::ir::Instruction::Loop { .. }
                            | ictl_frontend::ir::Instruction::LoopTick => {
                                let b = self.get_branch_mut(branch_id)?;
                                b.loop_depth += 1;
                            }
                            ictl_frontend::ir::Instruction::EndLoop { max_ms } => {
                                let b = self.get_branch_mut(branch_id)?;
                                b.loop_depth -= 1;
                                if b.loop_depth < target_depth {
                                    let max_ms_val = max_ms;
                                    self.EndLoop(branch_id, max_ms_val)?;
                                    let b = self.get_branch_mut(branch_id)?;
                                    b.pc += 1;
                                    // Skip the following Jump if present
                                    if b.pc < b.instructions.len() {
                                        if let ictl_frontend::ir::Instruction::Jump { .. } =
                                            b.instructions[b.pc]
                                        {
                                            b.pc += 1;
                                        }
                                    }
                                    break;
                                }
                            }
                            ictl_frontend::ir::Instruction::EndLoopTick => {
                                let b = self.get_branch_mut(branch_id)?;
                                b.loop_depth -= 1;
                                if b.loop_depth < target_depth {
                                    self.EndLoopTick(branch_id)?;
                                    let b = self.get_branch_mut(branch_id)?;
                                    b.pc += 1;
                                    // Skip the following Jump if present
                                    if b.pc < b.instructions.len() {
                                        if let ictl_frontend::ir::Instruction::Jump { .. } =
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

    pub fn consume_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
    ) -> Result<(), TemporalError> {
        let mut to_consume = Vec::new();
        to_consume.push((branch_id.to_string(), reg));

        // Find all entangled registers
        let mut entangled_found = true;
        while entangled_found {
            entangled_found = false;
            let current_to_consume = to_consume.clone();
            for set in &self.entanglements {
                if current_to_consume.iter().any(|item| set.contains(item)) {
                    for entangled in set {
                        if !to_consume.contains(entangled) {
                            to_consume.push(entangled.clone());
                            entangled_found = true;
                        }
                    }
                }
            }
        }

        for (b_id, r_id) in to_consume {
            if let Ok(branch) = self.get_branch_mut(&b_id) {
                branch.arena.consume(r_id).ok(); // Ignore if already consumed
            }
        }
        Ok(())
    }

    pub fn consume_field_reg(
        &mut self,
        branch_id: &str,
        reg: u32,
        field: &str,
    ) -> Result<(), TemporalError> {
        let mut to_consume = Vec::new();
        to_consume.push((branch_id.to_string(), reg));

        // Find all entangled registers
        let mut entangled_found = true;
        while entangled_found {
            entangled_found = false;
            let current_to_consume = to_consume.clone();
            for set in &self.entanglements {
                if current_to_consume.iter().any(|item| set.contains(item)) {
                    for entangled in set {
                        if !to_consume.contains(entangled) {
                            to_consume.push(entangled.clone());
                            entangled_found = true;
                        }
                    }
                }
            }
        }

        for (b_id, r_id) in to_consume {
            if let Ok(branch) = self.get_branch_mut(&b_id) {
                branch.arena.consume_field(r_id, field).ok();
            }
        }
        Ok(())
    }

    pub fn execute_instruction(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
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
                        ictl_frontend::ir::Instruction::$name $({ $($field),* })? => {
                            self.$name(branch_id, $($($field),*)?)?
                        },
                    )*
                }
            };
        }

        ictl_frontend::instructions!(dispatch_instruction);

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

    pub(crate) fn is_intrinsic(&self, name: &str) -> bool {
        matches!(
            name,
            "sqrt"
                | "sin"
                | "cos"
                | "tan"
                | "exp"
                | "ln"
                | "log10"
                | "floor"
                | "ceil"
                | "round"
        )
    }

    pub(crate) fn call_intrinsic(
        &self,
        name: &str,
        args: Vec<Payload>,
    ) -> Result<Payload, TemporalError> {
        if args.len() != 1 {
            return Err(TemporalError::EvalError(format!(
                "{} expects 1 argument",
                name
            )));
        }
        let f = args[0].as_float().ok_or_else(|| {
            TemporalError::TypeMismatch(format!("{} expects numeric", name))
        })?;
        let res = match name {
            "sqrt" => f.sqrt(),
            "sin" => f.sin(),
            "cos" => f.cos(),
            "tan" => f.tan(),
            "exp" => f.exp(),
            "ln" => f.ln(),
            "log10" => f.log10(),
            "floor" => f.floor(),
            "ceil" => f.ceil(),
            "round" => f.round(),
            _ => unreachable!(),
        };
        Ok(Payload::Float(res.to_bits()))
    }

    pub(crate) fn evaluate_unary_operation(
        &self,
        val: Payload,
        op: &ictl_core::UnaryOperator,
    ) -> Result<Payload, TemporalError> {
        match op {
            ictl_core::UnaryOperator::Neg => match val {
                Payload::Integer(i) => Ok(Payload::Integer(-i)),
                Payload::Float(bits) => {
                    let f = f64::from_bits(bits);
                    Ok(Payload::Float((-f).to_bits()))
                }
                _ => Err(TemporalError::TypeMismatch(format!(
                    "Cannot negate {:?}",
                    val
                ))),
            },
            ictl_core::UnaryOperator::Not => match val {
                Payload::Bool(b) => Ok(Payload::Bool(!b)),
                _ => Err(TemporalError::TypeMismatch(format!(
                    "Cannot apply NOT to {:?}",
                    val
                ))),
            },
        }
    }

    pub(crate) fn evaluate_binary_operation(
        &self,
        left_value: Payload,
        right_value: Payload,
        op: &BinaryOperator,
    ) -> Result<Payload, TemporalError> {
        let result = match (left_value, right_value) {
            (Payload::String(l), r) if op == &BinaryOperator::Add => {
                Payload::String(format!("{}{}", l, r))
            }
            (l, Payload::String(r)) if op == &BinaryOperator::Add => {
                Payload::String(format!("{}{}", l, r))
            }
            (Payload::Integer(l), Payload::Integer(r)) => match op {
                BinaryOperator::Add => Payload::Integer(l + r),
                BinaryOperator::Sub => Payload::Integer(l - r),
                BinaryOperator::Mul => Payload::Integer(l * r),
                BinaryOperator::Div => {
                    if r == 0 {
                        return Err(TemporalError::EvalError(
                            "Division by zero".into(),
                        ));
                    }
                    Payload::Integer(l / r)
                }
                BinaryOperator::Rem => {
                    if r == 0 {
                        return Err(TemporalError::EvalError(
                            "Modulo by zero".into(),
                        ));
                    }
                    Payload::Integer(l % r)
                }
                BinaryOperator::Pow => {
                    if r < 0 {
                        let lf = l as f64;
                        let rf = r as f64;
                        Payload::Float(lf.powf(rf).to_bits())
                    } else {
                        Payload::Integer(l.pow(r as u32))
                    }
                }
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                BinaryOperator::Lt => Payload::Bool(l < r),
                BinaryOperator::Gt => Payload::Bool(l > r),
                BinaryOperator::Le => Payload::Bool(l <= r),
                BinaryOperator::Ge => Payload::Bool(l >= r),
            },
            (l, r) if l.is_numeric() && r.is_numeric() => {
                let lf = l.as_float().unwrap();
                let rf = r.as_float().unwrap();
                match op {
                    BinaryOperator::Add => Payload::Float((lf + rf).to_bits()),
                    BinaryOperator::Sub => Payload::Float((lf - rf).to_bits()),
                    BinaryOperator::Mul => Payload::Float((lf * rf).to_bits()),
                    BinaryOperator::Div => {
                        if rf == 0.0 {
                            return Err(TemporalError::EvalError(
                                "Division by zero".into(),
                            ));
                        }
                        Payload::Float((lf / rf).to_bits())
                    }
                    BinaryOperator::Rem => {
                        if rf == 0.0 {
                            return Err(TemporalError::EvalError(
                                "Modulo by zero".into(),
                            ));
                        }
                        Payload::Float((lf % rf).to_bits())
                    }
                    BinaryOperator::Pow => Payload::Float(lf.powf(rf).to_bits()),
                    BinaryOperator::Eq => Payload::Bool(lf == rf),
                    BinaryOperator::Neq => Payload::Bool(lf != rf),
                    BinaryOperator::Lt => Payload::Bool(lf < rf),
                    BinaryOperator::Gt => Payload::Bool(lf > rf),
                    BinaryOperator::Le => Payload::Bool(lf <= rf),
                    BinaryOperator::Ge => Payload::Bool(lf >= rf),
                }
            }
            (Payload::Bool(l), Payload::Bool(r)) => match op {
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                _ => {
                    return Err(TemporalError::EvalError(
                        "Invalid boolean operator".into(),
                    ))
                }
            },
            (Payload::String(l), Payload::String(r)) => match op {
                BinaryOperator::Add => Payload::String(format!("{}{}", l, r)),
                BinaryOperator::Eq => Payload::Bool(l == r),
                BinaryOperator::Neq => Payload::Bool(l != r),
                _ => {
                    return Err(TemporalError::EvalError(
                        "String operator unsupported".into(),
                    ))
                }
            },
            (l, r) => {
                return Err(TemporalError::TypeMismatch(format!(
                    "Type mismatch in binary op: {:?} {:?} {:?}",
                    l, op, r
                )));
            }
        };

        Ok(result)
    }

    pub(crate) fn _execute_capability(
        &mut self,
        branch_id: &str,
        cap: &Capability,
    ) -> Result<(), TemporalError> {
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
            && cap.parameters.get("mode") == Some(&"chaos".to_string())
        {
            let branch = self.get_branch_mut(branch_id)?;
            branch.entropy_mode = EntropyMode::Chaos;
        }

        if let Some(handler) = self.capability_handlers.get(&cap.path) {
            handler(&cap.parameters).map_err(TemporalError::CapabilityViolation)?;
            Ok(())
        } else if cap.path == "System.Entropy" {
            // System.Entropy is a built-in mode that does not require host handler.
            Ok(())
        } else {
            Err(TemporalError::MissingCapability(cap.path.clone()))
        }
    }

    pub fn split_timeline(
        &mut self,
        parent_id: &str,
        branches: Vec<&str>,
    ) -> Result<(), TemporalError> {
        let (base_arena, cpu_budget_ms, entropy_mode, resource_budgets, slice_ms) = {
            let parent_timeline = if parent_id == "main" {
                &self.root_timeline
            } else {
                self.active_branches.get(parent_id).ok_or_else(|| {
                    TemporalError::BranchNotFound(parent_id.to_string())
                })?
            };
            (
                parent_timeline.arena.clone(),
                parent_timeline.cpu_budget_ms,
                parent_timeline.entropy_mode,
                parent_timeline.resource_budgets.clone(),
                parent_timeline.slice_ms,
            )
        };

        for branch_name in branches {
            let new_branch = Timeline {
                id: branch_name.to_string(),
                birth_global_time: self.global_clock,
                local_clock: 0,
                arena: base_arena.clone(),
                cpu_budget_ms,
                slice_ms,
                anchors: HashMap::new(),
                commit_horizon_passed: false,
                manifest_stack: Vec::new(),
                resource_budgets: resource_budgets.clone(),
                entropy_mode,
                break_requested: false,
                loop_depth: 0,
                loop_stack: Vec::new(),
                pc: 0,
                instructions: Vec::new(),
            };
            self.active_branches
                .insert(branch_name.to_string(), new_branch);

            // Propagate entanglement groups to new branch
            let mut new_entries = Vec::new();
            for group in &self.entanglements {
                let mut found_parent = false;
                let mut regs_to_add = Vec::new();
                for (b, r) in group {
                    if b == parent_id {
                        found_parent = true;
                        regs_to_add.push(*r);
                    }
                }
                if found_parent {
                    new_entries.push(regs_to_add);
                }
            }

            for regs in new_entries {
                for r in regs {
                    for group in &mut self.entanglements {
                        if group.contains(&(parent_id.to_string(), r)) {
                            group.insert((branch_name.to_string(), r));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn propagate_entanglement(
        &mut self,
        source_branch: &str,
        reg: u32,
    ) -> Result<(), TemporalError> {
        let mut groups_to_propagate = Vec::new();
        for (i, group) in self.entanglements.iter().enumerate() {
            if group.contains(&(source_branch.to_string(), reg)) {
                groups_to_propagate.push(i);
            }
        }

        for idx in groups_to_propagate {
            let group = self.entanglements[idx].clone();
            for (target_branch, target_reg) in group {
                if target_branch == source_branch && target_reg == reg {
                    continue;
                }
                // Mark as consumed in target branch
                if let Ok(branch) = self.get_branch_mut(&target_branch) {
                    branch.arena.set_consumed(target_reg).ok();
                }
            }
        }
        Ok(())
    }

    pub fn propagate_field_decay(
        &mut self,
        source_branch: &str,
        reg: u32,
        field_name: &str,
    ) -> Result<(), TemporalError> {
        let mut groups_to_propagate = Vec::new();
        for (i, group) in self.entanglements.iter().enumerate() {
            if group.contains(&(source_branch.to_string(), reg)) {
                groups_to_propagate.push(i);
            }
        }

        for idx in groups_to_propagate {
            let group = self.entanglements[idx].clone();
            for (target_branch, target_reg) in group {
                if target_branch == source_branch && target_reg == reg {
                    continue;
                }
                // Mark field as consumed in target branch
                if let Ok(branch) = self.get_branch_mut(&target_branch) {
                    branch.arena.consume_field(target_reg, field_name).ok();
                }
            }
        }
        Ok(())
    }

    pub fn merge_timelines(
        &mut self,
        branches: Vec<&str>,
        target: &str,
        resolution: &MergeResolution,
    ) -> Result<(), TemporalError> {
        let mut merged_registers: Vec<Option<EntropicState>> = Vec::new();
        let mut pending_reversion = None;

        // Build a mapping from register ID to resolution strategy
        let mut reg_resolutions: HashMap<u32, ResolutionStrategy> = HashMap::new();
        for (name, strategy) in &resolution.rules {
            if let Some(reg) = self.symbols.get(name) {
                reg_resolutions.insert(reg.0, strategy.clone());
            }
        }

        for branch_name in &branches {
            let branch =
                self.active_branches.get(*branch_name).ok_or_else(|| {
                    TemporalError::BranchNotFound(branch_name.to_string())
                })?;

            if merged_registers.len() < branch.arena.registers.len() {
                merged_registers.resize(branch.arena.registers.len(), None);
            }

            for (idx, state) in branch.arena.registers.iter().enumerate() {
                if let Some(existing) = &merged_registers[idx] {
                    let strategy = reg_resolutions
                        .get(&(idx as u32))
                        .unwrap_or(&ResolutionStrategy::Auto);
                    let (resolved, rev) = self.resolve_entropic_conflict(
                        &idx.to_string(),
                        existing,
                        state,
                        strategy,
                        branch_name,
                    );
                    merged_registers[idx] = Some(resolved);
                    if pending_reversion.is_none() {
                        pending_reversion = rev;
                    }
                } else {
                    merged_registers[idx] = Some(state.clone());
                }
            }
        }

        if let Some(reversion) = pending_reversion {
            let anchor = {
                let target_branch = self.get_branch_mut(&reversion.branch)?;
                target_branch
                    .anchors
                    .get(&reversion.anchor)
                    .ok_or_else(|| {
                        TemporalError::AnchorNotFound(reversion.anchor.clone())
                    })?
                    .clone()
            };

            let target_branch = self.get_branch_mut(&reversion.branch)?;
            target_branch.arena = anchor.arena_snapshot;
            target_branch.local_clock = anchor.clock_snapshot;
            target_branch.cpu_budget_ms = anchor.cpu_budget_snapshot;
            target_branch.resource_budgets = anchor.resource_budgets_snapshot;
            target_branch.commit_horizon_passed = false;
            target_branch.pc = anchor.pc_snapshot;

            return Ok(());
        }

        let target_branch = self.get_branch_mut(target)?;
        for (idx, v) in merged_registers.into_iter().enumerate() {
            if let Some(state) = v {
                target_branch.arena.insert(idx as u32, state)?;
            }
        }
        for b in branches {
            if let Some(branch) = self.active_branches.remove(b) {
                GarbageCollector::collect_branch(branch);
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

    #[allow(dead_code)]
    pub fn terminate_branch(
        &mut self,
        branch_id: &str,
    ) -> Result<(), TemporalError> {
        if branch_id == "main" {
            return Err(TemporalError::BranchNotFound(branch_id.to_string()));
        }

        if self.active_branches.contains_key(branch_id) {
            GarbageCollector::collect_branch_by_id(self, branch_id);
            Ok(())
        } else {
            Err(TemporalError::BranchNotFound(branch_id.to_string()))
        }
    }

    pub(crate) fn get_branch_mut(
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

    pub(crate) fn resolve_entropic_conflict(
        &self,
        _key: &str,
        existing: &EntropicState,
        incoming: &EntropicState,
        strategy: &ResolutionStrategy,
        incoming_branch: &str,
    ) -> (EntropicState, Option<ictl_core::CausalReversion>) {
        if matches!(existing, EntropicState::Consumed)
            || matches!(incoming, EntropicState::Consumed)
        {
            return (EntropicState::Consumed, None);
        }

        match strategy {
            ResolutionStrategy::FirstWins => (existing.clone(), None),
            ResolutionStrategy::Priority(p) => {
                if incoming_branch == p {
                    (incoming.clone(), None)
                } else {
                    (existing.clone(), None)
                }
            }
            ResolutionStrategy::Decay => (EntropicState::Consumed, None),
            ResolutionStrategy::TopologyUnion {
                key_rules,
                default,
                on_invalid,
            } => {
                match (existing, incoming) {
                    (
                        EntropicState::Valid(Payload::Topology(f1)),
                        EntropicState::Valid(Payload::Topology(f2)),
                    ) => {
                        let mut merged_fields = f1.clone();
                        let mut final_reversion = None;

                        for (field_name, incoming_f_state) in f2 {
                            if let Some(existing_f_state) =
                                merged_fields.get(field_name)
                            {
                                let field_strategy =
                                    key_rules.get(field_name).unwrap_or(default);
                                let (resolved_f, rev) = self
                                    .resolve_entropic_conflict(
                                        field_name,
                                        existing_f_state,
                                        incoming_f_state,
                                        field_strategy,
                                        incoming_branch,
                                    );
                                merged_fields.insert(field_name.clone(), resolved_f);
                                if final_reversion.is_none() {
                                    final_reversion = rev;
                                }
                            } else {
                                merged_fields.insert(
                                    field_name.clone(),
                                    incoming_f_state.clone(),
                                );
                            }
                        }

                        // Check if any merged fields became Consumed and if we should revert
                        if merged_fields
                            .values()
                            .any(|s| matches!(s, EntropicState::Consumed))
                        {
                            if let Some(rev) = on_invalid {
                                return (EntropicState::Consumed, Some(rev.clone()));
                            }
                        }

                        (
                            EntropicState::Valid(Payload::Topology(merged_fields)),
                            final_reversion,
                        )
                    }
                    _ => (EntropicState::Consumed, on_invalid.clone()),
                }
            }
            ResolutionStrategy::Auto => {
                if existing == incoming {
                    (existing.clone(), None)
                } else {
                    (EntropicState::Consumed, None)
                }
            }
            _ => (existing.clone(), None),
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
