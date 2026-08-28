use std::collections::HashMap;
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Value consumed: attempted to read a destructively read value")]
    AlreadyConsumed,
    #[error("Structural decay: attempted to move or send a decayed parent")]
    StructurallyDecayed,
    #[error("Type mismatch: attempted structural access on a non-struct payload")]
    NotAStruct,
    #[error("Memory budget exceeded: {0} bytes required, but only {1} available")]
    OutOfMemory(u64, u64),
    #[error("Clone budget exceeded")]
    CloneBudgetExceeded,
    #[error("Key not found in topology: {0}")]
    KeyNotFound(String),
    #[error("Value is currently leased and cannot be modified or moved until the lease expires")]
    Leased,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPromise {
    pub capability: String,
    pub params: HashMap<String, String>,
    pub requested_at: u64,
    pub ready_at: u64,
    pub deadline_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntropicState {
    Valid(Payload),
    Leased {
        original: Box<EntropicState>,
        expiration_ms: u64,
    },
    Decayed(HashMap<String, EntropicState>),
    Pending(PendingPromise),
    Consumed,
}

impl EntropicState {
    pub fn is_readable(&self) -> bool {
        !matches!(self, EntropicState::Consumed)
    }

    pub fn is_consumed(&self) -> bool {
        matches!(self, EntropicState::Consumed)
    }

    pub fn is_leased(&self) -> bool {
        matches!(self, EntropicState::Leased { .. })
    }

    pub fn can_mutate(&self) -> bool {
        !matches!(self, EntropicState::Consumed | EntropicState::Leased { .. })
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    Integer(i64),
    Float(u64), // Using bits for Eq
    Bool(bool),
    String(String),
    Struct(HashMap<String, EntropicState>),
    Topology(HashMap<String, EntropicState>),
    Array(Vec<Payload>),
    Null,
}

impl std::fmt::Display for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Payload::Integer(i) => write!(f, "{}", i),
            Payload::Float(bits) => write!(f, "{}", f64::from_bits(*bits)),
            Payload::Bool(b) => write!(f, "{}", b),
            Payload::String(s) => write!(f, "{}", s),
            Payload::Struct(fields) => {
                let mut pairs: Vec<String> = Vec::new();
                for (k, v) in fields {
                    let s = match v {
                        EntropicState::Valid(p) => format!("{}: {}", k, p),
                        EntropicState::Decayed(map) => {
                            let fields: Vec<String> = map
                                .iter()
                                .map(|(k2, v2)| match v2 {
                                    EntropicState::Valid(p2) => {
                                        format!("{}: {}", k2, p2)
                                    }
                                    _ => format!("{}: <decayed>", k2),
                                })
                                .collect();
                            format!("{}: {{ {} }}", k, fields.join(", "))
                        }
                        EntropicState::Pending(_) => format!("{}: <pending>", k),
                        EntropicState::Leased { .. } => format!("{}: <leased>", k),
                        EntropicState::Consumed => format!("{}: <consumed>", k),
                    };
                    pairs.push(s);
                }
                write!(f, "struct{{{}}}", pairs.join(", "))
            }
            Payload::Topology(fields) => {
                let mut pairs: Vec<String> = Vec::new();
                for (k, v) in fields {
                    let s = match v {
                        EntropicState::Valid(p) => format!("{}: {}", k, p),
                        EntropicState::Leased { .. } => format!("{}: Leased", k),
                        EntropicState::Decayed(_map) => format!("{}: Decayed", k),
                        EntropicState::Pending(_) => format!("{}: Pending", k),
                        EntropicState::Consumed => format!("{}: Consumed", k),
                    };
                    pairs.push(s);
                }
                write!(f, "topology {{ {} }}", pairs.join(", "))
            }
            Payload::Array(elems) => {
                if elems.len() > 8 {
                    if let Some(first) = elems.first() {
                        if elems.iter().all(|e| e == first) {
                            return write!(f, "[{}; {}]", first, elems.len());
                        }
                    }
                    let head: Vec<String> =
                        elems.iter().take(8).map(|e| format!("{}", e)).collect();
                    write!(f, "[{}, ... ({} items)]", head.join(", "), elems.len())
                } else {
                    let strings: Vec<String> =
                        elems.iter().map(|e| format!("{}", e)).collect();
                    write!(f, "[{}]", strings.join(", "))
                }
            }
            Payload::Null => write!(f, "null"),
        }
    }
}

impl Payload {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Payload::Integer(_) | Payload::Float(_))
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Payload::Integer(i) => Some(*i as f64),
            Payload::Float(bits) => Some(f64::from_bits(*bits)),
            _ => None,
        }
    }

    pub fn render_decay(&self, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        match self {
            Payload::Struct(fields) => {
                let mut s = "struct {".to_string();
                let mut keys: Vec<_> = fields.keys().collect();
                keys.sort();
                for k in keys {
                    s.push_str(&format!(
                        "\n{}  {}: {}",
                        indent,
                        k,
                        fields[k].render_decay(depth + 1)
                    ));
                }
                s.push_str(&format!("\n{}}}", indent));
                s
            }
            Payload::Topology(fields) => {
                let mut s = "topology {".to_string();
                let mut keys: Vec<_> = fields.keys().collect();
                keys.sort();
                for k in keys {
                    s.push_str(&format!(
                        "\n{}  {}: {}",
                        indent,
                        k,
                        fields[k].render_decay(depth + 1)
                    ));
                }
                s.push_str(&format!("\n{}}}", indent));
                s
            }
            _ => format!("{}", self),
        }
    }

    /// Deterministic size calculation for Causm payloads
    pub fn weight(&self) -> u64 {
        match self {
            Payload::Integer(_) => 8,
            Payload::Float(_) => 8,
            Payload::Bool(_) => 1,
            Payload::String(s) => s.len() as u64 + 24, // 24 bytes for String struct overhead
            Payload::Struct(fields) => {
                let fields_weight: u64 = fields.values().map(|s| s.weight()).sum();
                fields_weight + 48 // Overhead for HashMap and metadata
            }
            Payload::Topology(fields) => {
                let fields_weight: u64 = fields.values().map(|s| s.weight()).sum();
                fields_weight + 64 // Higher overhead for topologies
            }
            Payload::Array(elems) => {
                let total: u64 = elems.iter().map(|p| p.weight()).sum();
                total + 24 // Vec overhead
            }
            Payload::Null => 8,
        }
    }
}

impl EntropicState {
    pub fn render_decay(&self, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        match self {
            EntropicState::Valid(p) => {
                format!("\x1b[1;32m[Valid]\x1b[0m {}", p.render_decay(depth))
            }
            EntropicState::Consumed => "\x1b[1;31m[Consumed]\x1b[0m".to_string(),
            EntropicState::Pending(_) => "\x1b[1;34m[Pending]\x1b[0m".to_string(),
            EntropicState::Leased {
                expiration_ms,
                original,
            } => {
                format!(
                    "\x1b[1;35m[Leased until {}ms]\x1b[0m {}",
                    expiration_ms,
                    original.render_decay(depth)
                )
            }
            EntropicState::Decayed(fields) => {
                let mut s = "\x1b[1;33m[Decayed]\x1b[0m {".to_string();
                let mut keys: Vec<_> = fields.keys().collect();
                keys.sort();
                for k in keys {
                    s.push_str(&format!(
                        "\n{}  {}: {}",
                        indent,
                        k,
                        fields[k].render_decay(depth + 1)
                    ));
                }
                s.push_str(&format!("\n{}}}", indent));
                s
            }
        }
    }

    /// Calculate weight of the state including its variant overhead.
    pub fn weight(&self) -> u64 {
        match self {
            EntropicState::Valid(p) => p.weight() + 16,
            EntropicState::Leased { original, .. } => original.weight() + 24,
            EntropicState::Decayed(fields) => {
                let fields_weight: u64 = fields.values().map(|s| s.weight()).sum();
                fields_weight + 32
            }
            EntropicState::Pending(_) => 64,
            EntropicState::Consumed => 8,
        }
    }

    pub fn decay_recursive(self) -> Self {
        match self {
            EntropicState::Valid(Payload::Struct(mut fields)) => {
                for val in fields.values_mut() {
                    let old = std::mem::replace(val, EntropicState::Consumed);
                    *val = old.decay_recursive();
                }
                EntropicState::Decayed(fields)
            }
            EntropicState::Valid(Payload::Topology(mut fields)) => {
                for val in fields.values_mut() {
                    let old = std::mem::replace(val, EntropicState::Consumed);
                    *val = old.decay_recursive();
                }
                EntropicState::Decayed(fields)
            }
            EntropicState::Valid(_) => EntropicState::Decayed(HashMap::new()),
            EntropicState::Leased {
                original,
                expiration_ms,
            } => {
                let decayed_orig = original.decay_recursive();
                EntropicState::Leased {
                    original: Box::new(decayed_orig),
                    expiration_ms,
                }
            }
            EntropicState::Decayed(mut fields) => {
                for val in fields.values_mut() {
                    let old = std::mem::replace(val, EntropicState::Consumed);
                    *val = old.decay_recursive();
                }
                EntropicState::Decayed(fields)
            }
            s => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueMetadata {
    pub instantiated_at: u64,
    pub type_name: Option<String>,
    pub decay_after_ms: Option<u64>,
}

#[derive(Clone)]
pub struct Arena {
    pub capacity: u64,
    pub used: u64,
    pub base_watermark_used: u64,
    pub base_watermark_regs: usize,
    pub registers: Vec<EntropicState>,
    pub metadata: Vec<Option<ValueMetadata>>,
    pub is_persistent_partition: bool,
}

impl Arena {
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            used: 0,
            base_watermark_used: 0,
            base_watermark_regs: 0,
            registers: Vec::new(),
            metadata: Vec::new(),
            is_persistent_partition: false,
        }
    }

    pub fn freeze_base_watermark(&mut self) {
        self.base_watermark_used = self.used;
        self.base_watermark_regs = self.registers.len();
    }

    pub fn reset_to_base_watermark(&mut self) {
        if self.registers.len() > self.base_watermark_regs {
            self.registers.truncate(self.base_watermark_regs);
        }
        if self.metadata.len() > self.base_watermark_regs {
            self.metadata.truncate(self.base_watermark_regs);
        }
        self.used = self.base_watermark_used;
    }

    pub fn remaining(&self) -> u64 {
        self.capacity.saturating_sub(self.used)
    }

    pub fn used_bytes(&self) -> u64 {
        self.used
    }

    pub fn evict_decayed(&mut self) {
        for (i, state) in self.registers.iter_mut().enumerate() {
            if matches!(state, EntropicState::Decayed(_)) {
                let old_weight = state.weight();
                *state = EntropicState::Consumed;
                let new_weight = state.weight();
                self.used = self
                    .used
                    .saturating_sub(old_weight)
                    .saturating_add(new_weight);
                if i < self.metadata.len() {
                    self.metadata[i] = None;
                }
            }
        }
    }

    fn ensure_register(&mut self, reg: u32) {
        let idx = reg as usize;
        if idx >= self.registers.len() {
            self.registers.resize(idx + 1, EntropicState::Consumed);
            self.metadata.resize(idx + 1, None);
        }
    }

    /// Checks and reserves memory before insertion
    pub fn insert(
        &mut self,
        reg: u32,
        state: EntropicState,
    ) -> Result<(), MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;
        let mut potential_used = self.used;

        potential_used = potential_used.saturating_sub(self.registers[idx].weight());

        let state_weight = state.weight();
        if potential_used + state_weight > self.capacity {
            return Err(MemoryError::OutOfMemory(
                state_weight,
                self.capacity.saturating_sub(potential_used),
            ));
        }

        self.used = potential_used + state_weight;
        self.registers[idx] = state;
        self.metadata[idx] = None; // Clear old metadata
        Ok(())
    }

    pub fn insert_with_metadata(
        &mut self,
        reg: u32,
        state: EntropicState,
        meta: ValueMetadata,
    ) -> Result<(), MemoryError> {
        self.insert(reg, state)?;
        self.metadata[reg as usize] = Some(meta);
        Ok(())
    }

    pub fn get_metadata(&self, reg: u32) -> Option<&ValueMetadata> {
        self.metadata.get(reg as usize).and_then(|m| m.as_ref())
    }

    pub fn set_metadata(&mut self, reg: u32, meta: Option<ValueMetadata>) {
        self.ensure_register(reg);
        self.metadata[reg as usize] = meta;
    }

    /// Drop all arena state immediately for deterministic bulk deallocation.
    pub fn clear(&mut self) {
        self.registers.clear();
        self.metadata.clear();
        self.used = 0;
        self.base_watermark_used = 0;
        self.base_watermark_regs = 0;
    }

    /// Optionally compact consumed entries at branch boundaries.
    pub fn compact_consumed(&mut self) {
        // In a register VM, we don't "compact" the Vec as indices must remain stable.
        // We just recalculate used memory.
        let mut new_used = 0;
        for reg in &self.registers {
            new_used += reg.weight();
        }
        self.used = new_used;
    }

    pub fn consume(&mut self, reg: u32) -> Result<Payload, MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;
        let state = &self.registers[idx];
        match state {
            EntropicState::Valid(payload) => {
                let payload = payload.clone();
                let old_weight = state.weight();
                let new_state = EntropicState::Consumed;
                let new_weight = new_state.weight();

                self.used = self
                    .used
                    .saturating_sub(old_weight)
                    .saturating_add(new_weight);
                self.registers[idx] = new_state;
                self.metadata[idx] = None;
                Ok(payload)
            }
            EntropicState::Decayed(_) => Err(MemoryError::StructurallyDecayed),
            EntropicState::Leased { .. } => Err(MemoryError::Leased),
            _ => Err(MemoryError::AlreadyConsumed),
        }
    }

    /// Moves the entropic state out of the arena, replacing it with Consumed.
    pub fn consume_entropic(
        &mut self,
        reg: u32,
    ) -> Result<EntropicState, MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;

        if let EntropicState::Leased { .. } = &self.registers[idx] {
            return Err(MemoryError::Leased);
        }

        let state =
            std::mem::replace(&mut self.registers[idx], EntropicState::Consumed);

        if matches!(state, EntropicState::Consumed) {
            return Err(MemoryError::AlreadyConsumed);
        }

        let old_weight = state.weight();
        let new_state = EntropicState::Consumed;
        let new_weight = new_state.weight();

        self.used = self
            .used
            .saturating_sub(old_weight)
            .saturating_add(new_weight);
        self.metadata[idx] = None;
        Ok(state)
    }

    pub fn peek_field(&self, reg: u32, field: &str) -> Result<Payload, MemoryError> {
        let idx = reg as usize;
        if idx >= self.registers.len() {
            return Err(MemoryError::AlreadyConsumed);
        }
        match &self.registers[idx] {
            EntropicState::Valid(Payload::Struct(fields))
            | EntropicState::Valid(Payload::Topology(fields))
            | EntropicState::Decayed(fields) => match fields.get(field) {
                Some(EntropicState::Valid(p)) => Ok(p.clone()),
                Some(EntropicState::Decayed(_)) => {
                    Err(MemoryError::StructurallyDecayed)
                }
                Some(EntropicState::Leased { original, .. }) => match &**original {
                    EntropicState::Valid(p) => Ok(p.clone()),
                    _ => Err(MemoryError::AlreadyConsumed),
                },
                Some(EntropicState::Consumed) => Err(MemoryError::AlreadyConsumed),
                _ => Err(MemoryError::KeyNotFound(field.to_string())),
            },
            EntropicState::Consumed => Err(MemoryError::AlreadyConsumed),
            EntropicState::Leased { .. } => Err(MemoryError::Leased),
            _ => Err(MemoryError::NotAStruct),
        }
    }

    pub fn consume_field(
        &mut self,
        reg: u32,
        field: &str,
    ) -> Result<Payload, MemoryError> {
        match self.consume_field_entropic(reg, field)? {
            EntropicState::Valid(p) => Ok(p),
            EntropicState::Decayed(_) => Err(MemoryError::StructurallyDecayed),
            EntropicState::Leased { .. } => Err(MemoryError::Leased),
            _ => Err(MemoryError::AlreadyConsumed),
        }
    }

    pub fn consume_field_entropic(
        &mut self,
        reg: u32,
        field: &str,
    ) -> Result<EntropicState, MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;

        if let EntropicState::Leased { .. } = &self.registers[idx] {
            return Err(MemoryError::Leased);
        }

        let state =
            std::mem::replace(&mut self.registers[idx], EntropicState::Consumed);

        if matches!(state, EntropicState::Consumed) {
            return Err(MemoryError::AlreadyConsumed);
        }

        let old_parent_weight = state.weight();

        match state {
            EntropicState::Valid(Payload::Struct(mut fields)) => {
                if !fields.contains_key(field) {
                    self.registers[idx] = EntropicState::Valid(Payload::Struct(fields));
                    return Err(MemoryError::KeyNotFound(field.to_string()));
                }
                let field_state = fields.remove(field).unwrap();
                fields.insert(field.to_string(), EntropicState::Consumed);
                let new_state = EntropicState::Decayed(fields);
                let new_parent_weight = new_state.weight();
                self.used = self
                    .used
                    .saturating_sub(old_parent_weight)
                    .saturating_add(new_parent_weight);
                self.registers[idx] = new_state;
                Ok(field_state)
            }
            EntropicState::Valid(Payload::Topology(mut fields)) => {
                if !fields.contains_key(field) {
                    self.registers[idx] = EntropicState::Valid(Payload::Topology(fields));
                    return Err(MemoryError::KeyNotFound(field.to_string()));
                }
                let field_state = fields.remove(field).unwrap();
                fields.insert(field.to_string(), EntropicState::Consumed);
                let new_state = EntropicState::Decayed(fields);
                let new_parent_weight = new_state.weight();
                self.used = self
                    .used
                    .saturating_sub(old_parent_weight)
                    .saturating_add(new_parent_weight);
                self.registers[idx] = new_state;
                Ok(field_state)
            }
            EntropicState::Decayed(mut fields) => {
                if !fields.contains_key(field) {
                    self.registers[idx] = EntropicState::Decayed(fields);
                    return Err(MemoryError::KeyNotFound(field.to_string()));
                }
                let field_state = fields.remove(field).unwrap();
                fields.insert(field.to_string(), EntropicState::Consumed);
                let new_state = EntropicState::Decayed(fields);
                let new_parent_weight = new_state.weight();
                self.used = self
                    .used
                    .saturating_sub(old_parent_weight)
                    .saturating_add(new_parent_weight);
                self.registers[idx] = new_state;
                Ok(field_state)
            }
            _ => {
                self.registers[idx] = state;
                Err(MemoryError::NotAStruct)
            }
        }
    }

    pub fn peek(&self, reg: u32) -> Option<Payload> {
        let idx = reg as usize;
        if idx >= self.registers.len() {
            return None;
        }
        match &self.registers[idx] {
            EntropicState::Valid(payload) => Some(payload.clone()),
            EntropicState::Decayed(fields) => {
                // Return as a Struct payload; some internal fields may be Consumed
                Some(Payload::Struct(fields.clone()))
            }
            EntropicState::Leased { original, .. } => {
                // Return payload from original state
                match &**original {
                    EntropicState::Valid(p) => Some(p.clone()),
                    EntropicState::Decayed(f) => Some(Payload::Struct(f.clone())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn set_consumed(&mut self, reg: u32) -> Result<(), MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;
        let state = &self.registers[idx];

        if let EntropicState::Leased { .. } = state {
            return Err(MemoryError::Leased);
        }

        let old_weight = state.weight();
        let new_state = EntropicState::Consumed;
        let new_weight = new_state.weight();
        self.used = self
            .used
            .saturating_sub(old_weight)
            .saturating_add(new_weight);
        self.registers[idx] = new_state;
        self.metadata[idx] = None;
        Ok(())
    }

    pub fn decay(&mut self, reg: u32) -> Result<(), MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;

        if let EntropicState::Leased { .. } = &self.registers[idx] {
            return Err(MemoryError::Leased);
        }

        let state =
            std::mem::replace(&mut self.registers[idx], EntropicState::Consumed);
        let old_weight = state.weight();

        let new_state = match state {
            EntropicState::Valid(Payload::Struct(fields)) => {
                EntropicState::Decayed(fields)
            }
            EntropicState::Valid(_) => EntropicState::Consumed,
            EntropicState::Decayed(fields) => EntropicState::Decayed(fields),
            _ => EntropicState::Consumed,
        };

        let new_weight = new_state.weight();
        self.used = self
            .used
            .saturating_sub(old_weight)
            .saturating_add(new_weight);
        self.registers[idx] = new_state;
        Ok(())
    }

    /// Calculates the CPU and Memory overhead for cloning data.
    pub fn calculate_clone_cost(&self, payload: &Payload, depth: u32) -> u64 {
        let base_overhead = 10;
        let c_factor = 2;
        let k_factor = 5;

        base_overhead + (payload.weight() * c_factor) + (depth as u64 * k_factor)
    }

    pub fn update_field(
        &mut self,
        reg: u32,
        field: &str,
        new_value: Payload,
    ) -> Result<(), MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;

        if let EntropicState::Leased { .. } = &self.registers[idx] {
            return Err(MemoryError::Leased);
        }

        let state =
            std::mem::replace(&mut self.registers[idx], EntropicState::Consumed);

        if matches!(state, EntropicState::Consumed) {
            return Err(MemoryError::AlreadyConsumed);
        }

        let old_parent_weight = state.weight();
        let is_topology =
            matches!(state, EntropicState::Valid(Payload::Topology(_)));
        let is_struct = matches!(state, EntropicState::Valid(Payload::Struct(_)));

        match state {
            EntropicState::Valid(Payload::Struct(mut fields))
            | EntropicState::Valid(Payload::Topology(mut fields))
            | EntropicState::Decayed(mut fields) => {
                fields.insert(field.to_string(), EntropicState::Valid(new_value));

                let new_state = if is_struct {
                    EntropicState::Valid(Payload::Struct(fields))
                } else if is_topology {
                    EntropicState::Valid(Payload::Topology(fields))
                } else {
                    EntropicState::Decayed(fields)
                };

                let new_parent_weight = new_state.weight();
                if self.used.saturating_sub(old_parent_weight) + new_parent_weight
                    > self.capacity
                {
                    self.registers[idx] = EntropicState::Consumed;
                    return Err(MemoryError::OutOfMemory(
                        new_parent_weight,
                        self.capacity - (self.used - old_parent_weight),
                    ));
                }

                self.used = self
                    .used
                    .saturating_sub(old_parent_weight)
                    .saturating_add(new_parent_weight);
                self.registers[idx] = new_state;
                Ok(())
            }
            EntropicState::Valid(Payload::Array(mut elems)) => {
                if let Ok(idx_num) = field.parse::<usize>() {
                    if idx_num < elems.len() {
                        elems[idx_num] = new_value;
                        let new_state = EntropicState::Valid(Payload::Array(elems));
                        let new_parent_weight = new_state.weight();
                        if self.used.saturating_sub(old_parent_weight)
                            + new_parent_weight
                            > self.capacity
                        {
                            self.registers[idx] = EntropicState::Consumed;
                            return Err(MemoryError::OutOfMemory(
                                new_parent_weight,
                                self.capacity - (self.used - old_parent_weight),
                            ));
                        }
                        self.used = self
                            .used
                            .saturating_sub(old_parent_weight)
                            .saturating_add(new_parent_weight);
                        self.registers[idx] = new_state;
                        Ok(())
                    } else {
                        self.registers[idx] =
                            EntropicState::Valid(Payload::Array(elems));
                        Err(MemoryError::AlreadyConsumed)
                    }
                } else {
                    self.registers[idx] =
                        EntropicState::Valid(Payload::Array(elems));
                    Err(MemoryError::NotAStruct)
                }
            }
            _ => {
                self.registers[idx] = state;
                Err(MemoryError::NotAStruct)
            }
        }
    }

    pub fn update_index_field(
        &mut self,
        reg: u32,
        index: &str,
        field: &str,
        new_value: Payload,
    ) -> Result<(), MemoryError> {
        self.ensure_register(reg);
        let idx = reg as usize;

        if let EntropicState::Leased { .. } = &self.registers[idx] {
            return Err(MemoryError::Leased);
        }

        let state =
            std::mem::replace(&mut self.registers[idx], EntropicState::Consumed);

        if matches!(state, EntropicState::Consumed) {
            return Err(MemoryError::AlreadyConsumed);
        }

        let old_parent_weight = state.weight();

        match state {
            EntropicState::Valid(Payload::Array(mut elems)) if field.is_empty() => {
                if let Ok(idx_num) = index.parse::<usize>() {
                    if idx_num < elems.len() {
                        elems[idx_num] = new_value;
                        let new_state = EntropicState::Valid(Payload::Array(elems));
                        let new_parent_weight = new_state.weight();
                        if self.used.saturating_sub(old_parent_weight)
                            + new_parent_weight
                            > self.capacity
                        {
                            self.registers[idx] = EntropicState::Consumed;
                            return Err(MemoryError::OutOfMemory(
                                new_parent_weight,
                                self.capacity
                                    - (self.used.saturating_sub(old_parent_weight)),
                            ));
                        }
                        self.used = self
                            .used
                            .saturating_sub(old_parent_weight)
                            .saturating_add(new_parent_weight);
                        self.registers[idx] = new_state;
                        Ok(())
                    } else {
                        self.registers[idx] =
                            EntropicState::Valid(Payload::Array(elems));
                        Err(MemoryError::AlreadyConsumed)
                    }
                } else {
                    self.registers[idx] =
                        EntropicState::Valid(Payload::Array(elems));
                    Err(MemoryError::AlreadyConsumed)
                }
            }
            EntropicState::Valid(Payload::Struct(mut fields))
                if field.is_empty() =>
            {
                fields.insert(index.to_string(), EntropicState::Valid(new_value));
                let new_state = EntropicState::Valid(Payload::Struct(fields));
                let new_parent_weight = new_state.weight();
                if self.used.saturating_sub(old_parent_weight) + new_parent_weight
                    > self.capacity
                {
                    self.registers[idx] = EntropicState::Consumed;
                    return Err(MemoryError::OutOfMemory(
                        new_parent_weight,
                        self.capacity
                            - (self.used.saturating_sub(old_parent_weight)),
                    ));
                }
                self.used = self
                    .used
                    .saturating_sub(old_parent_weight)
                    .saturating_add(new_parent_weight);
                self.registers[idx] = new_state;
                Ok(())
            }
            EntropicState::Valid(Payload::Topology(mut fields)) => {
                if field.is_empty() {
                    fields
                        .insert(index.to_string(), EntropicState::Valid(new_value));
                } else {
                    let inner_state =
                        fields.get_mut(index).ok_or(MemoryError::AlreadyConsumed)?;
                    match inner_state {
                        EntropicState::Valid(Payload::Struct(inner_fields))
                        | EntropicState::Valid(Payload::Topology(inner_fields)) => {
                            inner_fields.insert(
                                field.to_string(),
                                EntropicState::Valid(new_value),
                            );
                        }
                        _ => {
                            self.registers[idx] =
                                EntropicState::Valid(Payload::Topology(fields));
                            return Err(MemoryError::NotAStruct);
                        }
                    }
                }

                let new_state = EntropicState::Valid(Payload::Topology(fields));

                let new_parent_weight = new_state.weight();
                if self.used.saturating_sub(old_parent_weight) + new_parent_weight
                    > self.capacity
                {
                    self.registers[idx] = EntropicState::Consumed;
                    return Err(MemoryError::OutOfMemory(
                        new_parent_weight,
                        self.capacity
                            - (self.used.saturating_sub(old_parent_weight)),
                    ));
                }

                self.used = self
                    .used
                    .saturating_sub(old_parent_weight)
                    .saturating_add(new_parent_weight);
                self.registers[idx] = new_state;
                Ok(())
            }
            _ => {
                self.registers[idx] = state;
                Err(MemoryError::NotAStruct)
            }
        }
    }

    pub fn update_deep_field(
        &mut self,
        reg: u32,
        path: &[String],
        new_value: Payload,
    ) -> Result<(), MemoryError> {
        if path.len() == 1 {
            return self.update_field(reg, &path[0], new_value);
        }

        self.ensure_register(reg);
        let idx = reg as usize;

        if let EntropicState::Leased { .. } = &self.registers[idx] {
            return Err(MemoryError::Leased);
        }

        let state =
            std::mem::replace(&mut self.registers[idx], EntropicState::Consumed);

        if matches!(state, EntropicState::Consumed) {
            return Err(MemoryError::AlreadyConsumed);
        }

        let old_weight = state.weight();
        let is_topology =
            matches!(state, EntropicState::Valid(Payload::Topology(_)));
        let is_struct = matches!(state, EntropicState::Valid(Payload::Struct(_)));

        match state {
            EntropicState::Valid(Payload::Struct(mut fields))
            | EntropicState::Valid(Payload::Topology(mut fields))
            | EntropicState::Decayed(mut fields) => {
                Self::deep_set(&mut fields, path, new_value)?;

                let final_state = if is_struct {
                    EntropicState::Valid(Payload::Struct(fields))
                } else if is_topology {
                    EntropicState::Valid(Payload::Topology(fields))
                } else {
                    EntropicState::Decayed(fields)
                };

                let new_weight = final_state.weight();
                if self.used.saturating_sub(old_weight) + new_weight > self.capacity
                {
                    self.registers[idx] = EntropicState::Consumed;
                    return Err(MemoryError::OutOfMemory(
                        new_weight,
                        self.capacity - (self.used - old_weight),
                    ));
                }

                self.used = self
                    .used
                    .saturating_sub(old_weight)
                    .saturating_add(new_weight);
                self.registers[idx] = final_state;
                Ok(())
            }
            _ => {
                self.registers[idx] = state;
                Err(MemoryError::NotAStruct)
            }
        }
    }

    fn deep_set(
        fields: &mut HashMap<String, EntropicState>,
        path: &[String],
        new_value: Payload,
    ) -> Result<(), MemoryError> {
        if path.is_empty() {
            return Err(MemoryError::KeyNotFound("empty path".to_string()));
        }

        if path.len() == 1 {
            fields.insert(path[0].clone(), EntropicState::Valid(new_value));
            return Ok(());
        }

        let key = &path[0];
        let entry = fields
            .get_mut(key)
            .ok_or(MemoryError::KeyNotFound(key.clone()))?;

        match entry {
            EntropicState::Valid(Payload::Struct(inner))
            | EntropicState::Valid(Payload::Topology(inner))
            | EntropicState::Decayed(inner) => {
                Self::deep_set(inner, &path[1..], new_value)
            }
            _ => Err(MemoryError::NotAStruct),
        }
    }
}
