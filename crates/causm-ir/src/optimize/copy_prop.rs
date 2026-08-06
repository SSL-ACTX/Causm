use super::OptimizationPass;
use crate::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator};
use std::collections::{HashMap, HashSet};

pub struct CopyPropagationPass;

impl OptimizationPass for CopyPropagationPass {
    fn name(&self) -> &str {
        "CopyPropagation"
    }

    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        _globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        copy_propagation(ssa_cfg)
    }
}

pub fn copy_propagation(ssa_cfg: &mut SsaCFG) -> bool {
    let mut copies: HashMap<SsaReg, SsaReg> = HashMap::new();

    // 0. Collect all consumed registers
    let mut consumed_regs: HashSet<SsaReg> = HashSet::new();
    for block in ssa_cfg.blocks.values() {
        for instr in &block.instructions {
            match instr {
                SsaInstruction::Consume { src } => {
                    consumed_regs.insert(*src);
                }
                SsaInstruction::ConsumeField { src, .. } => {
                    consumed_regs.insert(*src);
                }
                SsaInstruction::ConsumeFieldDynamic { target, .. } => {
                    consumed_regs.insert(*target);
                }
                _ => {}
            }
        }
    }

    // 1. Collect all direct copies (skipping src if it was consumed)
    for block in ssa_cfg.blocks.values() {
        for instr in &block.instructions {
            if let SsaInstruction::Move { dest, src } = instr {
                if !consumed_regs.contains(src) {
                    copies.insert(*dest, *src);
                }
            }
        }
    }

    // 2. Resolve transitivity (chains of copies)
    let keys: Vec<SsaReg> = copies.keys().cloned().collect();
    for mut key in keys {
        let mut path = HashSet::new();
        path.insert(key);
        while let Some(next) = copies.get(&key) {
            if path.contains(next) {
                // Cycle detected (should not happen in SSA, but safety first)
                break;
            }
            path.insert(*next);
            key = *next;
        }
        for p in path {
            if p != key {
                copies.insert(p, key);
            }
        }
    }

    if copies.is_empty() {
        return false;
    }

    let mut changed = false;

    // Helper to resolve copies
    let resolve = |r: SsaReg| -> SsaReg { copies.get(&r).copied().unwrap_or(r) };

    // 3. Replace all uses
    for block in ssa_cfg.blocks.values_mut() {
        // Phi nodes incoming
        for phi in &mut block.phi_nodes {
            for (_, incoming_reg) in &mut phi.incoming {
                let resolved = resolve(*incoming_reg);
                if *incoming_reg != resolved {
                    *incoming_reg = resolved;
                    changed = true;
                }
            }
        }

        // Instructions
        for instr in &mut block.instructions {
            match instr {
                SsaInstruction::BinaryOp { left, right, .. } => {
                    let new_left = resolve(*left);
                    if *left != new_left {
                        *left = new_left;
                        changed = true;
                    }
                    let new_right = resolve(*right);
                    if *right != new_right {
                        *right = new_right;
                        changed = true;
                    }
                }
                SsaInstruction::UnaryOp { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::Move { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::Clone { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::Call { args, .. } => {
                    for arg in args {
                        let new_arg = resolve(*arg);
                        if *arg != new_arg {
                            *arg = new_arg;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::DynamicCall { args, .. } => {
                    for arg in args {
                        let new_arg = resolve(*arg);
                        if *arg != new_arg {
                            *arg = new_arg;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::TypeAssert { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::TryTypeAssert { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::StructLit { fields, .. } => {
                    for (_, val_reg) in fields {
                        let new_val = resolve(*val_reg);
                        if *val_reg != new_val {
                            *val_reg = new_val;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::TopologyLit { fields, .. } => {
                    for (_, val_reg) in fields {
                        let new_val = resolve(*val_reg);
                        if *val_reg != new_val {
                            *val_reg = new_val;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::ArrayLit { elements, .. } => {
                    for elem in elements {
                        let new_elem = resolve(*elem);
                        if *elem != new_elem {
                            *elem = new_elem;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::FieldAccess { target, .. } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                }
                SsaInstruction::IndexAccess { target, index, .. } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                    let new_index = resolve(*index);
                    if *index != new_index {
                        *index = new_index;
                        changed = true;
                    }
                }
                SsaInstruction::FieldUpdate { target, src, .. } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::IndexFieldUpdate {
                    target, index, src, ..
                } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                    let new_index = resolve(*index);
                    if *index != new_index {
                        *index = new_index;
                        changed = true;
                    }
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::Print { src } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::ChanSend { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::For { source, .. } => {
                    let new_source = resolve(*source);
                    if *source != new_source {
                        *source = new_source;
                        changed = true;
                    }
                }
                SsaInstruction::SplitMap { source, .. } => {
                    let new_source = resolve(*source);
                    if *source != new_source {
                        *source = new_source;
                        changed = true;
                    }
                }
                SsaInstruction::ForStep { source, .. } => {
                    let new_source = resolve(*source);
                    if *source != new_source {
                        *source = new_source;
                        changed = true;
                    }
                }
                _ => {}
            }
        }

        // Terminators
        match &mut block.terminator {
            SsaTerminator::Branch { cond, .. } => {
                let new_cond = resolve(*cond);
                if *cond != new_cond {
                    *cond = new_cond;
                    changed = true;
                }
            }
            SsaTerminator::Return { src } => {
                if let Some(r) = src {
                    let new_r = resolve(*r);
                    if *r != new_r {
                        *r = new_r;
                        changed = true;
                    }
                }
            }
            SsaTerminator::MatchEntropy { target, .. } => {
                let new_target = resolve(*target);
                if *target != new_target {
                    *target = new_target;
                    changed = true;
                }
            }
            SsaTerminator::Select { cases, .. } => {
                for case in cases {
                    let new_dest = resolve(case.dest);
                    if case.dest != new_dest {
                        case.dest = new_dest;
                        changed = true;
                    }
                }
            }
            _ => {}
        }
    }

    changed
}
