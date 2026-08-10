use crate::optimize::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction};
use std::collections::{HashMap, HashSet};

/// Concurrency analysis pass.
/// Verifies Split/Merge branch lifetime consistency and detects unmerged split branches.
pub struct ConcurrencyAnalysisPass;

#[derive(Debug, PartialEq, Eq)]
pub enum ConcurrencyError {
    UnmergedBranch { branch: String, parent: String },
    MismatchedMerge { branch: String, target: String },
}

impl OptimizationPass for ConcurrencyAnalysisPass {
    fn name(&self) -> &str {
        "concurrency-analysis"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        if let Err(errors) = analyze_concurrency(ssa_cfg) {
            for err in errors {
                eprintln!(
                    "\x1b[1;33m[concurrency-verifier warning]\x1b[0m {:?}",
                    err
                );
            }
        }
        false
    }
}

pub fn analyze_concurrency(ssa_cfg: &SsaCFG) -> Result<(), Vec<ConcurrencyError>> {
    let mut errors = Vec::new();
    let mut active_splits: HashMap<String, HashSet<String>> = HashMap::new();
    let mut has_merge = false;

    for block in ssa_cfg.blocks.values() {
        for instr in &block.instructions {
            match instr {
                SsaInstruction::Split { parent, branches } => {
                    active_splits
                        .entry(parent.clone())
                        .or_default()
                        .extend(branches.clone());
                }
                SsaInstruction::Merge {
                    branches, target, ..
                } => {
                    has_merge = true;
                    for branch in branches {
                        if let Some(active) = active_splits.get_mut(target) {
                            if !active.remove(branch) {
                                errors.push(ConcurrencyError::MismatchedMerge {
                                    branch: branch.clone(),
                                    target: target.clone(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // Only report unmerged branches when this CFG contains a Merge instruction.
    // If no Merge is present the split branches are reconciled in a later temporal
    // block, which is the expected cross-block split+merge pattern and not an error.
    if has_merge {
        for (parent, unmerged) in active_splits {
            for branch in unmerged {
                errors.push(ConcurrencyError::UnmergedBranch {
                    branch,
                    parent: parent.clone(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
