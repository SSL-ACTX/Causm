use crate::cfg::BlockId;
use crate::optimize::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction, SsaTerminator};
use std::collections::{HashMap, HashSet};

/// Verification pass for SSA invariants and temporal rules.
pub struct VerifierPass;

#[derive(Debug, PartialEq, Eq)]
pub enum VerificationError {
    InvalidDominance { block: BlockId, reg: u32 },
    InvalidPhiPredecessor { block: BlockId, pred: BlockId },
    UnbalancedTemporalBlock { block: BlockId, reason: String },
}

impl OptimizationPass for VerifierPass {
    fn name(&self) -> &str {
        "verifier"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        if let Err(errors) = verify_ssa_cfg(ssa_cfg) {
            for err in errors {
                eprintln!("\x1b[1;33m[ssa-verifier warning]\x1b[0m {:?}", err);
            }
        }
        false
    }
}

fn get_ssa_terminator_successors(term: &SsaTerminator) -> Vec<BlockId> {
    match term {
        SsaTerminator::Jump { target } => vec![*target],
        SsaTerminator::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        SsaTerminator::MatchEntropy {
            valid_block,
            decayed_block,
            pending_block,
            consumed_block,
            ..
        } => {
            let mut succs = Vec::new();
            if let Some(b) = valid_block {
                succs.push(*b);
            }
            if let Some(b) = decayed_block {
                succs.push(*b);
            }
            if let Some(b) = pending_block {
                succs.push(*b);
            }
            if let Some(b) = consumed_block {
                succs.push(*b);
            }
            succs
        }
        SsaTerminator::Select {
            cases,
            timeout_block,
            ..
        } => {
            let mut succs: Vec<BlockId> =
                cases.iter().map(|c| c.target as BlockId).collect();
            if let Some(b) = timeout_block {
                succs.push(*b);
            }
            succs
        }
        SsaTerminator::Return { .. } | SsaTerminator::Unreachable => Vec::new(),
    }
}

pub fn verify_ssa_cfg(ssa_cfg: &SsaCFG) -> Result<(), Vec<VerificationError>> {
    let mut errors = Vec::new();

    // 1. Build predecessor map for phi node validation
    let mut predecessors: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
    for (&id, block) in &ssa_cfg.blocks {
        predecessors.entry(id).or_default();
        for succ in get_ssa_terminator_successors(&block.terminator) {
            predecessors.entry(succ).or_default().insert(id);
        }
    }

    // 2. Validate Phi Nodes: incoming predecessors must be valid predecessors of the block
    for (&id, block) in &ssa_cfg.blocks {
        let valid_preds = predecessors.get(&id).cloned().unwrap_or_default();
        for phi in &block.phi_nodes {
            for &(pred_id, _) in &phi.incoming {
                if !valid_preds.contains(&pred_id) {
                    errors.push(VerificationError::InvalidPhiPredecessor {
                        block: id,
                        pred: pred_id,
                    });
                }
            }
        }
    }

    // 3. Verify Temporal Invariants: check matching boundaries for While/EndWhile, Loop/EndLoop
    let mut while_depth = 0;
    let mut loop_depth = 0;

    for (&id, block) in &ssa_cfg.blocks {
        for instr in &block.instructions {
            match instr {
                SsaInstruction::While { .. } => while_depth += 1,
                SsaInstruction::EndWhile { .. } => {
                    if while_depth == 0 {
                        errors.push(VerificationError::UnbalancedTemporalBlock {
                            block: id,
                            reason: "EndWhile without matching While".to_string(),
                        });
                    } else {
                        while_depth -= 1;
                    }
                }
                SsaInstruction::Loop { .. } => loop_depth += 1,
                SsaInstruction::EndLoop { .. } => {
                    if loop_depth == 0 {
                        errors.push(VerificationError::UnbalancedTemporalBlock {
                            block: id,
                            reason: "EndLoop without matching Loop".to_string(),
                        });
                    } else {
                        loop_depth -= 1;
                    }
                }
                _ => {}
            }
        }
    }

    if while_depth != 0 {
        errors.push(VerificationError::UnbalancedTemporalBlock {
            block: ssa_cfg.entry_block,
            reason: format!("Unclosed While construct (depth: {})", while_depth),
        });
    }
    if loop_depth != 0 {
        errors.push(VerificationError::UnbalancedTemporalBlock {
            block: ssa_cfg.entry_block,
            reason: format!("Unclosed Loop construct (depth: {})", loop_depth),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
