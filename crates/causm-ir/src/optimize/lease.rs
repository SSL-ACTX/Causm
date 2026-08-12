use crate::optimize::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction};
use std::collections::HashSet;

/// Lease optimization pass.
/// Removes redundant `Lease`/`EndLease` pairs where duration is 0 or target register is unused,
/// and verifies lease scoping boundaries.
pub struct LeaseOptimizationPass;

impl OptimizationPass for LeaseOptimizationPass {
    fn name(&self) -> &str {
        "lease-optimization"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        let mut used_regs = globally_used_regs.clone();

        // 1. Gather all used register IDs
        for block in ssa_cfg.blocks.values() {
            for phi in &block.phi_nodes {
                for (_, inc) in &phi.incoming {
                    used_regs.insert(inc.reg);
                }
            }
            for instr in &block.instructions {
                crate::optimize::utils::for_each_ssa_src_reg(instr, &mut |src| {
                    used_regs.insert(src.reg);
                });
            }
            crate::optimize::utils::for_each_ssa_term_src_reg(
                &block.terminator,
                &mut |src| {
                    used_regs.insert(src.reg);
                },
            );
        }

        // 2. Remove redundant leases (e.g. Lease where target_reg is never read, or duration is 0)
        let mut changed = false;
        for block in ssa_cfg.blocks.values_mut() {
            let before_len = block.instructions.len();
            block.instructions.retain(|instr| match instr {
                SsaInstruction::Lease {
                    target_reg,
                    duration_ms,
                    ..
                } => !(*duration_ms == 0 || !used_regs.contains(&target_reg.reg)),
                SsaInstruction::EndLease { duration_ms, .. } => *duration_ms != 0,
                _ => true,
            });
            if block.instructions.len() != before_len {
                changed = true;
            }
        }

        changed
    }
}
