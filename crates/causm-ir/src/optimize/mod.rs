pub mod cfg_simp;
pub mod channels;
pub mod coalescing;
pub mod concurrency;
pub mod constant_prop;
pub mod copy_prop;
pub mod dead_code;
pub mod entropy;
pub mod lease;
pub(crate) mod utils;
pub mod verifier;

#[cfg(test)]
mod tests;

use crate::cfg::{self, BlockId, Terminator, CFG};
use crate::ssa::{SsaCFG, SsaInstruction, SsaReg, SsaTerminator, SsaTransformer};
use crate::{Instruction, IrProgram, Reg};
use std::collections::{HashMap, HashSet};

use cfg_simp::CfgSimplificationPass;
use channels::ChannelLivenessPass;
use coalescing::BlockCoalescingPass;
use concurrency::ConcurrencyAnalysisPass;
use constant_prop::ConstantPropagationPass;
use copy_prop::CopyPropagationPass;
use dead_code::DeadCodeEliminationPass;
use entropy::EntropyOptimizationPass;
use lease::LeaseOptimizationPass;
use verifier::VerifierPass;

pub use dead_code::{prune_import_duplicates, prune_unreachable_routines};

pub fn optimize_program(mut ir: IrProgram) -> IrProgram {
    // 0. Prune uncalled / unreachable routines (tree shaking)
    dead_code::prune_unreachable_routines(&mut ir);

    // Pre-scan all IR blocks and routines to build a global set of channel names
    //    that are referenced (send/recv/await/loop-tick/select) anywhere in the program.
    //    This is required so that ChannelLivenessPass never eliminates an `OpenChan`
    //    whose uses happen to live in a *different* temporal block.
    let globally_referenced_channels = collect_globally_referenced_channels(&ir);

    let mut manager = PassManager::new();
    manager.add_pass(Box::new(ConstantPropagationPass));
    manager.add_pass(Box::new(CopyPropagationPass));
    manager.add_pass(Box::new(CfgSimplificationPass));
    manager.add_pass(Box::new(EntropyOptimizationPass));
    manager.add_pass(Box::new(ChannelLivenessPass::new(
        globally_referenced_channels,
    )));
    manager.add_pass(Box::new(LeaseOptimizationPass));
    manager.add_pass(Box::new(ConcurrencyAnalysisPass));
    manager.add_pass(Box::new(BlockCoalescingPass));
    manager.add_pass(Box::new(DeadCodeEliminationPass));
    manager.add_pass(Box::new(VerifierPass));

    // 1. Optimize routines (routines are self-contained, no global usage tracking needed)
    for routine in ir.routines.values_mut() {
        if !routine.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&routine.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let ssa_cfg = ssa_transformer.transform();

            let destructed_cfg = destruct_ssa(ssa_cfg);
            routine.instructions = flatten_cfg(&destructed_cfg);
        }
    }

    // 2. Scan all blocks to build a global set of registers that are read before being defined in their blocks.
    let mut global_preserved_regs = HashSet::new();
    for reg in ir.symbols.values() {
        global_preserved_regs.insert(reg.0);
    }
    for block in &ir.blocks {
        if !block.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&block.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let ssa_cfg = ssa_transformer.transform();

            let mut defined = HashSet::new();
            for ssa_block in ssa_cfg.blocks.values() {
                for phi in &ssa_block.phi_nodes {
                    defined.insert(phi.dest.reg);
                    for (_, incoming_reg) in &phi.incoming {
                        if !defined.contains(&incoming_reg.reg) {
                            global_preserved_regs.insert(incoming_reg.reg);
                        }
                    }
                }
                for instr in &ssa_block.instructions {
                    if let Some(dest) = utils::get_ssa_dest_reg(instr) {
                        defined.insert(dest.reg);
                    }
                    utils::for_each_ssa_src_reg(instr, &mut |src| {
                        if !defined.contains(&src.reg) {
                            global_preserved_regs.insert(src.reg);
                        }
                    });
                }
                utils::for_each_ssa_term_src_reg(
                    &ssa_block.terminator,
                    &mut |src| {
                        if !defined.contains(&src.reg) {
                            global_preserved_regs.insert(src.reg);
                        }
                    },
                );
            }
        }
    }

    // 3. Optimize blocks using the global preserved registers set
    for block in ir.blocks.iter_mut() {
        if !block.instructions.is_empty() {
            let cfg = CFG::from_flat_instructions(&block.instructions);
            let ssa_transformer = SsaTransformer::new(cfg);
            let mut ssa_cfg = ssa_transformer.transform();

            manager.run(&mut ssa_cfg, &global_preserved_regs, false);

            let destructed_cfg = destruct_ssa(ssa_cfg);
            block.instructions = flatten_cfg(&destructed_cfg);
        }
    }

    let mut optimized_decay_handlers = HashMap::new();
    for (name, instrs) in ir.decay_handlers {
        if !instrs.is_empty() {
            let cfg = CFG::from_flat_instructions(&instrs);
            let ssa_transformer = SsaTransformer::new(cfg);
            let mut ssa_cfg = ssa_transformer.transform();
            manager.run(&mut ssa_cfg, &global_preserved_regs, false);
            let destructed_cfg = destruct_ssa(ssa_cfg);
            optimized_decay_handlers.insert(name, flatten_cfg(&destructed_cfg));
        } else {
            optimized_decay_handlers.insert(name, instrs);
        }
    }
    ir.decay_handlers = optimized_decay_handlers;

    ir
}

/// Scan all flat IR instructions across every top-level block and routine to collect
/// the names of every channel that is sent to, received from, awaited on, or used in
/// a `loop tick on` / `select` statement.  The resulting set is passed to
/// `ChannelLivenessPass` so that `OpenChan` instructions are never eliminated just
/// because their users happen to reside in a *different* temporal block.
fn collect_globally_referenced_channels(ir: &IrProgram) -> HashSet<String> {
    let mut referenced = HashSet::new();

    // Helper closure: inspect a single flat instruction and record any channel name.
    let mut record = |instr: &Instruction| match instr {
        Instruction::ChanSend { chan_id, .. }
        | Instruction::ChanRecv { chan_id, .. }
        | Instruction::AwaitChan { chan_id }
        | Instruction::LoopTickOn { chan_id } => {
            referenced.insert(chan_id.clone());
        }
        Instruction::Select { cases, .. } => {
            for c in cases {
                referenced.insert(c.chan_id.clone());
            }
        }
        _ => {}
    };

    for block in &ir.blocks {
        for instr in &block.instructions {
            record(instr);
        }
    }
    for routine in ir.routines.values() {
        for instr in &routine.instructions {
            record(instr);
        }
    }
    for instrs in ir.decay_handlers.values() {
        for instr in instrs {
            record(instr);
        }
    }

    referenced
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

pub(crate) trait IntoFlat<T> {
    fn into_flat(self) -> T;
}

impl IntoFlat<Reg> for SsaReg {
    #[inline]
    fn into_flat(self) -> Reg {
        Reg(self.reg)
    }
}

impl<T, U: IntoFlat<T>> IntoFlat<Option<T>> for Option<U> {
    #[inline]
    fn into_flat(self) -> Option<T> {
        self.map(|x| x.into_flat())
    }
}

impl<T, U: IntoFlat<T>> IntoFlat<Vec<T>> for Vec<U> {
    #[inline]
    fn into_flat(self) -> Vec<T> {
        self.into_iter().map(|x| x.into_flat()).collect()
    }
}

impl<T, U: IntoFlat<T>> IntoFlat<HashMap<String, T>> for HashMap<String, U> {
    #[inline]
    fn into_flat(self) -> HashMap<String, T> {
        self.into_iter().map(|(k, v)| (k, v.into_flat())).collect()
    }
}

impl IntoFlat<crate::IrSelectCase> for crate::ssa::SsaSelectCase {
    #[inline]
    fn into_flat(self) -> crate::IrSelectCase {
        crate::IrSelectCase {
            chan_id: self.chan_id,
            dest: Reg(self.dest.reg),
            target: self.target,
        }
    }
}

macro_rules! impl_into_flat_identity {
    ($($t:ty),* $(,)?) => {
        $(
            impl IntoFlat<$t> for $t {
                #[inline]
                fn into_flat(self) -> $t {
                    self
                }
            }
        )*
    };
}

impl_into_flat_identity!(
    i64,
    u64,
    usize,
    bool,
    String,
    causm_core::BinaryOperator,
    causm_core::UnaryOperator,
    causm_core::Manifest,
    causm_core::MergeResolution,
    causm_core::EntropyMode,
    causm_core::PolicyTarget,
    causm_core::SaturationPolicy,
    causm_core::ArenaIntrospect,
    causm_core::Capability,
    causm_core::SpeculationCommitMode,
    causm_core::SyscallTarget,
    causm_core::types::AutoDropSpec,
    causm_core::TypeName,
    causm_core::ParamMode
);

macro_rules! ssa_to_flat_match {
    ($( $variant:ident $({ $($field:ident),* $(,)? })? ),* $(,)?) => {
        fn ssa_instr_to_instr(ssa_instr: &SsaInstruction) -> Instruction {
            match ssa_instr.clone() {
                $(
                    SsaInstruction::$variant $({ $($field,)* .. })? => {
                        Instruction::$variant $({
                            $($field: $field.into_flat()),*
                        })?
                    }
                )*
                SsaInstruction::Other(s) => panic!("Cannot lower Other instruction: {}", s),
            }
        }
    };
}

ssa_to_flat_match!(
    BinaryOp {
        dest,
        op,
        left,
        right
    },
    UnaryOp { dest, op, src },
    LoadInt { dest, value },
    LoadFloat { dest, value },
    LoadBool { dest, value },
    LoadString { dest, value },
    LoadNull { dest },
    ConstInt { dest, value },
    ConstFloat { dest, value },
    ConstBool { dest, value },
    ConstString { dest, value },
    ConstNull { dest },
    Move { dest, src },
    Consume { src },
    ConsumeField { src, field },
    ConsumeFieldDynamic { target, index },
    Clone { dest, src },
    StrBytes { dest, src },
    ToStr { dest, src },
    ConditionalSelect {
        dest,
        cond,
        true_val,
        false_val
    },
    Call {
        routine,
        args,
        dest
    },
    DynamicCall {
        method,
        args,
        dest,
        budget
    },
    TypeAssert {
        dest,
        src,
        type_name
    },
    AssertState { src, state },
    TryTypeAssert {
        dest,
        src,
        type_name,
        success
    },
    TryEnumVariant {
        dest,
        src,
        enum_name,
        variant_name,
        success
    },
    StructLit {
        dest,
        fields,
        type_name
    },
    TopologyLit { dest, fields },
    ArrayLit { dest, elements },
    ArrayRepeat { dest, value, count },
    ArraySlice {
        dest,
        target,
        start,
        end,
        inclusive
    },
    FieldAccess {
        dest,
        target,
        field
    },
    FieldUpdate { target, field, src },
    IndexAccess {
        dest,
        target,
        index
    },
    IndexFieldUpdate {
        target,
        index,
        field,
        src
    },
    AssertTime { op, limit_ms },
    Capability { cap },
    For {
        dest_cond,
        item_reg,
        item_name,
        mode,
        source,
        pacing_ms,
        max_ms
    },
    EndFor,
    SplitMap {
        item_reg,
        item_name,
        mode,
        source,
        reconcile
    },
    EndSplitMap,
    Defer {
        dest,
        cap,
        deadline_ms
    },
    Await { target },
    Loop { max_ms },
    EndLoop { max_ms },
    Break,
    LoopTick,
    EndLoopTick,
    Print { src },
    Debug { src },
    Slice { ms },
    Isolate { name, manifest },
    EndIsolate,
    Lease {
        target_reg,
        source_reg,
        duration_ms
    },
    EndLease {
        source_reg,
        duration_ms
    },
    Split { parent, branches },
    Merge {
        branches,
        target,
        resolution
    },
    SetEntropyMode { mode },
    Anchor { name },
    Rewind { target, anchor },
    Commit { vars },
    Watchdog {
        target,
        timeout_ms,
        recovery_jump
    },
    Speculate {
        max_ms,
        fallback_target
    },
    EndSpeculate {
        max_ms,
        fallback_target
    },
    Collapse,
    RelativisticBlock {
        target,
        block_pc,
        block_len
    },
    SpeculationMode { mode },
    OpenChan {
        name,
        capacity,
        decay_after_ms
    },
    ChanSend { chan_id, src },
    ChanRecv { dest, chan_id },
    Entangle { regs },
    MatchEntropy {
        target,
        valid_target,
        decayed_target,
        pending_target,
        consumed_target
    },
    Select {
        max_ms,
        cases,
        timeout_target
    },
    AwaitChan { chan_id },
    Jump { target },
    JumpIf { cond, target },
    JumpIfNot { cond, target },
    While { max_ms },
    EndWhile { max_ms },
    ForStep {
        dest_cond,
        item_reg,
        item_name,
        source,
        step_ms
    },
    EndForStep,
    ArrayLen { dest, src },
    LoopTickOn { chan_id },
    TypeCast {
        dest,
        src,
        target_type
    },
    Syscall {
        dest,
        target,
        args,
        duration_ms
    },
    AutoDrop { target, spec },
    SetSaturationPolicy { target, policy },
    PeriodicEpoch {
        interval_ms,
        block_pc,
        block_len
    },
    EndPeriodicEpoch { interval_ms },
    FreezeBaseWatermark,
    ResetBaseWatermark,
    ArenaIntrospect { dest, kind },
    CapabilityCheck { dest, capability },
    TupleLit { dest, elems },
    TupleAccess { dest, tuple, index }
);

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
                let target_reg = ssa_reg_to_reg(phi.dest);
                let src_reg = ssa_reg_to_reg(incoming_reg);
                if target_reg != src_reg {
                    if let Some(pred_block) = blocks.get_mut(&pred_id) {
                        pred_block.instructions.push(Instruction::Move {
                            dest: target_reg,
                            src: src_reg,
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
