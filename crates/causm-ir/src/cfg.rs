use crate::{Instruction, Reg};
use std::collections::{HashMap, HashSet};

pub type BlockId = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFG {
    pub entry_block: BlockId,
    pub blocks: HashMap<BlockId, BasicBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    Jump {
        target: BlockId,
    },
    Branch {
        cond: Reg,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return {
        src: Option<Reg>,
    },
    Unreachable,
}

impl CFG {
    pub fn from_flat_instructions(instructions: &[Instruction]) -> Self {
        if instructions.is_empty() {
            return Self {
                entry_block: 0,
                blocks: HashMap::new(),
            };
        }

        // 1. Identify leaders (instruction indices that start basic blocks)
        let mut leaders = HashSet::new();
        leaders.insert(0);

        for (idx, instr) in instructions.iter().enumerate() {
            match instr {
                Instruction::Jump { target } => {
                    leaders.insert(*target);
                    leaders.insert(idx + 1);
                }
                Instruction::JumpIf { target, .. }
                | Instruction::JumpIfNot { target, .. } => {
                    leaders.insert(*target);
                    leaders.insert(idx + 1);
                }
                Instruction::Return { .. } => {
                    leaders.insert(idx + 1);
                }
                _ => {}
            }
        }

        let mut sorted_leaders: Vec<usize> = leaders
            .into_iter()
            .filter(|&l| l <= instructions.len())
            .collect();
        sorted_leaders.sort_unstable();

        // Map flat index to BlockId
        let mut index_to_block_id = HashMap::new();
        for (block_id, &start_idx) in sorted_leaders.iter().enumerate() {
            index_to_block_id.insert(start_idx, block_id as BlockId);
        }

        // Helper to find block id for any flat index (handles fallthrough / exact matches)
        let find_block_id = |flat_idx: usize| -> BlockId {
            let pos = sorted_leaders
                .binary_search(&flat_idx)
                .unwrap_or_else(|x| x - 1);
            index_to_block_id[&sorted_leaders[pos]]
        };

        let mut blocks = HashMap::new();

        for i in 0..sorted_leaders.len() {
            let start = sorted_leaders[i];
            let block_id = index_to_block_id[&start];
            let mut block_instrs = Vec::new();
            let terminator;

            if start < instructions.len() {
                let end = if i + 1 < sorted_leaders.len() {
                    sorted_leaders[i + 1].min(instructions.len())
                } else {
                    instructions.len()
                };

                block_instrs = instructions[start..end].to_vec();

                if let Some(last) = block_instrs.last().cloned() {
                    match last {
                        Instruction::Jump { target } => {
                            block_instrs.pop();
                            terminator = Terminator::Jump {
                                target: find_block_id(target),
                            };
                        }
                        Instruction::JumpIf { cond, target } => {
                            block_instrs.pop();
                            terminator = Terminator::Branch {
                                cond,
                                then_block: find_block_id(target),
                                else_block: (block_id + 1) as BlockId,
                            };
                        }
                        Instruction::JumpIfNot { cond, target } => {
                            block_instrs.pop();
                            // Reversing condition branch targets for JumpIfNot
                            terminator = Terminator::Branch {
                                cond,
                                then_block: (block_id + 1) as BlockId,
                                else_block: find_block_id(target),
                            };
                        }
                        Instruction::Return { src } => {
                            block_instrs.pop();
                            terminator = Terminator::Return { src };
                        }
                        _ => {
                            if end < instructions.len() {
                                terminator = Terminator::Jump {
                                    target: (block_id + 1) as BlockId,
                                };
                            } else {
                                terminator = Terminator::Return { src: None };
                            }
                        }
                    }
                } else {
                    terminator = Terminator::Return { src: None };
                }
            } else {
                terminator = Terminator::Return { src: None };
            }

            blocks.insert(
                block_id,
                BasicBlock {
                    id: block_id,
                    instructions: block_instrs,
                    terminator,
                },
            );
        }

        Self {
            entry_block: 0,
            blocks,
        }
    }
}

impl std::fmt::Display for CFG {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut block_ids: Vec<&BlockId> = self.blocks.keys().collect();
        block_ids.sort();

        for id in block_ids {
            let block = &self.blocks[id];
            writeln!(f, "  Block {}:", id)?;
            for instr in &block.instructions {
                writeln!(f, "    {:?}", instr)?;
            }
            writeln!(f, "    Terminator: {:?}", block.terminator)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_construction() {
        // R0 = 5
        // JumpIf R0 to 4
        // R1 = 10
        // Jump to 5
        // R1 = 20 (index 4)
        // Return R1 (index 5)
        let instructions = vec![
            Instruction::LoadInt {
                dest: Reg(0),
                value: 5,
            },
            Instruction::JumpIf {
                cond: Reg(0),
                target: 4,
            },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 10,
            },
            Instruction::Jump { target: 5 },
            Instruction::LoadInt {
                dest: Reg(1),
                value: 20,
            },
            Instruction::Return { src: Some(Reg(1)) },
        ];

        let cfg = CFG::from_flat_instructions(&instructions);

        assert_eq!(cfg.entry_block, 0);
        // Block 0: LoadInt, JumpIf. Ends with Branch to block 2 (target 4) and block 1 (fallthrough).
        let b0 = cfg.blocks.get(&0).unwrap();
        assert_eq!(b0.instructions.len(), 1);
        assert!(matches!(
            b0.terminator,
            Terminator::Branch {
                then_block: 2,
                else_block: 1,
                ..
            }
        ));

        // Block 1: LoadInt, Jump. Ends with Jump to block 3 (target 5).
        let b1 = cfg.blocks.get(&1).unwrap();
        assert_eq!(b1.instructions.len(), 1);
        assert!(matches!(b1.terminator, Terminator::Jump { target: 3 }));

        // Block 2: LoadInt. Ends with fallthrough Jump to block 3.
        let b2 = cfg.blocks.get(&2).unwrap();
        assert_eq!(b2.instructions.len(), 1);
        assert!(matches!(b2.terminator, Terminator::Jump { target: 3 }));

        // Block 3: Return.
        let b3 = cfg.blocks.get(&3).unwrap();
        assert_eq!(b3.instructions.len(), 0);
        assert!(matches!(
            b3.terminator,
            Terminator::Return { src: Some(Reg(1)) }
        ));
    }
}
