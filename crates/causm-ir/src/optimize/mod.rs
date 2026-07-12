pub mod coalescing;
pub mod constant_prop;
pub mod dead_code;
pub(crate) mod utils;

#[cfg(test)]
mod tests;

use crate::cfg::{self, BlockId, Terminator, CFG};
use crate::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator, SsaTransformer};
use crate::{Instruction, IrProgram, Reg};
use std::collections::{HashMap, HashSet};

use coalescing::BlockCoalescingPass;
use constant_prop::ConstantPropagationPass;
use dead_code::DeadCodeEliminationPass;

pub fn optimize_program(mut ir: IrProgram) -> IrProgram {
    let mut manager = PassManager::new();
    manager.add_pass(Box::new(ConstantPropagationPass));
    manager.add_pass(Box::new(BlockCoalescingPass));
    manager.add_pass(Box::new(DeadCodeEliminationPass));

    // 1. Optimize routines (routines are self-contained, no global usage tracking needed)
    let empty_set = HashSet::new();
    for routine in ir.routines.values_mut() {
        if !routine.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&routine.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let mut ssa_cfg = ssa_transformer.transform();

            manager.run(&mut ssa_cfg, &empty_set, true);

            let destructed_cfg = destruct_ssa(ssa_cfg);
            routine.instructions = flatten_cfg(&destructed_cfg);
        }
    }

    // 2. Optimize blocks
    for block in ir.blocks.iter_mut() {
        if !block.instructions.is_empty() {
            // Analyze the block to collect all read registers inside it (to feed globally_used_regs).
            // Any register that is read before it is defined in the block (or is a parameter) must be preserved.
            let cfg = CFG::from_flat_instructions(&block.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let ssa_cfg = ssa_transformer.transform();

            let mut defined = HashSet::new();
            let mut globally_used_regs = HashSet::new();

            for ssa_block in ssa_cfg.blocks.values() {
                for phi in &ssa_block.phi_nodes {
                    defined.insert(phi.dest.reg);
                    for (_, incoming_reg) in &phi.incoming {
                        if !defined.contains(&incoming_reg.reg) {
                            globally_used_regs.insert(incoming_reg.reg);
                        }
                    }
                }
                for instr in &ssa_block.instructions {
                    if let Some(dest) = utils::get_ssa_dest_reg(instr) {
                        defined.insert(dest.reg);
                    }
                    utils::for_each_ssa_src_reg(instr, &mut |src| {
                        if !defined.contains(&src.reg) {
                            globally_used_regs.insert(src.reg);
                        }
                    });
                }
                utils::for_each_ssa_term_src_reg(
                    &ssa_block.terminator,
                    &mut |src| {
                        if !defined.contains(&src.reg) {
                            globally_used_regs.insert(src.reg);
                        }
                    },
                );
            }

            let cfg = CFG::from_flat_instructions(&block.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let mut ssa_cfg = ssa_transformer.transform();

            manager.run(&mut ssa_cfg, &globally_used_regs, false);

            let destructed_cfg = destruct_ssa(ssa_cfg);
            block.instructions = flatten_cfg(&destructed_cfg);
        }
    }

    ir
}

pub trait OptimizationPass {
    fn name(&self) -> &str;
    fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        globally_used_regs: &HashSet<u32>,
        is_routine: bool,
    ) -> bool;
}

#[derive(Default)]
pub struct PassManager {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl PassManager {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub fn add_pass(&mut self, pass: Box<dyn OptimizationPass>) {
        self.passes.push(pass);
    }

    pub fn run(
        &self,
        ssa_cfg: &mut SsaCFG,
        globally_used_regs: &HashSet<u32>,
        is_routine: bool,
    ) {
        let mut changed = true;
        let mut iterations = 0;
        while changed && iterations < 10 {
            changed = false;
            for pass in &self.passes {
                if pass.run(ssa_cfg, globally_used_regs, is_routine) {
                    changed = true;
                }
            }
            iterations += 1;
        }
    }
}

fn ssa_reg_to_reg(r: SsaReg) -> Reg {
    Reg(r.reg)
}

fn ssa_instr_to_instr(ssa_instr: &SsaInstruction) -> Instruction {
    match ssa_instr {
        SsaInstruction::BinaryOp {
            dest,
            op,
            left,
            right,
        } => Instruction::BinaryOp {
            dest: ssa_reg_to_reg(*dest),
            op: *op,
            left: ssa_reg_to_reg(*left),
            right: ssa_reg_to_reg(*right),
        },
        SsaInstruction::UnaryOp { dest, op, src } => Instruction::UnaryOp {
            dest: ssa_reg_to_reg(*dest),
            op: *op,
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::LoadInt { dest, value } => Instruction::LoadInt {
            dest: ssa_reg_to_reg(*dest),
            value: *value,
        },
        SsaInstruction::LoadFloat { dest, value } => Instruction::LoadFloat {
            dest: ssa_reg_to_reg(*dest),
            value: *value,
        },
        SsaInstruction::LoadBool { dest, value } => Instruction::LoadBool {
            dest: ssa_reg_to_reg(*dest),
            value: *value,
        },
        SsaInstruction::LoadString { dest, value } => Instruction::LoadString {
            dest: ssa_reg_to_reg(*dest),
            value: value.clone(),
        },
        SsaInstruction::LoadNull { dest } => Instruction::LoadNull {
            dest: ssa_reg_to_reg(*dest),
        },
        SsaInstruction::Move { dest, src } => Instruction::Move {
            dest: ssa_reg_to_reg(*dest),
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::Consume { src } => Instruction::Consume {
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::ConsumeField { src, field } => Instruction::ConsumeField {
            src: ssa_reg_to_reg(*src),
            field: field.clone(),
        },
        SsaInstruction::ConsumeFieldDynamic { target, index } => {
            Instruction::ConsumeFieldDynamic {
                target: ssa_reg_to_reg(*target),
                index: ssa_reg_to_reg(*index),
            }
        }
        SsaInstruction::Clone { dest, src } => Instruction::Clone {
            dest: ssa_reg_to_reg(*dest),
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::Call {
            routine,
            args,
            dest,
        } => Instruction::Call {
            routine: routine.clone(),
            args: args.iter().copied().map(ssa_reg_to_reg).collect(),
            dest: ssa_reg_to_reg(*dest),
        },
        SsaInstruction::DynamicCall {
            method,
            args,
            dest,
            budget,
        } => Instruction::DynamicCall {
            method: method.clone(),
            args: args.iter().copied().map(ssa_reg_to_reg).collect(),
            dest: ssa_reg_to_reg(*dest),
            budget: *budget,
        },
        SsaInstruction::TypeAssert {
            dest,
            src,
            type_name,
        } => Instruction::TypeAssert {
            dest: ssa_reg_to_reg(*dest),
            src: ssa_reg_to_reg(*src),
            type_name: type_name.clone(),
        },
        SsaInstruction::StructLit {
            dest,
            fields,
            type_name,
        } => {
            let mut fields_mapped = HashMap::new();
            for (k, v) in fields {
                fields_mapped.insert(k.clone(), ssa_reg_to_reg(*v));
            }
            Instruction::StructLit {
                dest: ssa_reg_to_reg(*dest),
                fields: fields_mapped,
                type_name: type_name.clone(),
            }
        }
        SsaInstruction::TopologyLit { dest, fields } => {
            let mut fields_mapped = HashMap::new();
            for (k, v) in fields {
                fields_mapped.insert(k.clone(), ssa_reg_to_reg(*v));
            }
            Instruction::TopologyLit {
                dest: ssa_reg_to_reg(*dest),
                fields: fields_mapped,
            }
        }
        SsaInstruction::ArrayLit { dest, elements } => Instruction::ArrayLit {
            dest: ssa_reg_to_reg(*dest),
            elements: elements.iter().copied().map(ssa_reg_to_reg).collect(),
        },
        SsaInstruction::FieldAccess {
            dest,
            target,
            field,
        } => Instruction::FieldAccess {
            dest: ssa_reg_to_reg(*dest),
            target: ssa_reg_to_reg(*target),
            field: field.clone(),
        },
        SsaInstruction::FieldUpdate {
            target, field, src, ..
        } => Instruction::FieldUpdate {
            target: ssa_reg_to_reg(*target),
            field: field.clone(),
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::IndexAccess {
            dest,
            target,
            index,
        } => Instruction::IndexAccess {
            dest: ssa_reg_to_reg(*dest),
            target: ssa_reg_to_reg(*target),
            index: ssa_reg_to_reg(*index),
        },
        SsaInstruction::IndexFieldUpdate {
            target,
            index,
            field,
            src,
            ..
        } => Instruction::IndexFieldUpdate {
            target: ssa_reg_to_reg(*target),
            index: ssa_reg_to_reg(*index),
            field: field.clone(),
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::AssertTime { op, limit_ms } => Instruction::AssertTime {
            op: *op,
            limit_ms: *limit_ms,
        },
        SsaInstruction::Capability { cap } => {
            Instruction::Capability { cap: cap.clone() }
        }
        SsaInstruction::For {
            item_name,
            mode,
            source,
            body,
            pacing_ms,
            max_ms,
        } => {
            let body_mapped = body.iter().map(ssa_instr_to_instr).collect();
            Instruction::For {
                item_name: item_name.clone(),
                mode: mode.clone(),
                source: ssa_reg_to_reg(*source),
                body: body_mapped,
                pacing_ms: *pacing_ms,
                max_ms: *max_ms,
            }
        }
        SsaInstruction::SplitMap {
            item_name,
            mode,
            source,
            body,
            reconcile,
        } => {
            let body_mapped = body.iter().map(ssa_instr_to_instr).collect();
            Instruction::SplitMap {
                item_name: item_name.clone(),
                mode: mode.clone(),
                source: ssa_reg_to_reg(*source),
                body: body_mapped,
                reconcile: reconcile.clone(),
            }
        }
        SsaInstruction::Defer {
            dest,
            cap,
            deadline_ms,
        } => Instruction::Defer {
            dest: ssa_reg_to_reg(*dest),
            cap: cap.clone(),
            deadline_ms: *deadline_ms,
        },
        SsaInstruction::Await { target } => Instruction::Await {
            target: ssa_reg_to_reg(*target),
        },
        SsaInstruction::Loop { max_ms } => Instruction::Loop { max_ms: *max_ms },
        SsaInstruction::EndLoop { max_ms } => {
            Instruction::EndLoop { max_ms: *max_ms }
        }
        SsaInstruction::Break => Instruction::Break,
        SsaInstruction::LoopTick => Instruction::LoopTick,
        SsaInstruction::EndLoopTick => Instruction::EndLoopTick,
        SsaInstruction::NetworkRequest { domain } => Instruction::NetworkRequest {
            domain: domain.clone(),
        },
        SsaInstruction::Print { src } => Instruction::Print {
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::Debug { src } => Instruction::Debug {
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::Slice { ms } => Instruction::Slice { ms: *ms },
        SsaInstruction::Isolate { name, manifest } => Instruction::Isolate {
            name: name.clone(),
            manifest: manifest.clone(),
        },
        SsaInstruction::EndIsolate => Instruction::EndIsolate,
        SsaInstruction::Lease {
            target_reg,
            source_reg,
            duration_ms,
        } => Instruction::Lease {
            target_reg: ssa_reg_to_reg(*target_reg),
            source_reg: ssa_reg_to_reg(*source_reg),
            duration_ms: *duration_ms,
        },
        SsaInstruction::EndLease {
            source_reg,
            duration_ms,
        } => Instruction::EndLease {
            source_reg: ssa_reg_to_reg(*source_reg),
            duration_ms: *duration_ms,
        },
        SsaInstruction::Split { parent, branches } => Instruction::Split {
            parent: parent.clone(),
            branches: branches.clone(),
        },
        SsaInstruction::Merge {
            branches,
            target,
            resolution,
        } => Instruction::Merge {
            branches: branches.clone(),
            target: target.clone(),
            resolution: resolution.clone(),
        },
        SsaInstruction::Anchor { name } => {
            Instruction::Anchor { name: name.clone() }
        }
        SsaInstruction::Rewind { target, anchor } => Instruction::Rewind {
            target: target.clone(),
            anchor: anchor.clone(),
        },
        SsaInstruction::Commit { vars } => {
            Instruction::Commit { vars: vars.clone() }
        }
        SsaInstruction::Watchdog {
            target,
            timeout_ms,
            recovery_jump,
        } => Instruction::Watchdog {
            target: target.clone(),
            timeout_ms: *timeout_ms,
            recovery_jump: *recovery_jump,
        },
        SsaInstruction::Speculate {
            max_ms,
            fallback_target,
        } => Instruction::Speculate {
            max_ms: *max_ms,
            fallback_target: *fallback_target,
        },
        SsaInstruction::EndSpeculate {
            max_ms,
            fallback_target,
        } => Instruction::EndSpeculate {
            max_ms: *max_ms,
            fallback_target: *fallback_target,
        },
        SsaInstruction::Collapse => Instruction::Collapse,
        SsaInstruction::RelativisticBlock {
            target,
            block_pc,
            block_len,
        } => Instruction::RelativisticBlock {
            target: target.clone(),
            block_pc: *block_pc,
            block_len: *block_len,
        },
        SsaInstruction::SpeculationMode { mode } => {
            Instruction::SpeculationMode { mode: *mode }
        }
        SsaInstruction::OpenChan { name, capacity } => Instruction::OpenChan {
            name: name.clone(),
            capacity: *capacity,
        },
        SsaInstruction::ChanSend { chan_id, src } => Instruction::ChanSend {
            chan_id: chan_id.clone(),
            src: ssa_reg_to_reg(*src),
        },
        SsaInstruction::ChanRecv { dest, chan_id } => Instruction::ChanRecv {
            dest: ssa_reg_to_reg(*dest),
            chan_id: chan_id.clone(),
        },
        SsaInstruction::Entangle { regs } => Instruction::Entangle {
            regs: regs.iter().copied().map(ssa_reg_to_reg).collect(),
        },
        SsaInstruction::MatchEntropy {
            target,
            valid_target,
            decayed_target,
            pending_target,
            consumed_target,
        } => Instruction::MatchEntropy {
            target: ssa_reg_to_reg(*target),
            valid_target: *valid_target,
            decayed_target: *decayed_target,
            pending_target: *pending_target,
            consumed_target: *consumed_target,
        },
        SsaInstruction::Select {
            max_ms,
            cases,
            timeout_target,
        } => {
            let cases_mapped = cases
                .iter()
                .map(|c| crate::IrSelectCase {
                    chan_id: c.chan_id.clone(),
                    dest: ssa_reg_to_reg(c.dest),
                    target: c.target,
                })
                .collect();
            Instruction::Select {
                max_ms: *max_ms,
                cases: cases_mapped,
                timeout_target: *timeout_target,
            }
        }
        SsaInstruction::AwaitChan { chan_id } => Instruction::AwaitChan {
            chan_id: chan_id.clone(),
        },
        SsaInstruction::Other(s) => panic!("Cannot lower Other instruction: {}", s),
    }
}

pub fn destruct_ssa(ssa_cfg: SsaCFG) -> CFG {
    let mut blocks = HashMap::new();

    // 1. First, strip SSA formatting and convert to normal Basic Blocks.
    for (&id, ssa_block) in &ssa_cfg.blocks {
        let mut instrs = Vec::new();
        for ssa_instr in &ssa_block.instructions {
            instrs.push(ssa_instr_to_instr(ssa_instr));
        }

        let term = match &ssa_block.terminator {
            SsaTerminator::Jump { target } => Terminator::Jump { target: *target },
            SsaTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => Terminator::Branch {
                cond: ssa_reg_to_reg(*cond),
                then_block: *then_block,
                else_block: *else_block,
            },
            SsaTerminator::Return { src } => Terminator::Return {
                src: src.map(ssa_reg_to_reg),
            },
            SsaTerminator::MatchEntropy {
                target,
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
            } => Terminator::MatchEntropy {
                target: ssa_reg_to_reg(*target),
                valid_block: *valid_block,
                decayed_block: *decayed_block,
                pending_block: *pending_block,
                consumed_block: *consumed_block,
            },
            SsaTerminator::Select {
                max_ms,
                cases,
                timeout_block,
            } => {
                let cases_mapped = cases
                    .iter()
                    .map(|c| cfg::SelectCase {
                        chan_id: c.chan_id.clone(),
                        dest: ssa_reg_to_reg(c.dest),
                        target_block: c.target as BlockId,
                    })
                    .collect();
                Terminator::Select {
                    max_ms: *max_ms,
                    cases: cases_mapped,
                    timeout_block: *timeout_block,
                }
            }
            SsaTerminator::Unreachable => Terminator::Unreachable,
        };

        blocks.insert(
            id,
            cfg::BasicBlock {
                id,
                instructions: instrs,
                terminator: term,
            },
        );
    }

    // 2. Insert copy/move instructions before block terminators to satisfy Phi incoming parameters.
    for ssa_block in ssa_cfg.blocks.values() {
        for phi in &ssa_block.phi_nodes {
            for &(pred_id, incoming_reg) in &phi.incoming {
                if phi.original_reg.0 != incoming_reg.reg {
                    if let Some(pred_block) = blocks.get_mut(&pred_id) {
                        pred_block.instructions.push(Instruction::Move {
                            dest: phi.original_reg,
                            src: ssa_reg_to_reg(incoming_reg),
                        });
                    }
                }
            }
        }
    }

    CFG {
        entry_block: ssa_cfg.entry_block,
        blocks,
        original_pc_to_block_id: ssa_cfg.original_pc_to_block_id,
    }
}

enum FixupTarget {
    Jump {
        instr_idx: usize,
        target_block: BlockId,
    },
    JumpIf {
        instr_idx: usize,
        target_block: BlockId,
    },
    RelativisticBlock {
        instr_idx: usize,
        body_block: BlockId,
        end_block: BlockId,
    },
    Watchdog {
        instr_idx: usize,
        recovery_block: BlockId,
    },
    Speculate {
        instr_idx: usize,
        fallback_block: BlockId,
    },
    EndSpeculate {
        instr_idx: usize,
        fallback_block: BlockId,
    },
    MatchEntropy {
        instr_idx: usize,
        valid_block: Option<BlockId>,
        decayed_block: Option<BlockId>,
        pending_block: Option<BlockId>,
        consumed_block: Option<BlockId>,
    },
    Select {
        instr_idx: usize,
        cases: Vec<BlockId>,
        timeout_block: Option<BlockId>,
    },
}

pub fn flatten_cfg(cfg: &CFG) -> Vec<Instruction> {
    let mut block_ids: Vec<BlockId> = cfg.blocks.keys().copied().collect();
    block_ids.sort();

    let mut instructions = Vec::new();
    let mut block_offsets = HashMap::new();
    let mut fixup_targets = Vec::new();

    for &id in &block_ids {
        block_offsets.insert(id, instructions.len());
        let block = &cfg.blocks[&id];

        // Append standard block instructions, checking for internal PC targets to fix up
        for instr in &block.instructions {
            let global_idx = instructions.len();
            match instr {
                Instruction::RelativisticBlock {
                    block_pc,
                    block_len,
                    ..
                } => {
                    let body_block = cfg.original_pc_to_block_id[block_pc];
                    let end_block =
                        cfg.original_pc_to_block_id[&(block_pc + block_len)];
                    fixup_targets.push(FixupTarget::RelativisticBlock {
                        instr_idx: global_idx,
                        body_block,
                        end_block,
                    });
                    instructions.push(instr.clone());
                }
                Instruction::Watchdog { recovery_jump, .. } => {
                    if let Some(t) = recovery_jump {
                        let recovery_block = cfg.original_pc_to_block_id[t];
                        fixup_targets.push(FixupTarget::Watchdog {
                            instr_idx: global_idx,
                            recovery_block,
                        });
                    }
                    instructions.push(instr.clone());
                }
                Instruction::Speculate {
                    fallback_target, ..
                } => {
                    let fallback_block =
                        cfg.original_pc_to_block_id[fallback_target];
                    fixup_targets.push(FixupTarget::Speculate {
                        instr_idx: global_idx,
                        fallback_block,
                    });
                    instructions.push(instr.clone());
                }
                Instruction::EndSpeculate {
                    fallback_target, ..
                } => {
                    let fallback_block =
                        cfg.original_pc_to_block_id[fallback_target];
                    fixup_targets.push(FixupTarget::EndSpeculate {
                        instr_idx: global_idx,
                        fallback_block,
                    });
                    instructions.push(instr.clone());
                }
                _ => {
                    instructions.push(instr.clone());
                }
            }
        }

        // Translate the block terminator to flat instructions
        match &block.terminator {
            Terminator::Jump { target } => {
                let global_idx = instructions.len();
                fixup_targets.push(FixupTarget::Jump {
                    instr_idx: global_idx,
                    target_block: *target,
                });
                instructions.push(Instruction::Jump { target: 999999 });
            }
            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let global_idx1 = instructions.len();
                fixup_targets.push(FixupTarget::JumpIf {
                    instr_idx: global_idx1,
                    target_block: *then_block,
                });
                instructions.push(Instruction::JumpIf {
                    cond: *cond,
                    target: 999999,
                });

                let global_idx2 = instructions.len();
                fixup_targets.push(FixupTarget::Jump {
                    instr_idx: global_idx2,
                    target_block: *else_block,
                });
                instructions.push(Instruction::Jump { target: 999999 });
            }
            Terminator::Return { src } => {
                instructions.push(Instruction::Return { src: *src });
            }
            Terminator::MatchEntropy {
                target,
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
            } => {
                let global_idx = instructions.len();
                fixup_targets.push(FixupTarget::MatchEntropy {
                    instr_idx: global_idx,
                    valid_block: *valid_block,
                    decayed_block: *decayed_block,
                    pending_block: *pending_block,
                    consumed_block: *consumed_block,
                });
                instructions.push(Instruction::MatchEntropy {
                    target: *target,
                    valid_target: valid_block.map(|_| 999999),
                    decayed_target: decayed_block.map(|_| 999999),
                    pending_target: pending_block.map(|_| 999999),
                    consumed_target: consumed_block.map(|_| 999999),
                });
            }
            Terminator::Select {
                max_ms,
                cases,
                timeout_block,
            } => {
                let global_idx = instructions.len();
                let case_blocks = cases.iter().map(|c| c.target_block).collect();
                fixup_targets.push(FixupTarget::Select {
                    instr_idx: global_idx,
                    cases: case_blocks,
                    timeout_block: *timeout_block,
                });
                let s_cases = cases
                    .iter()
                    .map(|c| crate::IrSelectCase {
                        chan_id: c.chan_id.clone(),
                        dest: c.dest,
                        target: 999999,
                    })
                    .collect();
                instructions.push(Instruction::Select {
                    max_ms: *max_ms,
                    cases: s_cases,
                    timeout_target: timeout_block.map(|_| 999999),
                });
            }
            Terminator::Unreachable => {}
        }
    }

    // Second pass: resolve placeholders using block_offsets
    for target in fixup_targets {
        match target {
            FixupTarget::Jump {
                instr_idx,
                target_block,
            } => {
                if let Instruction::Jump { ref mut target } = instructions[instr_idx]
                {
                    *target = block_offsets[&target_block];
                }
            }
            FixupTarget::JumpIf {
                instr_idx,
                target_block,
            } => {
                if let Instruction::JumpIf { ref mut target, .. } =
                    instructions[instr_idx]
                {
                    *target = block_offsets[&target_block];
                }
            }
            FixupTarget::RelativisticBlock {
                instr_idx,
                body_block,
                end_block,
            } => {
                let start_pc = block_offsets[&body_block];
                let end_pc = block_offsets[&end_block];
                if let Instruction::RelativisticBlock {
                    ref mut block_pc,
                    ref mut block_len,
                    ..
                } = instructions[instr_idx]
                {
                    *block_pc = start_pc;
                    *block_len = end_pc - start_pc;
                }
            }
            FixupTarget::Watchdog {
                instr_idx,
                recovery_block,
            } => {
                let pc = block_offsets[&recovery_block];
                if let Instruction::Watchdog {
                    ref mut recovery_jump,
                    ..
                } = instructions[instr_idx]
                {
                    *recovery_jump = Some(pc);
                }
            }
            FixupTarget::Speculate {
                instr_idx,
                fallback_block,
            } => {
                let pc = block_offsets[&fallback_block];
                if let Instruction::Speculate {
                    ref mut fallback_target,
                    ..
                } = instructions[instr_idx]
                {
                    *fallback_target = pc;
                }
            }
            FixupTarget::EndSpeculate {
                instr_idx,
                fallback_block,
            } => {
                let pc = block_offsets[&fallback_block];
                if let Instruction::EndSpeculate {
                    ref mut fallback_target,
                    ..
                } = instructions[instr_idx]
                {
                    *fallback_target = pc;
                }
            }
            FixupTarget::MatchEntropy {
                instr_idx,
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
            } => {
                if let Instruction::MatchEntropy {
                    ref mut valid_target,
                    ref mut decayed_target,
                    ref mut pending_target,
                    ref mut consumed_target,
                    ..
                } = instructions[instr_idx]
                {
                    if let Some(t) = valid_block {
                        *valid_target = Some(block_offsets[&t]);
                    }
                    if let Some(t) = decayed_block {
                        *decayed_target = Some(block_offsets[&t]);
                    }
                    if let Some(t) = pending_block {
                        *pending_target = Some(block_offsets[&t]);
                    }
                    if let Some(t) = consumed_block {
                        *consumed_target = Some(block_offsets[&t]);
                    }
                }
            }
            FixupTarget::Select {
                instr_idx,
                cases,
                timeout_block,
            } => {
                if let Instruction::Select {
                    cases: ref mut s_cases,
                    ref mut timeout_target,
                    ..
                } = instructions[instr_idx]
                {
                    for (i, case_block) in cases.into_iter().enumerate() {
                        s_cases[i].target = block_offsets[&case_block];
                    }
                    if let Some(t) = timeout_block {
                        *timeout_target = Some(block_offsets[&t]);
                    }
                }
            }
        }
    }

    instructions
}
