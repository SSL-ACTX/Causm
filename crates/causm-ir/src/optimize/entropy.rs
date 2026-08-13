use super::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator};
use std::collections::HashSet;

pub struct EntropyOptimizationPass;

impl OptimizationPass for EntropyOptimizationPass {
    fn name(&self) -> &str {
        "EntropyOptimization"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        optimize_entropy(ssa_cfg)
    }
}

pub fn optimize_entropy(ssa_cfg: &mut SsaCFG) -> bool {
    let mut changed = false;

    // Track known entropic states of SSA registers within blocks
    let mut known_valid: HashSet<SsaReg> = HashSet::new();
    let mut known_consumed: HashSet<SsaReg> = HashSet::new();

    // Pass 1: Gather all valid definitions
    for block in ssa_cfg.blocks.values() {
        for inst in &block.instructions {
            match inst {
                SsaInstruction::LoadInt { dest, .. }
                | SsaInstruction::LoadFloat { dest, .. }
                | SsaInstruction::LoadBool { dest, .. }
                | SsaInstruction::LoadString { dest, .. }
                | SsaInstruction::LoadNull { dest }
                | SsaInstruction::ConstInt { dest, .. }
                | SsaInstruction::ConstFloat { dest, .. }
                | SsaInstruction::ConstBool { dest, .. }
                | SsaInstruction::ConstString { dest, .. }
                | SsaInstruction::ConstNull { dest }
                | SsaInstruction::BinaryOp { dest, .. }
                | SsaInstruction::UnaryOp { dest, .. }
                | SsaInstruction::StructLit { dest, .. }
                | SsaInstruction::TopologyLit { dest, .. }
                | SsaInstruction::ArrayLit { dest, .. } => {
                    known_valid.insert(*dest);
                }
                _ => {}
            }
        }
    }

    // Pass 2: Track consumptions and structural decay
    for block in ssa_cfg.blocks.values() {
        for inst in &block.instructions {
            match inst {
                SsaInstruction::Consume { src } => {
                    known_consumed.insert(*src);
                }
                SsaInstruction::ConsumeField { src, .. } => {
                    known_consumed.insert(*src);
                }
                SsaInstruction::ConsumeFieldDynamic { target, .. } => {
                    known_consumed.insert(*target);
                }
                SsaInstruction::FieldAccess { target, .. }
                | SsaInstruction::IndexAccess { target, .. } => {
                    known_valid.remove(target);
                }
                SsaInstruction::FieldUpdate { target, .. } => {
                    known_consumed.insert(*target);
                }
                SsaInstruction::IndexFieldUpdate { target, .. } => {
                    known_consumed.insert(*target);
                }
                SsaInstruction::ChanSend { src, .. } => {
                    known_consumed.insert(*src);
                }
                SsaInstruction::For { source, .. } => {
                    known_consumed.insert(*source);
                }
                SsaInstruction::ForStep { source, .. } => {
                    known_consumed.insert(*source);
                }
                SsaInstruction::SplitMap { source, .. } => {
                    known_consumed.insert(*source);
                }
                SsaInstruction::Call { args, .. } => {
                    for arg in args {
                        known_consumed.insert(*arg);
                    }
                }
                SsaInstruction::DynamicCall { args, .. } => {
                    for arg in args {
                        known_consumed.insert(*arg);
                    }
                }
                _ => {}
            }
        }
    }

    for block in ssa_cfg.blocks.values_mut() {
        if let SsaTerminator::MatchEntropy {
            target,
            valid_block,
            decayed_block,
            pending_block,
            consumed_block,
        } = &block.terminator
        {
            // 1. Fold redundant MatchEntropy where all non-None targets point to the same target
            let targets: Vec<crate::cfg::BlockId> = [
                *valid_block,
                *decayed_block,
                *pending_block,
                *consumed_block,
            ]
            .into_iter()
            .flatten()
            .collect();

            if !targets.is_empty() && targets.iter().all(|&t| t == targets[0]) {
                block.terminator = SsaTerminator::Jump { target: targets[0] };
                changed = true;
                continue;
            }

            // 2. Fold known state targets
            if known_valid.contains(target) && !known_consumed.contains(target) {
                if let Some(vt) = valid_block {
                    block.terminator = SsaTerminator::Jump { target: *vt };
                    changed = true;
                    continue;
                }
            } else if known_consumed.contains(target)
                && !known_valid.contains(target)
            {
                if let Some(ct) = consumed_block {
                    block.terminator = SsaTerminator::Jump { target: *ct };
                    changed = true;
                    continue;
                }
            }
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::CFG;
    use crate::Instruction;
    use crate::Reg;

    #[test]
    fn test_redundant_match_entropy_elimination() {
        let instrs = vec![
            Instruction::ConstInt {
                dest: Reg(1),
                value: 42,
            },
            Instruction::MatchEntropy {
                target: Reg(1),
                valid_target: Some(1),
                decayed_target: Some(1),
                pending_target: Some(1),
                consumed_target: Some(1),
            },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = crate::ssa::SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let modified = optimize_entropy(&mut ssa_cfg);
        assert!(modified);

        let entry = ssa_cfg.blocks.get(&ssa_cfg.entry_block).unwrap();
        assert!(matches!(
            entry.terminator,
            SsaTerminator::Jump { target: 1 }
        ));
    }

    #[test]
    fn test_known_valid_match_entropy_folding() {
        let instrs = vec![
            Instruction::ConstInt {
                dest: Reg(1),
                value: 100,
            },
            Instruction::MatchEntropy {
                target: Reg(1),
                valid_target: Some(2),
                decayed_target: Some(3),
                pending_target: None,
                consumed_target: None,
            },
            Instruction::Anchor {
                name: "valid_path".to_string(),
            },
            Instruction::Anchor {
                name: "decayed_path".to_string(),
            },
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = crate::ssa::SsaTransformer::new(cfg);
        let mut ssa_cfg = transformer.transform();

        let modified = optimize_entropy(&mut ssa_cfg);
        assert!(modified);

        let entry = ssa_cfg.blocks.get(&ssa_cfg.entry_block).unwrap();
        // Index 2 ("valid_path") maps to BlockId 1
        assert!(matches!(
            entry.terminator,
            SsaTerminator::Jump { target: 1 }
        ));
    }
}
