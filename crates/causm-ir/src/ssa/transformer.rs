use super::types::*;
use crate::{cfg::BlockId, Instruction, Reg, Terminator, CFG};
use std::collections::{HashMap, HashSet, VecDeque};

pub struct SsaTransformer {
    cfg: CFG,
    predecessors: HashMap<BlockId, Vec<BlockId>>,
    successors: HashMap<BlockId, Vec<BlockId>>,
    doms: HashMap<BlockId, BlockId>, // idom
    df: HashMap<BlockId, HashSet<BlockId>>,
    // Renaming state
    counter: HashMap<u32, u32>,
    stack: HashMap<u32, Vec<u32>>,
}

impl SsaTransformer {
    pub fn new(cfg: CFG) -> Self {
        let mut predecessors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        let mut successors: HashMap<BlockId, Vec<BlockId>> = HashMap::new();

        for (&id, block) in &cfg.blocks {
            predecessors.entry(id).or_default();
            successors.entry(id).or_default();

            match &block.terminator {
                Terminator::Jump { target } => {
                    successors.entry(id).or_default().push(*target);
                    predecessors.entry(*target).or_default().push(id);
                }
                Terminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    successors.entry(id).or_default().push(*then_block);
                    successors.entry(id).or_default().push(*else_block);
                    predecessors.entry(*then_block).or_default().push(id);
                    predecessors.entry(*else_block).or_default().push(id);
                }
                Terminator::MatchEntropy {
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
                Terminator::Select {
                    cases,
                    timeout_block,
                    ..
                } => {
                    for case in cases {
                        successors.entry(id).or_default().push(case.target_block);
                        predecessors.entry(case.target_block).or_default().push(id);
                    }
                    if let Some(t) = timeout_block {
                        successors.entry(id).or_default().push(*t);
                        predecessors.entry(*t).or_default().push(id);
                    }
                }
                Terminator::Return { .. } | Terminator::Unreachable => {}
            }

            for instr in &block.instructions {
                match instr {
                    Instruction::RelativisticBlock {
                        block_pc,
                        block_len,
                        ..
                    } => {
                        let body_block = cfg.original_pc_to_block_id[block_pc];
                        let end_block =
                            cfg.original_pc_to_block_id[&(block_pc + block_len)];
                        let succ_list = successors.entry(id).or_default();
                        if !succ_list.contains(&body_block) {
                            succ_list.push(body_block);
                            predecessors.entry(body_block).or_default().push(id);
                        }
                        let succ_list = successors.entry(id).or_default();
                        if !succ_list.contains(&end_block) {
                            succ_list.push(end_block);
                            predecessors.entry(end_block).or_default().push(id);
                        }
                    }
                    Instruction::Watchdog {
                        recovery_jump: Some(t),
                        ..
                    } => {
                        let recovery_block = cfg.original_pc_to_block_id[t];
                        let succ_list = successors.entry(id).or_default();
                        if !succ_list.contains(&recovery_block) {
                            succ_list.push(recovery_block);
                            predecessors.entry(recovery_block).or_default().push(id);
                        }
                    }
                    Instruction::Speculate {
                        fallback_target, ..
                    }
                    | Instruction::EndSpeculate {
                        fallback_target, ..
                    } => {
                        let fallback_block =
                            cfg.original_pc_to_block_id[fallback_target];
                        let succ_list = successors.entry(id).or_default();
                        if !succ_list.contains(&fallback_block) {
                            succ_list.push(fallback_block);
                            predecessors.entry(fallback_block).or_default().push(id);
                        }
                    }
                    _ => {}
                }
            }
        }

        Self {
            cfg,
            predecessors,
            successors,
            doms: HashMap::new(),
            df: HashMap::new(),
            counter: HashMap::new(),
            stack: HashMap::new(),
        }
    }

    pub fn transform(mut self) -> SsaCFG {
        let entry = self.cfg.entry_block;
        let mut post_order = Vec::new();
        let mut visited = HashSet::new();
        self.post_order_dfs(entry, &mut visited, &mut post_order);

        // Keep only reachable blocks and purge unreachable edges
        self.predecessors.retain(|k, _| visited.contains(k));
        for preds in self.predecessors.values_mut() {
            preds.retain(|p| visited.contains(p));
        }

        self.successors.retain(|k, _| visited.contains(k));
        for succs in self.successors.values_mut() {
            succs.retain(|s| visited.contains(s));
        }

        self.cfg.blocks.retain(|k, _| visited.contains(k));

        if self.cfg.blocks.is_empty() {
            return SsaCFG {
                entry_block: self.cfg.entry_block,
                blocks: HashMap::new(),
                original_pc_to_block_id: self.cfg.original_pc_to_block_id.clone(),
            };
        }

        self.compute_dominators();
        self.compute_dominance_frontiers();

        // 1. Find all registers and their definition sites
        let mut def_sites: HashMap<u32, HashSet<BlockId>> = HashMap::new();
        let mut all_regs = HashSet::new();

        for (&block_id, block) in &self.cfg.blocks {
            for instr in &block.instructions {
                for_each_dest_reg_recursive(instr, &mut |dest| {
                    def_sites.entry(dest.0).or_default().insert(block_id);
                    all_regs.insert(dest.0);
                });
            }
            if let Terminator::Select { cases, .. } = &block.terminator {
                for case in cases {
                    def_sites.entry(case.dest.0).or_default().insert(block_id);
                    all_regs.insert(case.dest.0);
                }
            }
        }

        // 2. Insert Phi nodes
        let mut inserted_phis: HashMap<BlockId, Vec<SsaPhiNode>> = HashMap::new();

        for &reg in &all_regs {
            let mut work_list: VecDeque<BlockId> = def_sites
                .get(&reg)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();
            let mut added_phi: HashSet<BlockId> = HashSet::new();

            while let Some(x) = work_list.pop_front() {
                if let Some(frontier) = self.df.get(&x) {
                    for &y in frontier {
                        if !added_phi.contains(&y) {
                            added_phi.insert(y);
                            let phi = SsaPhiNode {
                                dest: SsaReg { reg, version: 0 },
                                original_reg: Reg(reg),
                                incoming: Vec::new(),
                            };
                            inserted_phis.entry(y).or_default().push(phi);
                            work_list.push_back(y);
                        }
                    }
                }
            }
        }

        // 3. Rename variables (DFS traversal of dominator tree)
        let mut renamed_blocks = HashMap::new();
        self.rename(
            self.cfg.entry_block,
            &mut inserted_phis,
            &mut renamed_blocks,
        );

        for (block_id, phis) in inserted_phis {
            if let Some(block) = renamed_blocks.get_mut(&block_id) {
                for (i, p) in phis.into_iter().enumerate() {
                    if i < block.phi_nodes.len() {
                        block.phi_nodes[i].incoming = p.incoming;
                    }
                }
            }
        }

        SsaCFG {
            entry_block: self.cfg.entry_block,
            blocks: renamed_blocks,
            original_pc_to_block_id: self.cfg.original_pc_to_block_id.clone(),
        }
    }

    fn compute_dominators(&mut self) {
        let entry = self.cfg.entry_block;
        let mut post_order = Vec::new();
        let mut visited = HashSet::new();
        self.post_order_dfs(entry, &mut visited, &mut post_order);

        let mut post_order_index = HashMap::new();
        for (i, &id) in post_order.iter().enumerate() {
            post_order_index.insert(id, i);
        }

        self.doms.insert(entry, entry);
        let mut changed = true;

        while changed {
            changed = false;
            for &b in post_order.iter().rev() {
                if b == entry {
                    continue;
                }
                let preds = &self.predecessors[&b];
                let mut new_idom = preds
                    .iter()
                    .cloned()
                    .find(|p| self.doms.contains_key(p))
                    .unwrap();

                for &p in preds {
                    if p != new_idom && self.doms.contains_key(&p) {
                        new_idom = self.intersect(p, new_idom, &post_order_index);
                    }
                }

                if self.doms.get(&b) != Some(&new_idom) {
                    self.doms.insert(b, new_idom);
                    changed = true;
                }
            }
        }
    }

    fn intersect(
        &self,
        mut b1: BlockId,
        mut b2: BlockId,
        post_order_index: &HashMap<BlockId, usize>,
    ) -> BlockId {
        while b1 != b2 {
            while post_order_index[&b1] < post_order_index[&b2] {
                b1 = self.doms[&b1];
            }
            while post_order_index[&b2] < post_order_index[&b1] {
                b2 = self.doms[&b2];
            }
        }
        b1
    }

    fn post_order_dfs(
        &self,
        node: BlockId,
        visited: &mut HashSet<BlockId>,
        post_order: &mut Vec<BlockId>,
    ) {
        visited.insert(node);
        if let Some(succs) = self.successors.get(&node) {
            for &succ in succs {
                if !visited.contains(&succ) {
                    self.post_order_dfs(succ, visited, post_order);
                }
            }
        }
        post_order.push(node);
    }

    fn compute_dominance_frontiers(&mut self) {
        for &b in self.cfg.blocks.keys() {
            self.df.insert(b, HashSet::new());
        }

        for (&b, preds) in &self.predecessors {
            if preds.len() >= 2 {
                for &p in preds {
                    let mut runner = p;
                    let idom = self.doms[&b];
                    while runner != idom {
                        self.df.get_mut(&runner).unwrap().insert(b);
                        runner = self.doms[&runner];
                    }
                }
            }
        }
    }

    fn rename(
        &mut self,
        block_id: BlockId,
        inserted_phis: &mut HashMap<BlockId, Vec<SsaPhiNode>>,
        renamed_blocks: &mut HashMap<BlockId, SsaBasicBlock>,
    ) {
        let block = self.cfg.blocks[&block_id].clone();

        // 1. Rename Phi node destinations
        let mut phis = inserted_phis.get(&block_id).cloned().unwrap_or_default();
        for phi in &mut phis {
            let r = phi.original_reg.0;
            let new_ver = self.next_version(r);
            phi.dest = SsaReg {
                reg: r,
                version: new_ver,
            };
            self.push_version(r, new_ver);
        }

        // 2. Rename instructions
        let mut ssa_instrs = Vec::new();
        for instr in &block.instructions {
            let ssa_instr = self.rename_instruction(instr);
            ssa_instrs.push(ssa_instr);
        }

        // 3. Rename terminator condition/src
        let ssa_term = match &block.terminator {
            Terminator::Jump { target } => SsaTerminator::Jump { target: *target },
            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => SsaTerminator::Branch {
                cond: self.current_ssa_reg(*cond),
                then_block: *then_block,
                else_block: *else_block,
            },
            Terminator::Return { src } => SsaTerminator::Return {
                src: src.map(|r| self.current_ssa_reg(r)),
            },
            Terminator::MatchEntropy {
                target,
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
            } => SsaTerminator::MatchEntropy {
                target: self.current_ssa_reg(*target),
                valid_block: *valid_block,
                decayed_block: *decayed_block,
                pending_block: *pending_block,
                consumed_block: *consumed_block,
            },
            Terminator::Select {
                max_ms,
                cases,
                timeout_block,
            } => {
                let cases_ssa = cases
                    .iter()
                    .map(|c| {
                        let dest_ver = self.next_version(c.dest.0);
                        self.push_version(c.dest.0, dest_ver);
                        SsaSelectCase {
                            chan_id: c.chan_id.clone(),
                            dest: SsaReg {
                                reg: c.dest.0,
                                version: dest_ver,
                            },
                            target: c.target_block as usize,
                        }
                    })
                    .collect();
                SsaTerminator::Select {
                    max_ms: *max_ms,
                    cases: cases_ssa,
                    timeout_block: *timeout_block,
                }
            }
            Terminator::Unreachable => SsaTerminator::Unreachable,
        };

        // 4. Fill in successor Phi node parameters in inserted_phis
        if let Some(succs) = self.successors.get(&block_id) {
            for &succ in succs {
                if let Some(succ_phis) = inserted_phis.get_mut(&succ) {
                    for phi in succ_phis {
                        let orig_r = phi.original_reg.0;
                        let ver = self.current_version(orig_r);
                        phi.incoming.push((
                            block_id,
                            SsaReg {
                                reg: orig_r,
                                version: ver,
                            },
                        ));
                    }
                }
            }
        }

        // Insert our completed block
        renamed_blocks.insert(
            block_id,
            SsaBasicBlock {
                id: block_id,
                phi_nodes: phis.clone(),
                instructions: ssa_instrs,
                terminator: ssa_term,
            },
        );

        // 5. Recurse on children in Dominator Tree
        let mut children = Vec::new();
        for (&child, &parent) in &self.doms {
            if parent == block_id && child != block_id {
                children.push(child);
            }
        }
        children.sort();
        for child in children {
            self.rename(child, inserted_phis, renamed_blocks);
        }

        // 6. Pop versions from stacks
        for phi in &phis {
            self.pop_version(phi.original_reg.0);
        }
        for instr in &block.instructions {
            for_each_dest_reg(instr, |dest| {
                self.pop_version(dest.0);
            });
        }
        if let Terminator::Select { cases, .. } = &block.terminator {
            for case in cases {
                self.pop_version(case.dest.0);
            }
        }
    }

    fn rename_instruction(&mut self, instr: &Instruction) -> SsaInstruction {
        match instr {
            Instruction::BinaryOp {
                dest,
                op,
                left,
                right,
            } => {
                let left_ver = self.current_version(left.0);
                let right_ver = self.current_version(right.0);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::BinaryOp {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    op: *op,
                    left: SsaReg {
                        reg: left.0,
                        version: left_ver,
                    },
                    right: SsaReg {
                        reg: right.0,
                        version: right_ver,
                    },
                }
            }
            Instruction::UnaryOp { dest, op, src } => {
                let src_ver = self.current_version(src.0);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::UnaryOp {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    op: *op,
                    src: SsaReg {
                        reg: src.0,
                        version: src_ver,
                    },
                }
            }
            Instruction::LoadInt { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::LoadInt {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: *value,
                }
            }
            Instruction::LoadFloat { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::LoadFloat {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: *value,
                }
            }
            Instruction::LoadBool { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::LoadBool {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: *value,
                }
            }
            Instruction::LoadString { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::LoadString {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: value.clone(),
                }
            }
            Instruction::LoadNull { dest } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::LoadNull {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                }
            }
            Instruction::ConstInt { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ConstInt {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: *value,
                }
            }
            Instruction::ConstFloat { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ConstFloat {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: *value,
                }
            }
            Instruction::ConstBool { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ConstBool {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: *value,
                }
            }
            Instruction::ConstString { dest, value } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ConstString {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    value: value.clone(),
                }
            }
            Instruction::ConstNull { dest } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ConstNull {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                }
            }
            Instruction::Move { dest, src } => {
                let src_ver = self.current_version(src.0);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::Move {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: SsaReg {
                        reg: src.0,
                        version: src_ver,
                    },
                }
            }
            Instruction::Consume { src } => SsaInstruction::Consume {
                src: self.current_ssa_reg(*src),
            },
            Instruction::ConsumeField { src, field } => {
                SsaInstruction::ConsumeField {
                    src: self.current_ssa_reg(*src),
                    field: field.clone(),
                }
            }
            Instruction::ConsumeFieldDynamic { target, index } => {
                SsaInstruction::ConsumeFieldDynamic {
                    target: self.current_ssa_reg(*target),
                    index: self.current_ssa_reg(*index),
                }
            }
            Instruction::Clone { dest, src } => {
                let src_ver = self.current_version(src.0);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::Clone {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: SsaReg {
                        reg: src.0,
                        version: src_ver,
                    },
                }
            }
            Instruction::StrBytes { dest, src } => {
                let src_ver = self.current_version(src.0);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::StrBytes {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: SsaReg {
                        reg: src.0,
                        version: src_ver,
                    },
                }
            }
            Instruction::ToStr { dest, src } => {
                let src_ver = self.current_version(src.0);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ToStr {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: SsaReg {
                        reg: src.0,
                        version: src_ver,
                    },
                }
            }
            Instruction::Call {
                routine,
                args,
                dest,
            } => {
                let args_ssa =
                    args.iter().map(|&r| self.current_ssa_reg(r)).collect();
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::Call {
                    routine: routine.clone(),
                    args: args_ssa,
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                }
            }
            Instruction::DynamicCall {
                method,
                args,
                dest,
                budget,
            } => {
                let args_ssa =
                    args.iter().map(|&r| self.current_ssa_reg(r)).collect();
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::DynamicCall {
                    method: method.clone(),
                    args: args_ssa,
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    budget: *budget,
                }
            }
            Instruction::TypeAssert {
                dest,
                src,
                type_name,
            } => {
                let src_ssa = self.current_ssa_reg(*src);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::TypeAssert {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: src_ssa,
                    type_name: type_name.clone(),
                }
            }
            Instruction::TypeCast {
                dest,
                src,
                target_type,
            } => {
                let src_ssa = self.current_ssa_reg(*src);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::TypeCast {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: src_ssa,
                    target_type: target_type.clone(),
                }
            }
            Instruction::AssertState { src, state } => SsaInstruction::AssertState {
                src: self.current_ssa_reg(*src),
                state: state.clone(),
            },
            Instruction::TryTypeAssert {
                dest,
                src,
                type_name,
                success,
            } => {
                let src_ssa = self.current_ssa_reg(*src);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                let success_ver = self.next_version(success.0);
                self.push_version(success.0, success_ver);
                SsaInstruction::TryTypeAssert {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: src_ssa,
                    type_name: type_name.clone(),
                    success: SsaReg {
                        reg: success.0,
                        version: success_ver,
                    },
                }
            }
            Instruction::Print { src } => SsaInstruction::Print {
                src: self.current_ssa_reg(*src),
            },
            Instruction::Debug { src } => SsaInstruction::Debug {
                src: self.current_ssa_reg(*src),
            },
            Instruction::Slice { ms } => SsaInstruction::Slice { ms: *ms },
            Instruction::Isolate { name, manifest } => SsaInstruction::Isolate {
                name: name.clone(),
                manifest: manifest.clone(),
            },
            Instruction::EndIsolate => SsaInstruction::EndIsolate,
            Instruction::Lease {
                target_reg,
                source_reg,
                duration_ms,
            } => {
                let source_ssa = self.current_ssa_reg(*source_reg);
                let target_ver = self.next_version(target_reg.0);
                self.push_version(target_reg.0, target_ver);
                SsaInstruction::Lease {
                    target_reg: SsaReg {
                        reg: target_reg.0,
                        version: target_ver,
                    },
                    source_reg: source_ssa,
                    duration_ms: *duration_ms,
                }
            }
            Instruction::EndLease {
                source_reg,
                duration_ms,
            } => SsaInstruction::EndLease {
                source_reg: self.current_ssa_reg(*source_reg),
                duration_ms: *duration_ms,
            },
            Instruction::Split { parent, branches } => SsaInstruction::Split {
                parent: parent.clone(),
                branches: branches.clone(),
            },
            Instruction::Merge {
                branches,
                target,
                resolution,
            } => SsaInstruction::Merge {
                branches: branches.clone(),
                target: target.clone(),
                resolution: resolution.clone(),
            },
            Instruction::Entangle { regs } => {
                let regs_ssa =
                    regs.iter().map(|&r| self.current_ssa_reg(r)).collect();
                SsaInstruction::Entangle { regs: regs_ssa }
            }
            Instruction::SetEntropyMode { mode } => {
                SsaInstruction::SetEntropyMode { mode: *mode }
            }
            Instruction::Anchor { name } => {
                SsaInstruction::Anchor { name: name.clone() }
            }
            Instruction::Rewind { target, anchor } => SsaInstruction::Rewind {
                target: target.clone(),
                anchor: anchor.clone(),
            },
            Instruction::Commit { vars } => {
                SsaInstruction::Commit { vars: vars.clone() }
            }
            Instruction::Watchdog {
                target,
                timeout_ms,
                recovery_jump,
            } => SsaInstruction::Watchdog {
                target: target.clone(),
                timeout_ms: *timeout_ms,
                recovery_jump: *recovery_jump,
            },
            Instruction::Speculate {
                max_ms,
                fallback_target,
            } => SsaInstruction::Speculate {
                max_ms: *max_ms,
                fallback_target: *fallback_target,
            },
            Instruction::EndSpeculate {
                max_ms,
                fallback_target,
            } => SsaInstruction::EndSpeculate {
                max_ms: *max_ms,
                fallback_target: *fallback_target,
            },
            Instruction::Collapse => SsaInstruction::Collapse,
            Instruction::Select {
                max_ms,
                cases,
                timeout_target,
            } => {
                let cases_ssa = cases
                    .iter()
                    .map(|c| {
                        let dest_ver = self.next_version(c.dest.0);
                        self.push_version(c.dest.0, dest_ver);
                        SsaSelectCase {
                            chan_id: c.chan_id.clone(),
                            dest: SsaReg {
                                reg: c.dest.0,
                                version: dest_ver,
                            },
                            target: c.target,
                        }
                    })
                    .collect();
                SsaInstruction::Select {
                    max_ms: *max_ms,
                    cases: cases_ssa,
                    timeout_target: *timeout_target,
                }
            }
            Instruction::MatchEntropy {
                target,
                valid_target,
                decayed_target,
                pending_target,
                consumed_target,
            } => SsaInstruction::MatchEntropy {
                target: self.current_ssa_reg(*target),
                valid_target: *valid_target,
                decayed_target: *decayed_target,
                pending_target: *pending_target,
                consumed_target: *consumed_target,
            },
            Instruction::RelativisticBlock {
                target,
                block_pc,
                block_len,
            } => SsaInstruction::RelativisticBlock {
                target: target.clone(),
                block_pc: *block_pc,
                block_len: *block_len,
            },
            Instruction::SpeculationMode { mode } => {
                SsaInstruction::SpeculationMode { mode: *mode }
            }
            Instruction::OpenChan {
                name,
                capacity,
                decay_after_ms,
            } => SsaInstruction::OpenChan {
                name: name.clone(),
                capacity: *capacity,
                decay_after_ms: *decay_after_ms,
            },
            Instruction::ChanSend { chan_id, src } => SsaInstruction::ChanSend {
                chan_id: chan_id.clone(),
                src: self.current_ssa_reg(*src),
            },
            Instruction::ChanRecv { dest, chan_id } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ChanRecv {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    chan_id: chan_id.clone(),
                }
            }
            Instruction::AwaitChan { chan_id } => SsaInstruction::AwaitChan {
                chan_id: chan_id.clone(),
            },
            Instruction::StructLit {
                dest,
                fields,
                type_name,
            } => {
                let fields_ssa = fields
                    .iter()
                    .map(|(k, &v)| (k.clone(), self.current_ssa_reg(v)))
                    .collect();
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::StructLit {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    fields: fields_ssa,
                    type_name: type_name.clone(),
                }
            }
            Instruction::TopologyLit { dest, fields } => {
                let fields_ssa = fields
                    .iter()
                    .map(|(k, &v)| (k.clone(), self.current_ssa_reg(v)))
                    .collect();
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::TopologyLit {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    fields: fields_ssa,
                }
            }
            Instruction::ArrayLit { dest, elements } => {
                let elements_ssa =
                    elements.iter().map(|&v| self.current_ssa_reg(v)).collect();
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::ArrayLit {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    elements: elements_ssa,
                }
            }
            Instruction::FieldAccess {
                dest,
                target,
                field,
            } => {
                let target_ssa = self.current_ssa_reg(*target);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::FieldAccess {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    target: target_ssa,
                    field: field.clone(),
                }
            }
            Instruction::FieldUpdate { target, field, src } => {
                let old_target_ssa = self.current_ssa_reg(*target);
                let src_ssa = self.current_ssa_reg(*src);
                let target_ver = self.next_version(target.0);
                self.push_version(target.0, target_ver);
                SsaInstruction::FieldUpdate {
                    target: SsaReg {
                        reg: target.0,
                        version: target_ver,
                    },
                    old_target: old_target_ssa,
                    field: field.clone(),
                    src: src_ssa,
                }
            }
            Instruction::IndexAccess {
                dest,
                target,
                index,
            } => {
                let target_ssa = self.current_ssa_reg(*target);
                let index_ssa = self.current_ssa_reg(*index);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::IndexAccess {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    target: target_ssa,
                    index: index_ssa,
                }
            }
            Instruction::IndexFieldUpdate {
                target,
                index,
                field,
                src,
            } => {
                let old_target_ssa = self.current_ssa_reg(*target);
                let index_ssa = self.current_ssa_reg(*index);
                let src_ssa = self.current_ssa_reg(*src);
                let target_ver = self.next_version(target.0);
                self.push_version(target.0, target_ver);
                SsaInstruction::IndexFieldUpdate {
                    target: SsaReg {
                        reg: target.0,
                        version: target_ver,
                    },
                    old_target: old_target_ssa,
                    index: index_ssa,
                    field: field.clone(),
                    src: src_ssa,
                }
            }
            Instruction::AssertTime { op, limit_ms } => SsaInstruction::AssertTime {
                op: *op,
                limit_ms: *limit_ms,
            },
            Instruction::Capability { cap } => {
                SsaInstruction::Capability { cap: cap.clone() }
            }
            Instruction::For {
                dest_cond,
                item_reg,
                item_name,
                mode,
                source,
                pacing_ms,
                max_ms,
            } => {
                let dest_ver = self.next_version(dest_cond.0);
                self.push_version(dest_cond.0, dest_ver);
                let item_ver = self.next_version(item_reg.0);
                self.push_version(item_reg.0, item_ver);
                SsaInstruction::For {
                    dest_cond: SsaReg {
                        reg: dest_cond.0,
                        version: dest_ver,
                    },
                    item_reg: SsaReg {
                        reg: item_reg.0,
                        version: item_ver,
                    },
                    item_name: item_name.clone(),
                    mode: mode.clone(),
                    source: self.current_ssa_reg(*source),
                    pacing_ms: *pacing_ms,
                    max_ms: *max_ms,
                }
            }
            Instruction::EndFor => SsaInstruction::EndFor,
            Instruction::SplitMap {
                item_reg,
                item_name,
                mode,
                source,
                reconcile,
            } => {
                let item_ver = self.next_version(item_reg.0);
                self.push_version(item_reg.0, item_ver);
                SsaInstruction::SplitMap {
                    item_reg: SsaReg {
                        reg: item_reg.0,
                        version: item_ver,
                    },
                    item_name: item_name.clone(),
                    mode: mode.clone(),
                    source: self.current_ssa_reg(*source),
                    reconcile: reconcile.clone(),
                }
            }
            Instruction::EndSplitMap => SsaInstruction::EndSplitMap,
            Instruction::Defer {
                dest,
                cap,
                deadline_ms,
            } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::Defer {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    cap: cap.clone(),
                    deadline_ms: *deadline_ms,
                }
            }
            Instruction::Await { target } => SsaInstruction::Await {
                target: self.current_ssa_reg(*target),
            },
            Instruction::Loop { max_ms } => SsaInstruction::Loop { max_ms: *max_ms },
            Instruction::EndLoop { max_ms } => {
                SsaInstruction::EndLoop { max_ms: *max_ms }
            }
            Instruction::Break => SsaInstruction::Break,
            Instruction::LoopTick => SsaInstruction::LoopTick,
            Instruction::EndLoopTick => SsaInstruction::EndLoopTick,
            Instruction::NetworkRequest { domain } => {
                SsaInstruction::NetworkRequest {
                    domain: domain.clone(),
                }
            }
            Instruction::Jump { target } => SsaInstruction::Jump { target: *target },
            Instruction::JumpIf { cond, target } => SsaInstruction::JumpIf {
                cond: self.current_ssa_reg(*cond),
                target: *target,
            },
            Instruction::JumpIfNot { cond, target } => SsaInstruction::JumpIfNot {
                cond: self.current_ssa_reg(*cond),
                target: *target,
            },
            Instruction::While { max_ms } => {
                SsaInstruction::While { max_ms: *max_ms }
            }
            Instruction::EndWhile { max_ms } => {
                SsaInstruction::EndWhile { max_ms: *max_ms }
            }
            Instruction::ForStep {
                dest_cond,
                item_reg,
                item_name,
                source,
                step_ms,
            } => {
                let dest_ver = self.next_version(dest_cond.0);
                self.push_version(dest_cond.0, dest_ver);
                let item_ver = self.next_version(item_reg.0);
                self.push_version(item_reg.0, item_ver);
                SsaInstruction::ForStep {
                    dest_cond: SsaReg {
                        reg: dest_cond.0,
                        version: dest_ver,
                    },
                    item_reg: SsaReg {
                        reg: item_reg.0,
                        version: item_ver,
                    },
                    item_name: item_name.clone(),
                    source: self.current_ssa_reg(*source),
                    step_ms: *step_ms,
                }
            }
            Instruction::EndForStep => SsaInstruction::EndForStep,
            Instruction::ArrayLen { dest, src } => {
                let v = self.next_version(dest.0);
                self.push_version(dest.0, v);
                SsaInstruction::ArrayLen {
                    dest: SsaReg {
                        reg: dest.0,
                        version: v,
                    },
                    src: self.current_ssa_reg(*src),
                }
            }
            Instruction::LoopTickOn { chan_id } => SsaInstruction::LoopTickOn {
                chan_id: chan_id.clone(),
            },
            Instruction::Syscall {
                dest,
                target,
                args,
                duration_ms,
            } => {
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                let ssa_dest = SsaReg {
                    reg: dest.0,
                    version: dest_ver,
                };
                let ssa_args =
                    args.iter().map(|r| self.current_ssa_reg(*r)).collect();
                SsaInstruction::Syscall {
                    dest: ssa_dest,
                    target: target.clone(),
                    args: ssa_args,
                    duration_ms: *duration_ms,
                }
            }
            Instruction::AutoDrop { target, spec } => SsaInstruction::AutoDrop {
                target: self.current_ssa_reg(*target),
                spec: spec.clone(),
            },
            _ => SsaInstruction::Other(format!("{:?}", instr)),
        }
    }

    fn current_ssa_reg(&self, reg: Reg) -> SsaReg {
        SsaReg {
            reg: reg.0,
            version: self.current_version(reg.0),
        }
    }

    fn current_version(&self, reg: u32) -> u32 {
        self.stack
            .get(&reg)
            .and_then(|v| v.last())
            .cloned()
            .unwrap_or(0)
    }

    fn next_version(&mut self, reg: u32) -> u32 {
        let count = self.counter.entry(reg).or_insert(0);
        *count += 1;
        *count
    }

    fn push_version(&mut self, reg: u32, version: u32) {
        self.stack.entry(reg).or_default().push(version);
    }

    fn pop_version(&mut self, reg: u32) {
        if let Some(stack) = self.stack.get_mut(&reg) {
            stack.pop();
        }
    }
}

fn for_each_dest_reg(instr: &Instruction, mut f: impl FnMut(Reg)) {
    match instr {
        Instruction::BinaryOp { dest, .. } => f(*dest),
        Instruction::UnaryOp { dest, .. } => f(*dest),
        Instruction::LoadInt { dest, .. } => f(*dest),
        Instruction::LoadFloat { dest, .. } => f(*dest),
        Instruction::LoadBool { dest, .. } => f(*dest),
        Instruction::LoadString { dest, .. } => f(*dest),
        Instruction::LoadNull { dest } => f(*dest),
        Instruction::ConstInt { dest, .. } => f(*dest),
        Instruction::ConstFloat { dest, .. } => f(*dest),
        Instruction::ConstBool { dest, .. } => f(*dest),
        Instruction::ConstString { dest, .. } => f(*dest),
        Instruction::ConstNull { dest } => f(*dest),
        Instruction::Move { dest, .. } => f(*dest),
        Instruction::Clone { dest, .. } => f(*dest),
        Instruction::StrBytes { dest, .. } => f(*dest),
        Instruction::ToStr { dest, .. } => f(*dest),
        Instruction::ArrayLen { dest, .. } => f(*dest),
        Instruction::Call { dest, .. } => f(*dest),
        Instruction::DynamicCall { dest, .. } => f(*dest),
        Instruction::TypeAssert { dest, .. } => f(*dest),
        Instruction::TypeCast { dest, .. } => f(*dest),
        Instruction::TryTypeAssert { dest, success, .. } => {
            f(*dest);
            f(*success);
        }
        Instruction::StructLit { dest, .. } => f(*dest),
        Instruction::TopologyLit { dest, .. } => f(*dest),
        Instruction::ArrayLit { dest, .. } => f(*dest),
        Instruction::FieldAccess { dest, .. } => f(*dest),
        Instruction::IndexAccess { dest, .. } => f(*dest),
        Instruction::Defer { dest, .. } => f(*dest),
        Instruction::Lease { target_reg, .. } => f(*target_reg),
        Instruction::ChanRecv { dest, .. } => f(*dest),
        Instruction::FieldUpdate { target, .. } => f(*target),
        Instruction::IndexFieldUpdate { target, .. } => f(*target),
        Instruction::For {
            dest_cond,
            item_reg,
            ..
        } => {
            f(*dest_cond);
            f(*item_reg);
        }
        Instruction::ForStep {
            dest_cond,
            item_reg,
            ..
        } => {
            f(*dest_cond);
            f(*item_reg);
        }
        Instruction::SplitMap { item_reg, .. } => f(*item_reg),
        Instruction::Select { cases, .. } => {
            for case in cases {
                f(case.dest);
            }
        }
        _ => {}
    }
}

fn for_each_dest_reg_recursive(instr: &Instruction, f: &mut impl FnMut(Reg)) {
    for_each_dest_reg(instr, f);
}
