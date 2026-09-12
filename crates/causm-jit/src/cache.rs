use causm_ir::ssa::SsaCFG;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug)]
pub struct CompiledRoutine {
    pub code_ptr: *const u8,
    pub param_count: usize,
}

unsafe impl Send for CompiledRoutine {}
unsafe impl Sync for CompiledRoutine {}

static NATIVE_ROUTINE_CACHE: OnceLock<Mutex<HashMap<u64, CompiledRoutine>>> =
    OnceLock::new();

fn get_cache() -> &'static Mutex<HashMap<u64, CompiledRoutine>> {
    NATIVE_ROUTINE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Compute a deterministic 64-bit hash of an SSA CFG.
pub fn hash_ssa_cfg(routine_name: &str, ssa_cfg: &SsaCFG) -> u64 {
    let mut hasher = DefaultHasher::new();
    routine_name.hash(&mut hasher);
    ssa_cfg.entry_block.hash(&mut hasher);
    // Hash number of blocks and their IDs
    let mut block_ids: Vec<_> = ssa_cfg.blocks.keys().copied().collect();
    block_ids.sort();
    for id in block_ids {
        id.hash(&mut hasher);
        let block = &ssa_cfg.blocks[&id];
        block.phi_nodes.len().hash(&mut hasher);
        for phi in &block.phi_nodes {
            phi.dest.reg.hash(&mut hasher);
            phi.dest.version.hash(&mut hasher);
        }
        block.instructions.len().hash(&mut hasher);
        format!("{:?}", block.terminator).hash(&mut hasher);
    }
    hasher.finish()
}

/// Look up a previously compiled native routine by its SSA hash.
pub fn lookup_compiled_routine(hash: u64) -> Option<CompiledRoutine> {
    let cache = get_cache().lock().unwrap();
    cache.get(&hash).copied()
}

/// Cache a newly compiled native routine.
pub fn store_compiled_routine(hash: u64, routine: CompiledRoutine) {
    let mut cache = get_cache().lock().unwrap();
    cache.insert(hash, routine);
}

/// Clear the routine cache.
pub fn clear_routine_cache() {
    let mut cache = get_cache().lock().unwrap();
    cache.clear();
}
