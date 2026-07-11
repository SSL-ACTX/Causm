use crate::{cfg::BlockId, Instruction, Reg, Terminator, CFG};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SsaReg {
    pub reg: u32,
    pub version: u32,
}

impl std::fmt::Display for SsaReg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}_{}", self.reg, self.version)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaPhiNode {
    pub dest: SsaReg,
    pub original_reg: Reg,
    pub incoming: Vec<(BlockId, SsaReg)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaBasicBlock {
    pub id: BlockId,
    pub phi_nodes: Vec<SsaPhiNode>,
    pub instructions: Vec<SsaInstruction>,
    pub terminator: SsaTerminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsaTerminator {
    Jump {
        target: BlockId,
    },
    Branch {
        cond: SsaReg,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return {
        src: Option<SsaReg>,
    },
    Unreachable,
}

// SsaInstruction corresponds to Instruction but uses SsaReg
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsaInstruction {
    BinaryOp {
        dest: SsaReg,
        op: causm_core::BinaryOperator,
        left: SsaReg,
        right: SsaReg,
    },
    UnaryOp {
        dest: SsaReg,
        op: causm_core::UnaryOperator,
        src: SsaReg,
    },
    LoadInt {
        dest: SsaReg,
        value: i64,
    },
    LoadFloat {
        dest: SsaReg,
        value: u64,
    },
    LoadBool {
        dest: SsaReg,
        value: bool,
    },
    LoadString {
        dest: SsaReg,
        value: String,
    },
    LoadNull {
        dest: SsaReg,
    },
    Move {
        dest: SsaReg,
        src: SsaReg,
    },
    Consume {
        src: SsaReg,
    },
    ConsumeField {
        src: SsaReg,
        field: String,
    },
    ConsumeFieldDynamic {
        target: SsaReg,
        index: SsaReg,
    },
    Clone {
        dest: SsaReg,
        src: SsaReg,
    },
    Call {
        routine: String,
        args: Vec<SsaReg>,
        dest: SsaReg,
    },
    Print {
        src: SsaReg,
    },
    Debug {
        src: SsaReg,
    },
    Slice {
        ms: u64,
    },
    // Other instructions can be represented as Generic for simplicity
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaCFG {
    pub entry_block: BlockId,
    pub blocks: HashMap<BlockId, SsaBasicBlock>,
}

impl std::fmt::Display for SsaCFG {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut block_ids: Vec<&BlockId> = self.blocks.keys().collect();
        block_ids.sort();

        for id in block_ids {
            let block = &self.blocks[id];
            writeln!(f, "  Block {}:", id)?;
            for phi in &block.phi_nodes {
                let incoming_str: Vec<String> = phi
                    .incoming
                    .iter()
                    .map(|(b, reg)| format!("{} from Block {}", reg, b))
                    .collect();
                writeln!(
                    f,
                    "    {} = phi [ {} ]",
                    phi.dest,
                    incoming_str.join(", ")
                )?;
            }
            for instr in &block.instructions {
                writeln!(f, "    {:?}", instr)?;
            }
            writeln!(f, "    Terminator: {:?}", block.terminator)?;
        }
        Ok(())
    }
}

// SSA Transformer
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
                Terminator::Return { .. } | Terminator::Unreachable => {}
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
            };
        }

        self.compute_dominators();
        self.compute_dominance_frontiers();

        // 1. Find all registers and their definition sites
        let mut def_sites: HashMap<u32, HashSet<BlockId>> = HashMap::new();
        let mut all_regs = HashSet::new();

        for (&block_id, block) in &self.cfg.blocks {
            for instr in &block.instructions {
                if let Some(dest) = get_dest_reg(instr) {
                    def_sites.entry(dest.0).or_default().insert(block_id);
                    all_regs.insert(dest.0);
                }
            }
        }

        // 2. Insert Phi nodes
        // phi_nodes[block_id] = list of (original_reg, PhiNode)
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

        SsaCFG {
            entry_block: self.cfg.entry_block,
            blocks: renamed_blocks,
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
        let ssa_term = match block.terminator {
            Terminator::Jump { target } => SsaTerminator::Jump { target },
            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => SsaTerminator::Branch {
                cond: self.current_ssa_reg(cond),
                then_block,
                else_block,
            },
            Terminator::Return { src } => SsaTerminator::Return {
                src: src.map(|r| self.current_ssa_reg(r)),
            },
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
        // Deterministic walk
        children.sort();
        for child in children {
            self.rename(child, inserted_phis, renamed_blocks);
        }

        // 6. Pop versions from stacks
        for phi in &phis {
            self.pop_version(phi.original_reg.0);
        }
        for instr in &block.instructions {
            if let Some(dest) = get_dest_reg(instr) {
                self.pop_version(dest.0);
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
                let left_ssa = self.current_ssa_reg(*left);
                let right_ssa = self.current_ssa_reg(*right);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::BinaryOp {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    op: *op,
                    left: left_ssa,
                    right: right_ssa,
                }
            }
            Instruction::UnaryOp { dest, op, src } => {
                let src_ssa = self.current_ssa_reg(*src);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::UnaryOp {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    op: *op,
                    src: src_ssa,
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
            Instruction::Move { dest, src } => {
                let src_ssa = self.current_ssa_reg(*src);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::Move {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: src_ssa,
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
                let src_ssa = self.current_ssa_reg(*src);
                let dest_ver = self.next_version(dest.0);
                self.push_version(dest.0, dest_ver);
                SsaInstruction::Clone {
                    dest: SsaReg {
                        reg: dest.0,
                        version: dest_ver,
                    },
                    src: src_ssa,
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
            Instruction::Print { src } => SsaInstruction::Print {
                src: self.current_ssa_reg(*src),
            },
            Instruction::Debug { src } => SsaInstruction::Debug {
                src: self.current_ssa_reg(*src),
            },
            Instruction::Slice { ms } => SsaInstruction::Slice { ms: *ms },
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

// Helpers to extract dest register from flat Instruction
fn get_dest_reg(instr: &Instruction) -> Option<Reg> {
    match instr {
        Instruction::BinaryOp { dest, .. } => Some(*dest),
        Instruction::UnaryOp { dest, .. } => Some(*dest),
        Instruction::LoadInt { dest, .. } => Some(*dest),
        Instruction::LoadFloat { dest, .. } => Some(*dest),
        Instruction::LoadBool { dest, .. } => Some(*dest),
        Instruction::LoadString { dest, .. } => Some(*dest),
        Instruction::LoadNull { dest } => Some(*dest),
        Instruction::Move { dest, .. } => Some(*dest),
        Instruction::Clone { dest, .. } => Some(*dest),
        Instruction::Call { dest, .. } => Some(*dest),
        Instruction::StructLit { dest, .. } => Some(*dest),
        Instruction::TopologyLit { dest, .. } => Some(*dest),
        Instruction::ArrayLit { dest, .. } => Some(*dest),
        Instruction::FieldAccess { dest, .. } => Some(*dest),
        Instruction::IndexAccess { dest, .. } => Some(*dest),
        Instruction::Defer { dest, .. } => Some(*dest),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssa_renaming_and_phi() {
        // R1 = 5
        // JumpIf R1 to Block 2
        // Block 1 (fallthrough):
        // R1 = 10
        // Jump to Block 3
        // Block 2:
        // R1 = 20
        // Jump to Block 3
        // Block 3:
        // R2 = R1 + R1
        // Return R2
        // Wait, let's write a simpler deterministic program for the unit test:
        // Block 0:
        // R0 = 5
        // JumpIf R0 target index 4
        // Block 1 (fallthrough):
        // R1 = 10
        // Jump target index 5
        // Block 2 (index 4):
        // R1 = 20
        // Block 3 (index 5):
        // R2 = R1 + R0
        let instrs = vec![
            Instruction::LoadInt {
                dest: Reg(0),
                value: 5,
            }, // 0: leader Block 0
            Instruction::JumpIf {
                cond: Reg(0),
                target: 4,
            }, // 1: Branch to 2 (index 4) or 1 (index 2)
            Instruction::LoadInt {
                dest: Reg(1),
                value: 10,
            }, // 2: leader Block 1
            Instruction::Jump { target: 5 }, // 3: Jump to 3 (index 5)
            Instruction::LoadInt {
                dest: Reg(1),
                value: 20,
            }, // 4: leader Block 2
            Instruction::BinaryOp {
                dest: Reg(2),
                op: causm_core::BinaryOperator::Add,
                left: Reg(1),
                right: Reg(0),
            }, // 5: leader Block 3
        ];

        let cfg = CFG::from_flat_instructions(&instrs);
        let transformer = SsaTransformer::new(cfg);
        let ssa_cfg = transformer.transform();

        // Let's assert Block 3 has a Phi node for R1!
        let b3 = ssa_cfg.blocks.get(&3).unwrap();
        assert_eq!(b3.phi_nodes.len(), 1);
        let phi = &b3.phi_nodes[0];
        println!("PHI INCOMING: {:?}", phi.incoming);
        assert_eq!(phi.original_reg, Reg(1));
        assert_eq!(phi.dest, SsaReg { reg: 1, version: 3 }); // version 3 (after v1=10, v2=20, dest phi becomes version 3)

        // Incoming to Phi node should be: (Block 1, R1_1) and (Block 2, R1_2)
        let incoming: HashSet<(BlockId, SsaReg)> =
            phi.incoming.iter().cloned().collect();
        assert!(incoming.contains(&(1, SsaReg { reg: 1, version: 1 })));
        assert!(incoming.contains(&(2, SsaReg { reg: 1, version: 2 })));
    }
}
