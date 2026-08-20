use crate::cfg::BlockId;
use crate::optimize::OptimizationPass;
use crate::ssa::{SsaBasicBlock, SsaCFG, SsaInstruction, SsaTerminator};
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
                eprintln!(
                    "\x1b[1;31m[TVM Middle-End Invariant Error]\x1b[0m Invariant violation in Block: {:?}",
                    err
                );
            }
        }
        false
    }
}

/// Compute ALL successors of an SSA basic block, including both the terminator's
/// control-flow targets and any instruction-embedded CFG edges that the
/// `SsaTransformer` also records (RelativisticBlock, Watchdog, Speculate /
/// EndSpeculate). This must mirror the logic in `SsaTransformer::new()` so
/// that the phi-predecessor validator and the dominator/frontier algorithms
/// agree on the shape of the CFG.
fn get_block_successors(block: &SsaBasicBlock, ssa_cfg: &SsaCFG) -> Vec<BlockId> {
    let mut succs = Vec::new();

    // 1. Terminator successors
    match &block.terminator {
        SsaTerminator::Jump { target } => succs.push(*target),
        SsaTerminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            succs.push(*then_block);
            succs.push(*else_block);
        }
        SsaTerminator::MatchEntropy {
            valid_block,
            decayed_block,
            pending_block,
            consumed_block,
            ..
        } => {
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
        }
        SsaTerminator::Select {
            cases,
            timeout_block,
            ..
        } => {
            for c in cases {
                succs.push(c.target as BlockId);
            }
            if let Some(b) = timeout_block {
                succs.push(*b);
            }
        }
        SsaTerminator::Return { .. } | SsaTerminator::Unreachable => {}
    }

    // 2. Instruction-embedded CFG edges (mirrors SsaTransformer::new)
    for instr in &block.instructions {
        match instr {
            SsaInstruction::RelativisticBlock {
                block_pc,
                block_len,
                ..
            } => {
                if let Some(&body) = ssa_cfg.original_pc_to_block_id.get(block_pc) {
                    if !succs.contains(&body) {
                        succs.push(body);
                    }
                }
                if let Some(&end) =
                    ssa_cfg.original_pc_to_block_id.get(&(block_pc + block_len))
                {
                    if !succs.contains(&end) {
                        succs.push(end);
                    }
                }
            }
            SsaInstruction::Watchdog {
                recovery_jump: Some(pc),
                ..
            } => {
                if let Some(&rb) = ssa_cfg.original_pc_to_block_id.get(pc) {
                    if !succs.contains(&rb) {
                        succs.push(rb);
                    }
                }
            }
            SsaInstruction::Speculate {
                fallback_target, ..
            }
            | SsaInstruction::EndSpeculate {
                fallback_target, ..
            } => {
                if let Some(&fb) =
                    ssa_cfg.original_pc_to_block_id.get(fallback_target)
                {
                    if !succs.contains(&fb) {
                        succs.push(fb);
                    }
                }
            }
            _ => {}
        }
    }

    succs
}

pub fn verify_ssa_cfg(ssa_cfg: &SsaCFG) -> Result<(), Vec<VerificationError>> {
    let mut errors = Vec::new();

    // 1. Build predecessor map for phi node validation.
    //    Uses get_block_successors which includes instruction-embedded edges
    //    (RelativisticBlock, Watchdog, Speculate) so the predecessor set matches
    //    what SsaTransformer used during phi insertion.
    let mut predecessors: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
    for (&id, block) in &ssa_cfg.blocks {
        predecessors.entry(id).or_default();
        for succ in get_block_successors(block, ssa_cfg) {
            predecessors.entry(succ).or_default().insert(id);
        }
    }

    // 2. Validate Phi Nodes: every incoming predecessor must be a *direct* CFG predecessor
    //    of the block. We only report predecessors that have no path at all into the block
    //    via the CFG to avoid false positives from dominator-tree phi filling.
    for (&id, block) in &ssa_cfg.blocks {
        let valid_preds = predecessors.get(&id).cloned().unwrap_or_default();
        for phi in &block.phi_nodes {
            for &(pred_id, _) in &phi.incoming {
                // Only flag if the claimed predecessor has no outgoing edge that reaches
                // this block. A predecessor contributed via dominance-frontier insertion
                // but that has an actual CFG path to the block is not an error.
                if !valid_preds.contains(&pred_id) {
                    let pred_succs = ssa_cfg
                        .blocks
                        .get(&pred_id)
                        .map(|b| get_block_successors(b, ssa_cfg))
                        .unwrap_or_default();
                    if !pred_succs.contains(&id) {
                        errors.push(VerificationError::InvalidPhiPredecessor {
                            block: id,
                            pred: pred_id,
                        });
                    }
                }
            }
        }
    }

    // 3. Verify Temporal Invariants: check matching While/EndWhile and Loop/EndLoop.
    //    Blocks must be visited in sorted (program) order so that depth tracking is
    //    deterministic — HashMap iteration order is arbitrary and causes false positives.
    let mut sorted_block_ids: Vec<BlockId> =
        ssa_cfg.blocks.keys().copied().collect();
    sorted_block_ids.sort();

    let mut while_depth: i32 = 0;
    let mut loop_depth: i32 = 0;

    for id in &sorted_block_ids {
        let block = &ssa_cfg.blocks[id];
        for instr in &block.instructions {
            match instr {
                SsaInstruction::While { .. } => while_depth += 1,
                SsaInstruction::EndWhile { .. } => {
                    if while_depth == 0 {
                        errors.push(VerificationError::UnbalancedTemporalBlock {
                            block: *id,
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
                            block: *id,
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
