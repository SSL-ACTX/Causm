use super::utils::{
    for_each_ssa_src_reg, for_each_ssa_term_src_reg, get_ssa_dest_reg,
    has_side_effects,
};
use super::OptimizationPass;
use crate::ssa::{SsaCFG, SsaReg};
use std::collections::{HashMap, HashSet};

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
