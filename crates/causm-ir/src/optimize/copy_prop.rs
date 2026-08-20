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
        globally_used_regs: &HashSet<u32>,
        _is_routine: bool,
    ) -> bool {
        copy_propagation(ssa_cfg, globally_used_regs)
    }
}

use crate::properties::SsaInstructionProperties;

pub fn copy_propagation(
    ssa_cfg: &mut SsaCFG,
    globally_used_regs: &HashSet<u32>,
) -> bool {
    let mut copies: HashMap<SsaReg, SsaReg> = HashMap::new();

    // 0. Collect all consumed registers
    let mut consumed_regs: HashSet<SsaReg> = HashSet::new();
    for block in ssa_cfg.blocks.values() {
        for instr in &block.instructions {
            for c in instr.consumed_ssa_regs() {
                consumed_regs.insert(c);
            }
        }
    }

    // 1. Collect all direct copies (skipping src if it was consumed, and dest if it is a preserved register)
    for block in ssa_cfg.blocks.values() {
        for instr in &block.instructions {
            if let SsaInstruction::Move { dest, src } = instr {
                if !consumed_regs.contains(src)
                    && !globally_used_regs.contains(&dest.reg)
                {
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
                SsaInstruction::ConditionalSelect {
                    cond,
                    true_val,
                    false_val,
                    ..
                } => {
                    let new_cond = resolve(*cond);
                    if *cond != new_cond {
                        *cond = new_cond;
                        changed = true;
                    }
                    let new_true = resolve(*true_val);
                    if *true_val != new_true {
                        *true_val = new_true;
                        changed = true;
                    }
                    let new_false = resolve(*false_val);
                    if *false_val != new_false {
                        *false_val = new_false;
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
                SsaInstruction::Clone { src, .. }
                | SsaInstruction::StrBytes { src, .. }
                | SsaInstruction::ToStr { src, .. }
                | SsaInstruction::ArrayLen { src, .. } => {
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
                SsaInstruction::TypeCast { src, .. } => {
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
                    for val_reg in fields.values_mut() {
                        let new_val = resolve(*val_reg);
                        if *val_reg != new_val {
                            *val_reg = new_val;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::TopologyLit { fields, .. } => {
                    for val_reg in fields.values_mut() {
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
                SsaInstruction::ArrayRepeat { value, count, .. } => {
                    let new_val = resolve(*value);
                    if *value != new_val {
                        *value = new_val;
                        changed = true;
                    }
                    let new_count = resolve(*count);
                    if *count != new_count {
                        *count = new_count;
                        changed = true;
                    }
                }
                SsaInstruction::ArraySlice {
                    target, start, end, ..
                } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                    if let Some(s) = start {
                        let new_s = resolve(*s);
                        if *s != new_s {
                            *s = new_s;
                            changed = true;
                        }
                    }
                    if let Some(e) = end {
                        let new_e = resolve(*e);
                        if *e != new_e {
                            *e = new_e;
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
                SsaInstruction::Await { target } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                }
                SsaInstruction::MatchEntropy { target, .. } => {
                    let new_target = resolve(*target);
                    if *target != new_target {
                        *target = new_target;
                        changed = true;
                    }
                }
                SsaInstruction::Lease {
                    target_reg,
                    source_reg,
                    ..
                } => {
                    let new_target = resolve(*target_reg);
                    if *target_reg != new_target {
                        *target_reg = new_target;
                        changed = true;
                    }
                    let new_source = resolve(*source_reg);
                    if *source_reg != new_source {
                        *source_reg = new_source;
                        changed = true;
                    }
                }
                SsaInstruction::EndLease { source_reg, .. } => {
                    let new_source = resolve(*source_reg);
                    if *source_reg != new_source {
                        *source_reg = new_source;
                        changed = true;
                    }
                }
                SsaInstruction::Entangle { regs } => {
                    for r in regs {
                        let new_r = resolve(*r);
                        if *r != new_r {
                            *r = new_r;
                            changed = true;
                        }
                    }
                }
                SsaInstruction::Consume { src } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::ConsumeField { src, .. } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::ConsumeFieldDynamic { target, index } => {
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
                SsaInstruction::Debug { src } => {
                    let new_src = resolve(*src);
                    if *src != new_src {
                        *src = new_src;
                        changed = true;
                    }
                }
                SsaInstruction::JumpIf { cond, .. } => {
                    let new_cond = resolve(*cond);
                    if *cond != new_cond {
                        *cond = new_cond;
                        changed = true;
                    }
                }
                SsaInstruction::JumpIfNot { cond, .. } => {
                    let new_cond = resolve(*cond);
                    if *cond != new_cond {
                        *cond = new_cond;
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
            SsaTerminator::Return { src: Some(r) } => {
                let new_r = resolve(*r);
                if *r != new_r {
                    *r = new_r;
                    changed = true;
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
