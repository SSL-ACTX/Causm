use causm_runtime::vm::Vm;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClockProfile {
    pub global_clock_ms: u64,
    pub root_local_clock_ms: u64,
    pub branch_clocks_ms: std::collections::HashMap<String, u64>,
}

impl ClockProfile {
    pub fn profile_vm(vm: &Vm) -> Self {
        let mut branch_clocks = std::collections::HashMap::new();
        for (name, tl) in &vm.active_branches {
            branch_clocks.insert(name.clone(), tl.local_clock);
        }
        Self {
            global_clock_ms: vm.global_clock,
            root_local_clock_ms: vm.root_timeline.local_clock,
            branch_clocks_ms: branch_clocks,
        }
    }
}
