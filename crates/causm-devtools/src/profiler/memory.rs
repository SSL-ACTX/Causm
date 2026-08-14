use causm_runtime::vm::Vm;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryProfile {
    pub peak_arena_slots: usize,
    pub used_bytes: u64,
    pub capacity_bytes: u64,
    pub active_variables_count: usize,
}

impl MemoryProfile {
    pub fn profile_vm(vm: &Vm) -> Self {
        let arena = &vm.root_timeline.arena;
        let active = arena
            .registers
            .iter()
            .filter(|s| !matches!(s, causm_core::value::EntropicState::Consumed))
            .count();
        Self {
            peak_arena_slots: arena.registers.len(),
            used_bytes: arena.used,
            capacity_bytes: arena.capacity,
            active_variables_count: active,
        }
    }
}
