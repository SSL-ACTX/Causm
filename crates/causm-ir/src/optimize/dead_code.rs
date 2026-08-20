// crates/causm-ir/src/optimize/dead_code.rs

use super::utils::{
    for_each_ssa_src_reg, for_each_ssa_term_src_reg, get_ssa_dest_reg,
    has_side_effects,
};
use super::OptimizationPass;
use crate::ssa::{SsaCFG, SsaReg};
use crate::{Instruction, IrProgram};
use std::collections::{HashMap, HashSet};

/// For emit/dump purposes only: strip unnamespaced routines that are import-
/// internal duplicates of a namespaced alias, remap call sites to the namespaced
/// alias, and strip empty foreign declaration stubs.
///
/// This must NOT be called at analysis or execution time — only before emit.
pub fn prune_import_duplicates(ir: &mut IrProgram) {
    // 1. Build a mapping from unnamespaced names to namespaced routine aliases.
    let mut alias_map: HashMap<String, String> = HashMap::new();
    for key in ir.routines.keys() {
        if let Some(dot_idx) = key.find('.') {
            let unnamespaced = key[dot_idx + 1..].to_string();

            let is_target_non_empty = ir
                .routines
                .get(key)
                .map(|r| !r.instructions.is_empty())
                .unwrap_or(false);

            let is_unnamespaced_ffi = ir
                .routines
                .get(&unnamespaced)
                .map(|r| r.instructions.is_empty())
                .unwrap_or(false);

            if is_target_non_empty
                && !is_unnamespaced_ffi
                && (ir.routines.contains_key(&unnamespaced)
                    || unnamespaced.contains('.'))
            {
                alias_map.insert(unnamespaced, key.clone());
            }
        }
    }

    // 2. Remap all Call instructions in root timeline blocks and routines to use namespaced aliases.
    let remap_call = |instr: &mut Instruction| {
        if let Instruction::Call { routine, .. } = instr {
            if let Some(namespaced) = alias_map.get(routine) {
                *routine = namespaced.clone();
            }
        }
    };

    for block in &mut ir.blocks {
        for instr in &mut block.instructions {
            remap_call(instr);
        }
    }

    for routine in ir.routines.values_mut() {
        for instr in &mut routine.instructions {
            remap_call(instr);
        }
    }

    // 3. Remove unnamespaced duplicates from ir.routines whose namespaced equivalent exists.
    for unnamespaced in alias_map.keys() {
        ir.routines.remove(unnamespaced);
    }

    // 4. Strip empty routines/stubs with no instructions so they do not produce empty headers in dump.
    ir.routines
        .retain(|_, routine| !routine.instructions.is_empty());
}

/// Prune uncalled / unreachable routines (tree shaking) from the IR program.
/// Traverses all Call and DynamicCall sites starting from the root timeline blocks.
pub fn prune_unreachable_routines(ir: &mut IrProgram) {
    if ir.blocks.is_empty() {
        return;
    }

    let mut reachable: HashSet<String> = HashSet::new();
    let mut worklist: Vec<String> = Vec::new();
    let mut dynamic_methods: HashSet<String> = HashSet::new();

    let check_instr = |instr: &Instruction,
                       worklist: &mut Vec<String>,
                       reachable: &mut HashSet<String>,
                       dynamic_methods: &mut HashSet<String>| {
        match instr {
            Instruction::Call { routine, .. } => {
                if reachable.insert(routine.clone()) {
                    worklist.push(routine.clone());
                }
            }
            Instruction::DynamicCall { method, .. } => {
                dynamic_methods.insert(method.clone());
            }
            _ => {}
        }
    };

    // 1. Seed reachability from all root timeline execution blocks
    for block in &ir.blocks {
        for instr in &block.instructions {
            check_instr(instr, &mut worklist, &mut reachable, &mut dynamic_methods);
        }
    }

    // 2. If dynamic calls exist, include all candidate matching methods
    if !dynamic_methods.is_empty() {
        for name in ir.routines.keys() {
            for method in &dynamic_methods {
                if (name == method || name.ends_with(&format!(".{}", method)))
                    && reachable.insert(name.clone())
                {
                    worklist.push(name.clone());
                }
            }
        }
    }

    // 3. Transitively follow all Call instructions in reachable routines
    while let Some(current_name) = worklist.pop() {
        if let Some(routine) = ir.routines.get(&current_name) {
            for instr in &routine.instructions {
                check_instr(
                    instr,
                    &mut worklist,
                    &mut reachable,
                    &mut dynamic_methods,
                );
            }
        }
    }

    // 4. Retain only reachable routines
    ir.routines.retain(|name, _| reachable.contains(name));
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
            // Prune dead phi nodes
            let mut p = 0;
            while p < block.phi_nodes.len() {
                let phi = &block.phi_nodes[p];
                let dest = phi.dest;
                let mut is_used = use_counts.get(&dest).copied().unwrap_or(0) > 0;
                if !is_routine && globally_used_regs.contains(&dest.reg) {
                    is_used = true;
                }
                if !is_used {
                    for (_, incoming_reg) in &phi.incoming {
                        if let Some(count) = use_counts.get_mut(incoming_reg) {
                            if *count > 0 {
                                *count -= 1;
                            }
                        }
                    }
                    block.phi_nodes.remove(p);
                    changed = true;
                    any_changed = true;
                    continue;
                }
                p += 1;
            }

            // Prune dead instructions
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
