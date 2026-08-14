// crates/causm-devtools/src/profiler/timeline.rs
use super::clock::ClockProfile;
use super::memory::MemoryProfile;
use causm_runtime::vm::Vm;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimelineProfileReport {
    pub clock: ClockProfile,
    pub memory: MemoryProfile,
}

impl TimelineProfileReport {
    pub fn profile_vm(vm: &Vm) -> Self {
        Self {
            clock: ClockProfile::profile_vm(vm),
            memory: MemoryProfile::profile_vm(vm),
        }
    }
}
