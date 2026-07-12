use super::OptimizationPass;
use crate::cfg::BlockId;
use crate::ssa::{SsaCFG, SsaInstruction, SsaTerminator};
use std::collections::{HashMap, HashSet};

pub struct BlockCoalescingPass;

impl OptimizationPass for BlockCoalescingPass {
    fn name(&self) -> &str {
        "BlockCoalescing"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        let mut changed = false;

        loop {
            let mut merge_candidate: Option<(BlockId, BlockId)> = None;
            let (preds, succs) = compute_ssa_predecessors_successors(ssa_cfg);

            for &id in ssa_cfg.blocks.keys() {
                if let Some(successors_list) = succs.get(&id) {
                    if successors_list.len() == 1 {
                        let successor_b = successors_list[0];
                        if let Some(predecessors_list) = preds.get(&successor_b) {
                            if predecessors_list.len() == 1
                                && predecessors_list[0] == id
                                && successor_b != ssa_cfg.entry_block
                            {
                                merge_candidate = Some((id, successor_b));
                                break;
                            }
                        }
                    }
                }
            }

            if let Some((id_a, id_b)) = merge_candidate {
                let block_b = ssa_cfg.blocks.remove(&id_b).unwrap();
                let block_a = ssa_cfg.blocks.get_mut(&id_a).unwrap();

                let mut b_instructions = Vec::new();
                for phi in block_b.phi_nodes {
                    if let Some((_, src_reg)) =
                        phi.incoming.iter().find(|(pred, _)| *pred == id_a)
                    {
                        b_instructions.push(SsaInstruction::Move {
                            dest: phi.dest,
                            src: *src_reg,
                        });
                    }
                }
                b_instructions.extend(block_b.instructions);

                block_a.instructions.extend(b_instructions);
                block_a.terminator = block_b.terminator;

                redirect_predecessor(ssa_cfg, id_b, id_a);
                changed = true;
            } else {
                break;
            }
        }

        changed
    }
}

fn compute_ssa_predecessors_successors(
    ssa_cfg: &SsaCFG,
) -> (
    HashMap<BlockId, Vec<BlockId>>,
    HashMap<BlockId, Vec<BlockId>>,
) {
    let mut predecessors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    let mut successors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();

    for &id in ssa_cfg.blocks.keys() {
        predecessors.entry(id).or_default();
        successors.entry(id).or_default();
    }

    for (&id, block) in &ssa_cfg.blocks {
        match &block.terminator {
            SsaTerminator::Jump { target } => {
                successors.entry(id).or_default().push(*target);
                predecessors.entry(*target).or_default().push(id);
            }
            SsaTerminator::Branch {
                then_block,
                else_block,
                ..
            } => {
                successors.entry(id).or_default().push(*then_block);
                successors.entry(id).or_default().push(*else_block);
                predecessors.entry(*then_block).or_default().push(id);
                predecessors.entry(*else_block).or_default().push(id);
            }
            SsaTerminator::MatchEntropy {
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
                ..
            } => {
                let targets = [
                    *valid_block,
                    *decayed_block,
                    *pending_block,
                    *consumed_block,
                ];
                for t in targets.into_iter().flatten() {
                    successors.entry(id).or_default().push(t);
                    predecessors.entry(t).or_default().push(id);
                }
            }
            SsaTerminator::Select {
                cases,
                timeout_block,
                ..
            } => {
                for case in cases {
                    let target_id = case.target as BlockId;
                    successors.entry(id).or_default().push(target_id);
                    predecessors.entry(target_id).or_default().push(id);
                }
                if let Some(t) = timeout_block {
                    successors.entry(id).or_default().push(*t);
                    predecessors.entry(*t).or_default().push(id);
                }
            }
            SsaTerminator::Return { .. } | SsaTerminator::Unreachable => {}
        }
    }

    (predecessors, successors)
}

fn redirect_predecessor(ssa_cfg: &mut SsaCFG, old_pred: BlockId, new_pred: BlockId) {
    for block in ssa_cfg.blocks.values_mut() {
        for phi in &mut block.phi_nodes {
            for (incoming_block, _) in &mut phi.incoming {
                if *incoming_block == old_pred {
                    *incoming_block = new_pred;
                }
            }
        }
    }
}
