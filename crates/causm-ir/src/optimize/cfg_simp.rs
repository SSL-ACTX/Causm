use super::OptimizationPass;
use crate::cfg::BlockId;
use crate::ssa::{SsaCFG, SsaInstruction, SsaTerminator};
use std::collections::{HashMap, HashSet, VecDeque};

pub struct CfgSimplificationPass;

impl OptimizationPass for CfgSimplificationPass {
    fn name(&self) -> &str {
        "CfgSimplification"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        let mut changed = false;

        let mut local_changed = true;
        while local_changed {
            local_changed = false;

            // 1. Dead block elimination (unreachable blocks)
            if self.eliminate_unreachable_blocks(ssa_cfg) {
                local_changed = true;
                changed = true;
            }

            // 2. Fold redundant branch conds / identical targets
            if self.fold_redundant_branches(ssa_cfg) {
                local_changed = true;
                changed = true;
            }

            // 3. Bypass empty blocks that only jump
            if self.bypass_empty_jump_blocks(ssa_cfg) {
                local_changed = true;
                changed = true;
            }
        }

        changed
    }
}

impl CfgSimplificationPass {
    fn get_preserved_blocks(&self, ssa_cfg: &SsaCFG) -> HashSet<BlockId> {
        let mut preserved = HashSet::new();
        preserved.insert(ssa_cfg.entry_block);

        for block in ssa_cfg.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    SsaInstruction::RelativisticBlock {
                        block_pc,
                        block_len,
                        ..
                    } => {
                        if let Some(&b) =
                            ssa_cfg.original_pc_to_block_id.get(block_pc)
                        {
                            preserved.insert(b);
                        }
                        if let Some(&b) = ssa_cfg
                            .original_pc_to_block_id
                            .get(&(block_pc + block_len))
                        {
                            preserved.insert(b);
                        }
                    }
                    SsaInstruction::Watchdog { recovery_jump, .. } => {
                        if let Some(pc) = recovery_jump {
                            if let Some(&b) = ssa_cfg.original_pc_to_block_id.get(pc)
                            {
                                preserved.insert(b);
                            }
                        }
                    }
                    SsaInstruction::Speculate {
                        fallback_target, ..
                    }
                    | SsaInstruction::EndSpeculate {
                        fallback_target, ..
                    } => {
                        if let Some(&b) =
                            ssa_cfg.original_pc_to_block_id.get(fallback_target)
                        {
                            preserved.insert(b);
                        }
                    }
                    _ => {}
                }
            }
        }
        preserved
    }

    fn eliminate_unreachable_blocks(&self, ssa_cfg: &mut SsaCFG) -> bool {
        let preserved = self.get_preserved_blocks(ssa_cfg);
        let mut reachable = HashSet::new();
        let mut queue = VecDeque::new();

        for &b in &preserved {
            reachable.insert(b);
            queue.push_back(b);
        }

        let (_, succs) = compute_predecessors_successors(ssa_cfg);

        while let Some(curr) = queue.pop_front() {
            if let Some(successors_list) = succs.get(&curr) {
                for &succ in successors_list {
                    if reachable.insert(succ) {
                        queue.push_back(succ);
                    }
                }
            }
        }

        let all_blocks: Vec<BlockId> = ssa_cfg.blocks.keys().cloned().collect();
        let mut removed = false;
        for id in all_blocks {
            if !reachable.contains(&id) {
                ssa_cfg.blocks.remove(&id);
                if let Some(successors_list) = succs.get(&id) {
                    for &succ in successors_list {
                        if let Some(succ_block) = ssa_cfg.blocks.get_mut(&succ) {
                            for phi in &mut succ_block.phi_nodes {
                                phi.incoming.retain(|(pred, _)| *pred != id);
                            }
                        }
                    }
                }
                removed = true;
            }
        }
        removed
    }

    fn fold_redundant_branches(&self, ssa_cfg: &mut SsaCFG) -> bool {
        let mut changed = false;

        let mut const_bools = HashMap::new();
        for block in ssa_cfg.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    SsaInstruction::ConstBool { dest, value }
                    | SsaInstruction::LoadBool { dest, value } => {
                        const_bools.insert(*dest, *value);
                    }
                    _ => {}
                }
            }
        }

        let ids: Vec<BlockId> = ssa_cfg.blocks.keys().cloned().collect();
        for id in ids {
            let mut folded_to_jump = None;
            if let Some(block) = ssa_cfg.blocks.get(&id) {
                match &block.terminator {
                    SsaTerminator::Branch {
                        cond,
                        then_block,
                        else_block,
                    } => {
                        if then_block == else_block {
                            folded_to_jump = Some((*then_block, None));
                        } else if let Some(val) = const_bools.get(cond) {
                            let target =
                                if *val { *then_block } else { *else_block };
                            let dead_branch =
                                if *val { *else_block } else { *then_block };
                            folded_to_jump = Some((target, Some(dead_branch)));
                        }
                    }
                    _ => {}
                }
            }

            if let Some((target, dead_branch_opt)) = folded_to_jump {
                let block = ssa_cfg.blocks.get_mut(&id).unwrap();
                block.terminator = SsaTerminator::Jump { target };

                if let Some(dead_block) = dead_branch_opt {
                    if let Some(succ_block) = ssa_cfg.blocks.get_mut(&dead_block) {
                        for phi in &mut succ_block.phi_nodes {
                            phi.incoming.retain(|(pred, _)| *pred != id);
                        }
                    }
                }
                changed = true;
            }
        }

        changed
    }

    fn bypass_empty_jump_blocks(&self, ssa_cfg: &mut SsaCFG) -> bool {
        let mut changed = false;
        let preserved = self.get_preserved_blocks(ssa_cfg);

        let ids: Vec<BlockId> = ssa_cfg.blocks.keys().cloned().collect();
        for id in ids {
            if id == ssa_cfg.entry_block {
                continue;
            }

            let is_empty_jump = {
                if let Some(block) = ssa_cfg.blocks.get(&id) {
                    !preserved.contains(&id)
                        && block.instructions.is_empty()
                        && block.phi_nodes.is_empty()
                        && matches!(block.terminator, SsaTerminator::Jump { .. })
                } else {
                    false
                }
            };

            if is_empty_jump {
                let target = match ssa_cfg.blocks.get(&id).unwrap().terminator {
                    SsaTerminator::Jump { target } => target,
                    _ => unreachable!(),
                };

                if id == target {
                    continue;
                }

                let (preds, _) = compute_predecessors_successors(ssa_cfg);
                let predecessors = preds.get(&id).cloned().unwrap_or_default();

                if predecessors.is_empty() {
                    continue;
                }

                let target_preds = preds.get(&target).cloned().unwrap_or_default();
                let has_duplicate_edge =
                    predecessors.iter().any(|p| target_preds.contains(p));
                if has_duplicate_edge {
                    continue;
                }

                for &pred_id in &predecessors {
                    let pred_block = ssa_cfg.blocks.get_mut(&pred_id).unwrap();
                    match &mut pred_block.terminator {
                        SsaTerminator::Jump { target: t } => {
                            if *t == id {
                                *t = target;
                            }
                        }
                        SsaTerminator::Branch {
                            then_block,
                            else_block,
                            ..
                        } => {
                            if *then_block == id {
                                *then_block = target;
                            }
                            if *else_block == id {
                                *else_block = target;
                            }
                        }
                        SsaTerminator::MatchEntropy {
                            valid_block,
                            decayed_block,
                            pending_block,
                            consumed_block,
                            ..
                        } => {
                            if let Some(b) = valid_block {
                                if *b == id {
                                    *b = target;
                                }
                            }
                            if let Some(b) = decayed_block {
                                if *b == id {
                                    *b = target;
                                }
                            }
                            if let Some(b) = pending_block {
                                if *b == id {
                                    *b = target;
                                }
                            }
                            if let Some(b) = consumed_block {
                                if *b == id {
                                    *b = target;
                                }
                            }
                        }
                        SsaTerminator::Select {
                            cases,
                            timeout_block,
                            ..
                        } => {
                            for case in cases {
                                if case.target == id as usize {
                                    case.target = target as usize;
                                }
                            }
                            if let Some(b) = timeout_block {
                                if *b == id {
                                    *b = target;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(target_block) = ssa_cfg.blocks.get_mut(&target) {
                    for phi in &mut target_block.phi_nodes {
                        let mut val_opt = None;
                        for &(incoming_pred, reg) in &phi.incoming {
                            if incoming_pred == id {
                                val_opt = Some(reg);
                                break;
                            }
                        }
                        if let Some(val) = val_opt {
                            phi.incoming.retain(|(p, _)| *p != id);
                            for &pred_id in &predecessors {
                                phi.incoming.push((pred_id, val));
                            }
                        }
                    }
                }

                ssa_cfg.blocks.remove(&id);
                changed = true;
                break;
            }
        }

        changed
    }
}

fn compute_predecessors_successors(
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
                if let Some(t) = valid_block {
                    successors.entry(id).or_default().push(*t);
                    predecessors.entry(*t).or_default().push(id);
                }
                if let Some(t) = decayed_block {
                    successors.entry(id).or_default().push(*t);
                    predecessors.entry(*t).or_default().push(id);
                }
                if let Some(t) = pending_block {
                    successors.entry(id).or_default().push(*t);
                    predecessors.entry(*t).or_default().push(id);
                }
                if let Some(t) = consumed_block {
                    successors.entry(id).or_default().push(*t);
                    predecessors.entry(*t).or_default().push(id);
                }
            }
            SsaTerminator::Select {
                cases,
                timeout_block,
                ..
            } => {
                for case in cases {
                    successors
                        .entry(id)
                        .or_default()
                        .push(case.target as BlockId);
                    predecessors
                        .entry(case.target as BlockId)
                        .or_default()
                        .push(id);
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
