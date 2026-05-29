pub use causm_jit::{hw_timing, pin_memory, Jit, JitError};

use crate::vm::error::TemporalError;
use crate::vm::state::Vm;
use causm_frontend::ir::IrRoutine;
use std::collections::HashMap;

use causm_core::value::{EntropicState, Payload};

pub extern "C" fn temporal_freeze_callback(vm_ptr: *mut Vm, cycles: u64) {
    let vm = unsafe { &mut *vm_ptr };
    // Convert cycles to ms roughly (assuming 3GHz)
    let ms = cycles / 3_000_000;
    vm.temporal_freeze(ms.max(1));
}

pub extern "C" fn arena_consume_int_callback(
    timeline_ptr: *mut crate::vm::state::Timeline,
    reg: u32,
) -> i64 {
    let timeline = unsafe { &mut *timeline_ptr };
    match timeline.arena.consume_entropic(reg) {
        Ok(EntropicState::Valid(Payload::Integer(i))) => i,
        _ => 0,
    }
}

pub fn init_jit() -> Jit {
    let mut symbols: HashMap<String, *const u8> = HashMap::new();
    symbols.insert(
        "temporal_freeze".to_string(),
        temporal_freeze_callback as *const u8,
    );
    symbols.insert(
        "arena_consume_int".to_string(),
        arena_consume_int_callback as *const u8,
    );
    Jit::new(symbols)
}

impl From<JitError> for TemporalError {
    fn from(err: JitError) -> Self {
        TemporalError::CapabilityViolation(err.to_string())
    }
}

pub trait JitExt {
    fn compile_routine_wrapped(
        &mut self,
        name: &str,
        routine: &IrRoutine,
    ) -> Result<*const u8, TemporalError>;
}

impl JitExt for Jit {
    fn compile_routine_wrapped(
        &mut self,
        name: &str,
        routine: &IrRoutine,
    ) -> Result<*const u8, TemporalError> {
        self.compile_routine(name, routine).map_err(|e| e.into())
    }
}
