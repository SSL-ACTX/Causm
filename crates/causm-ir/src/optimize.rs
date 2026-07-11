use crate::cfg::{self, BlockId, Terminator, CFG};
use crate::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator, SsaTransformer};
use crate::{Instruction, IrProgram, Reg};
use std::collections::{HashMap, HashSet};

pub fn optimize_program(mut ir: IrProgram) -> IrProgram {
    // 1. Optimize routines (routines are self-contained, no global usage tracking needed)
    let empty_set = HashSet::new();
    for routine in ir.routines.values_mut() {
        if !routine.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&routine.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let mut ssa_cfg = ssa_transformer.transform();

            dead_code_elimination(&mut ssa_cfg, &empty_set, true);

            let destructed_cfg = destruct_ssa(ssa_cfg);
            routine.instructions = flatten_cfg(&destructed_cfg);
        }
    }

    // 2. Scan all timeline blocks to build a set of registers used anywhere in the timeline
    let mut globally_used_regs = HashSet::new();
    for block in &ir.blocks {
        if !block.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&block.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let ssa_cfg = ssa_transformer.transform();

            for ssa_block in ssa_cfg.blocks.values() {
                for phi in &ssa_block.phi_nodes {
                    for (_, incoming_reg) in &phi.incoming {
                        globally_used_regs.insert(incoming_reg.reg);
                    }
                }
                for instr in &ssa_block.instructions {
                    for_each_ssa_src_reg(instr, &mut |r| {
                        globally_used_regs.insert(r.reg);
                    });
                }
                for_each_ssa_term_src_reg(&ssa_block.terminator, &mut |r| {
                    globally_used_regs.insert(r.reg);
                });
            }
        }
    }

    // 3. Optimize timeline blocks with the global usage set
    for block in &mut ir.blocks {
        if !block.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&block.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let mut ssa_cfg = ssa_transformer.transform();

            dead_code_elimination(&mut ssa_cfg, &globally_used_regs, false);

            let destructed_cfg = destruct_ssa(ssa_cfg);
            block.instructions = flatten_cfg(&destructed_cfg);
        }
    }

    ir
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

fn get_ssa_dest_reg(instr: &SsaInstruction) -> Option<SsaReg> {
    match instr {
        SsaInstruction::BinaryOp { dest, .. }
        | SsaInstruction::UnaryOp { dest, .. }
        | SsaInstruction::LoadInt { dest, .. }
        | SsaInstruction::LoadFloat { dest, .. }
        | SsaInstruction::LoadBool { dest, .. }
        | SsaInstruction::LoadString { dest, .. }
        | SsaInstruction::LoadNull { dest }
        | SsaInstruction::Move { dest, .. }
        | SsaInstruction::Clone { dest, .. }
        | SsaInstruction::Call { dest, .. }
        | SsaInstruction::StructLit { dest, .. }
        | SsaInstruction::TopologyLit { dest, .. }
        | SsaInstruction::ArrayLit { dest, .. }
        | SsaInstruction::FieldAccess { dest, .. }
        | SsaInstruction::IndexAccess { dest, .. }
        | SsaInstruction::Defer { dest, .. } => Some(*dest),

        SsaInstruction::FieldUpdate { target, .. }
        | SsaInstruction::IndexFieldUpdate { target, .. } => Some(*target),

        _ => None,
    }
}

fn for_each_ssa_src_reg(instr: &SsaInstruction, f: &mut impl FnMut(SsaReg)) {
    match instr {
        SsaInstruction::BinaryOp { left, right, .. } => {
            f(*left);
            f(*right);
        }
        SsaInstruction::UnaryOp { src, .. } => {
            f(*src);
        }
        SsaInstruction::Move { src, .. } => {
            f(*src);
        }
        SsaInstruction::Consume { src } => {
            f(*src);
        }
        SsaInstruction::ConsumeField { src, .. } => {
            f(*src);
        }
        SsaInstruction::ConsumeFieldDynamic { target, index } => {
            f(*target);
            f(*index);
        }
        SsaInstruction::Clone { src, .. } => {
            f(*src);
        }
        SsaInstruction::Call { args, .. } => {
            for a in args {
                f(*a);
            }
        }
        SsaInstruction::StructLit { fields, .. } => {
            for v in fields.values() {
                f(*v);
            }
        }
        SsaInstruction::TopologyLit { fields, .. } => {
            for v in fields.values() {
                f(*v);
            }
        }
        SsaInstruction::ArrayLit { elements, .. } => {
            for e in elements {
                f(*e);
            }
        }
        SsaInstruction::FieldAccess { target, .. } => {
            f(*target);
        }
        SsaInstruction::FieldUpdate {
            old_target, src, ..
        } => {
            f(*old_target);
            f(*src);
        }
        SsaInstruction::IndexAccess { target, index, .. } => {
            f(*target);
            f(*index);
        }
        SsaInstruction::IndexFieldUpdate {
            old_target,
            index,
            src,
            ..
        } => {
            f(*old_target);
            f(*index);
            f(*src);
        }
        SsaInstruction::For { source, body, .. } => {
            f(*source);
            for b in body {
                for_each_ssa_src_reg(b, f);
            }
        }
        SsaInstruction::SplitMap { source, body, .. } => {
            f(*source);
            for b in body {
                for_each_ssa_src_reg(b, f);
            }
        }
        SsaInstruction::Await { target } => {
            f(*target);
        }
        SsaInstruction::Print { src } => {
            f(*src);
        }
        SsaInstruction::Debug { src } => {
            f(*src);
        }
        SsaInstruction::ChanSend { src, .. } => {
            f(*src);
        }
        SsaInstruction::Lease { source_reg, .. } => {
            f(*source_reg);
        }
        SsaInstruction::EndLease { source_reg, .. } => {
            f(*source_reg);
        }
        SsaInstruction::Entangle { regs } => {
            for &r in regs {
                f(r);
            }
        }
        SsaInstruction::MatchEntropy { target, .. } => {
            f(*target);
        }
        _ => {}
    }
}

fn for_each_ssa_term_src_reg(term: &SsaTerminator, f: &mut impl FnMut(SsaReg)) {
    match term {
        SsaTerminator::Branch { cond, .. } => {
            f(*cond);
        }
        SsaTerminator::Return { src: Some(r) } => {
            f(*r);
        }
        SsaTerminator::MatchEntropy { target, .. } => {
            f(*target);
        }
        _ => {}
    }
}

fn has_side_effects(instr: &SsaInstruction) -> bool {
    !matches!(
        instr,
        SsaInstruction::BinaryOp { .. }
            | SsaInstruction::UnaryOp { .. }
            | SsaInstruction::LoadInt { .. }
            | SsaInstruction::LoadFloat { .. }
            | SsaInstruction::LoadBool { .. }
            | SsaInstruction::LoadString { .. }
            | SsaInstruction::LoadNull { .. }
            | SsaInstruction::Move { .. }
            | SsaInstruction::Clone { .. }
            | SsaInstruction::StructLit { .. }
            | SsaInstruction::TopologyLit { .. }
            | SsaInstruction::ArrayLit { .. }
            | SsaInstruction::FieldAccess { .. }
            | SsaInstruction::IndexAccess { .. }
            | SsaInstruction::FieldUpdate { .. }
            | SsaInstruction::IndexFieldUpdate { .. }
    )
}

pub fn dead_code_elimination(
    ssa_cfg: &mut SsaCFG,
    globally_used_regs: &HashSet<u32>,
    is_routine: bool,
) {
    let mut use_counts: HashMap<SsaReg, usize> = HashMap::new();
    let mut count_uses = |r: SsaReg| {
        *use_counts.entry(r).or_insert(0) += 1;
    };

    for block in ssa_cfg.blocks.values() {
        for phi in &block.phi_nodes {
            for (_, incoming_reg) in &phi.incoming {
                count_uses(*incoming_reg);
            }
        }
        for instr in &block.instructions {
            for_each_ssa_src_reg(instr, &mut count_uses);
        }
        for_each_ssa_term_src_reg(&block.terminator, &mut count_uses);
    }

    let mut changed = true;
    while changed {
        changed = false;
        for block in ssa_cfg.blocks.values_mut() {
            let mut i = 0;
            while i < block.instructions.len() {
                let instr = &block.instructions[i];
                if let Some(dest) = get_ssa_dest_reg(instr) {
                    let mut is_used =
                        use_counts.get(&dest).copied().unwrap_or(0) > 0;
                    if !is_routine && globally_used_regs.contains(&dest.reg) {
                        is_used = true;
                    }
                    if !is_used && !has_side_effects(instr) {
                        for_each_ssa_src_reg(instr, &mut |r| {
                            if let Some(count) = use_counts.get_mut(&r) {
                                if *count > 0 {
                                    *count -= 1;
                                }
                            }
                        });
                        block.instructions.remove(i);
                        changed = true;
                        continue;
                    }
                }
                i += 1;
            }
        }
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
                        target_block: c.target as u32,
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
