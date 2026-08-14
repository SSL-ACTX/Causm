use super::utils::{
    for_each_ssa_src_reg, for_each_ssa_term_src_reg, get_ssa_dest_reg,
    has_side_effects,
};
use super::OptimizationPass;
use crate::ssa::{SsaCFG, SsaReg};
use crate::{Instruction, IrProgram};
use std::collections::{HashMap, HashSet};

/// For emit/dump purposes only: strip unnamespaced routines that are import-
/// internal duplicates of a namespaced alias and are never called directly from
/// any IR block (i.e. they exist only for intra-module call resolution).
///
/// Example: `import "std/net" as Net` emits both `socket` (raw FFI binding,
/// needed by `create_socket` internally) and `Net.socket` (the public alias).
/// If the root timeline never calls `socket` directly, we strip it from the
/// dump so the output shows only the clean `Net.*` surface.
///
/// This must NOT be called at analysis or execution time — only before emit.
pub fn prune_import_duplicates(ir: &mut IrProgram) {
    // Collect all namespaced names: e.g. {"Net.socket", "Net.tcp_bind", ...}
    let namespaced: HashSet<&str> = ir
        .routines
        .keys()
        .filter(|k| k.contains('.'))
        .map(|k| k.as_str())
        .collect();

    // Build a set of all Call targets inside IR blocks (the root timeline).
    let mut root_callees: HashSet<String> = HashSet::new();
    for block in &ir.blocks {
        for instr in &block.instructions {
            if let Instruction::Call { routine, .. } = instr {
                root_callees.insert(routine.clone());
            }
        }
    }

    // Also collect calls made by *namespaced* routines — those are intra-module
    // and should not prevent removal of the unqualified name from the dump.
    // (We only care about root-level callers here.)

    // Find unqualified names that:
    //   1. Have NO dot in their name (they are the raw unqualified binding).
    //   2. Have at least one namespaced counterpart `Ns.name` in the IR.
    //   3. Are NOT directly called from any root IR block.
    let to_remove: Vec<String> = ir
        .routines
        .keys()
        .filter(|name| {
            // must be unqualified
            if name.contains('.') {
                return false;
            }
            // there must be at least one Ns.<name> variant
            let has_ns_alias = namespaced
                .iter()
                .any(|ns| ns.ends_with(&format!(".{}", name)));
            if !has_ns_alias {
                return false;
            }
            // must not be directly called from root blocks
            !root_callees.contains(*name)
        })
        .cloned()
        .collect();

    for name in to_remove {
        ir.routines.remove(&name);
    }
}

pub struct DeadCodeEliminationPass;

impl OptimizationPass for DeadCodeEliminationPass {
    fn name(&self) -> &str {
        "DeadCodeElimination"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        globally_used_regs: &HashSet<u32>,
        is_routine: bool,
    ) -> bool {
        dead_code_elimination(ssa_cfg, globally_used_regs, is_routine)
    }
}

pub fn dead_code_elimination(
    ssa_cfg: &mut SsaCFG,
    globally_used_regs: &HashSet<u32>,
    is_routine: bool,
) -> bool {
    let mut use_counts: HashMap<SsaReg, usize> = HashMap::new();
    let mut count_uses = |r: SsaReg| {
        *use_counts.entry(r).or_insert(0) += 1;
    };

    for block in ssa_cfg.blocks.values() {
        for phi in &block.phi_nodes {
            for (_, incoming_reg) in &phi.incoming {
                count_uses(*incoming_reg);
            }
        }
        for instr in &block.instructions {
            for_each_ssa_src_reg(instr, &mut count_uses);
        }
        for_each_ssa_term_src_reg(&block.terminator, &mut count_uses);
    }

    let mut changed = true;
    let mut any_changed = false;
    while changed {
        changed = false;
        for block in ssa_cfg.blocks.values_mut() {
            let mut i = 0;
            while i < block.instructions.len() {
                let instr = &block.instructions[i];
                if let Some(dest) = get_ssa_dest_reg(instr) {
                    let mut is_used =
                        use_counts.get(&dest).copied().unwrap_or(0) > 0;
                    if !is_routine && globally_used_regs.contains(&dest.reg) {
                        is_used = true;
                    }
                    if !is_used && !has_side_effects(instr) {
                        for_each_ssa_src_reg(instr, &mut |r| {
                            if let Some(count) = use_counts.get_mut(&r) {
                                if *count > 0 {
                                    *count -= 1;
                                }
                            }
                        });
                        block.instructions.remove(i);
                        changed = true;
                        any_changed = true;
                        continue;
                    }
                }
                i += 1;
            }
        }
    }
    any_changed
}
