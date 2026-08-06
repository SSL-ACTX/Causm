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
    MatchEntropy {
        target: SsaReg,
        valid_block: Option<BlockId>,
        decayed_block: Option<BlockId>,
        pending_block: Option<BlockId>,
        consumed_block: Option<BlockId>,
    },
    Select {
        max_ms: u64,
        cases: Vec<SsaSelectCase>,
        timeout_block: Option<BlockId>,
    },
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaSelectCase {
    pub chan_id: String,
    pub dest: SsaReg,
    pub target: usize,
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
    ConstInt {
        dest: SsaReg,
        value: i64,
    },
    ConstFloat {
        dest: SsaReg,
        value: u64,
    },
    ConstBool {
        dest: SsaReg,
        value: bool,
    },
    ConstString {
        dest: SsaReg,
        value: String,
    },
    ConstNull {
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
    DynamicCall {
        method: String,
        args: Vec<SsaReg>,
        dest: SsaReg,
        budget: Option<u64>,
    },
    TypeAssert {
        dest: SsaReg,
        src: SsaReg,
        type_name: String,
    },
    AssertState {
        src: SsaReg,
        state: String,
    },
    TryTypeAssert {
        dest: SsaReg,
        src: SsaReg,
        type_name: String,
        success: SsaReg,
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
    Isolate {
        name: String,
        manifest: causm_core::Manifest,
    },
    EndIsolate,
    Lease {
        target_reg: SsaReg,
        source_reg: SsaReg,
        duration_ms: u64,
    },
    EndLease {
        source_reg: SsaReg,
        duration_ms: u64,
    },
    Split {
        parent: String,
        branches: Vec<String>,
    },
    Merge {
        branches: Vec<String>,
        target: String,
        resolution: causm_core::MergeResolution,
    },
    Entangle {
        regs: Vec<SsaReg>,
    },
    Anchor {
        name: String,
    },
    Rewind {
        target: String,
        anchor: String,
    },
    Commit {
        vars: Vec<String>,
    },
    Watchdog {
        target: String,
        timeout_ms: u64,
        recovery_jump: Option<usize>,
    },
    Speculate {
        max_ms: u64,
        fallback_target: usize,
    },
    EndSpeculate {
        max_ms: u64,
        fallback_target: usize,
    },
    Collapse,
    Select {
        max_ms: u64,
        cases: Vec<SsaSelectCase>,
        timeout_target: Option<usize>,
    },
    MatchEntropy {
        target: SsaReg,
        valid_target: Option<usize>,
        decayed_target: Option<usize>,
        pending_target: Option<usize>,
        consumed_target: Option<usize>,
    },
    RelativisticBlock {
        target: String,
        block_pc: usize,
        block_len: usize,
    },
    SpeculationMode {
        mode: causm_core::SpeculationCommitMode,
    },
    OpenChan {
        name: String,
        capacity: usize,
        decay_after_ms: Option<u64>,
    },
    ChanSend {
        chan_id: String,
        src: SsaReg,
    },
    ChanRecv {
        dest: SsaReg,
        chan_id: String,
    },
    AwaitChan {
        chan_id: String,
    },
    StructLit {
        dest: SsaReg,
        fields: std::collections::HashMap<String, SsaReg>,
        type_name: Option<String>,
    },
    TopologyLit {
        dest: SsaReg,
        fields: std::collections::HashMap<String, SsaReg>,
    },
    ArrayLit {
        dest: SsaReg,
        elements: Vec<SsaReg>,
    },
    FieldAccess {
        dest: SsaReg,
        target: SsaReg,
        field: String,
    },
    FieldUpdate {
        target: SsaReg,
        old_target: SsaReg,
        field: String,
        src: SsaReg,
    },
    IndexAccess {
        dest: SsaReg,
        target: SsaReg,
        index: SsaReg,
    },
    IndexFieldUpdate {
        target: SsaReg,
        old_target: SsaReg,
        index: SsaReg,
        field: String,
        src: SsaReg,
    },
    AssertTime {
        op: causm_core::BinaryOperator,
        limit_ms: u64,
    },
    Capability {
        cap: causm_core::Capability,
    },
    For {
        dest_cond: SsaReg,
        item_reg: SsaReg,
        item_name: String,
        mode: causm_core::ParamMode,
        source: SsaReg,
        pacing_ms: Option<u64>,
        max_ms: Option<u64>,
    },
    EndFor,
    SplitMap {
        item_reg: SsaReg,
        item_name: String,
        mode: causm_core::ParamMode,
        source: SsaReg,
        reconcile: Option<causm_core::MergeResolution>,
    },
    EndSplitMap,
    Defer {
        dest: SsaReg,
        cap: causm_core::Capability,
        deadline_ms: u64,
    },
    Await {
        target: SsaReg,
    },
    Loop {
        max_ms: u64,
    },
    EndLoop {
        max_ms: u64,
    },
    Break,
    LoopTick,
    EndLoopTick,
    NetworkRequest {
        domain: String,
    },
    Jump {
        target: usize,
    },
    JumpIf {
        cond: SsaReg,
        target: usize,
    },
    JumpIfNot {
        cond: SsaReg,
        target: usize,
    },
    While {
        max_ms: u64,
    },
    EndWhile {
        max_ms: u64,
    },
    ForStep {
        dest_cond: SsaReg,
        item_reg: SsaReg,
        item_name: String,
        source: SsaReg,
        step_ms: u64,
    },
    EndForStep,
    LoopTickOn {
        chan_id: String,
    },
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaCFG {
    pub entry_block: BlockId,
    pub blocks: HashMap<BlockId, SsaBasicBlock>,
    pub original_pc_to_block_id: HashMap<usize, BlockId>,
}

impl std::fmt::Display for SsaInstruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsaInstruction::BinaryOp {
                dest,
                op,
                left,
                right,
            } => {
                write!(f, "{} = {} {:?} {}", dest, left, op, right)
            }
            SsaInstruction::UnaryOp { dest, op, src } => {
                write!(f, "{} = {:?} {}", dest, op, src)
            }
            SsaInstruction::LoadInt { dest, value }
            | SsaInstruction::ConstInt { dest, value } => {
                write!(f, "{} = {}", dest, value)
            }
            SsaInstruction::LoadFloat { dest, value }
            | SsaInstruction::ConstFloat { dest, value } => {
                write!(f, "{} = {}", dest, value)
            }
            SsaInstruction::LoadBool { dest, value }
            | SsaInstruction::ConstBool { dest, value } => {
                write!(f, "{} = {}", dest, value)
            }
            SsaInstruction::LoadString { dest, value }
            | SsaInstruction::ConstString { dest, value } => {
                write!(f, "{} = {:?}", dest, value)
            }
            SsaInstruction::LoadNull { dest }
            | SsaInstruction::ConstNull { dest } => {
                write!(f, "{} = null", dest)
            }
            SsaInstruction::Move { dest, src } => {
                write!(f, "{} = {}", dest, src)
            }
            SsaInstruction::Consume { src } => {
                write!(f, "Consume {}", src)
            }
            SsaInstruction::ConsumeField { src, field } => {
                write!(f, "ConsumeField {}.{}", src, field)
            }
            SsaInstruction::ConsumeFieldDynamic { target, index } => {
                write!(f, "ConsumeFieldDynamic {}[{}]", target, index)
            }
            SsaInstruction::Clone { dest, src } => {
                write!(f, "{} = Clone {}", dest, src)
            }
            SsaInstruction::Call {
                routine,
                args,
                dest,
            } => {
                let args_str: Vec<String> =
                    args.iter().map(|a| a.to_string()).collect();
                write!(f, "{} = Call {}({})", dest, routine, args_str.join(", "))
            }
            SsaInstruction::DynamicCall {
                method,
                args,
                dest,
                budget,
            } => {
                let args_str: Vec<String> =
                    args.iter().map(|a| a.to_string()).collect();
                let budget_str = budget
                    .map(|b| format!(" [budget: {}ms]", b))
                    .unwrap_or_default();
                write!(
                    f,
                    "{} = DynamicCall {}({}){}",
                    dest,
                    method,
                    args_str.join(", "),
                    budget_str
                )
            }
            SsaInstruction::TypeAssert {
                dest,
                src,
                type_name,
            } => {
                write!(f, "{} = {} as {}", dest, src, type_name)
            }
            SsaInstruction::AssertState { src, state } => {
                write!(f, "AssertState {} is {}", src, state)
            }
            SsaInstruction::TryTypeAssert {
                dest,
                src,
                type_name,
                success,
            } => {
                write!(
                    f,
                    "{}, {} = TryTypeAssert {} as {}",
                    dest, success, src, type_name
                )
            }
            SsaInstruction::Print { src } => {
                write!(f, "Print {}", src)
            }
            SsaInstruction::Debug { src } => {
                write!(f, "Debug {}", src)
            }
            SsaInstruction::Slice { ms } => {
                write!(f, "Slice {}ms", ms)
            }
            SsaInstruction::Isolate { name, manifest } => {
                write!(f, "Isolate {} {:?}", name, manifest)
            }
            SsaInstruction::EndIsolate => {
                write!(f, "EndIsolate")
            }
            SsaInstruction::Lease {
                target_reg,
                source_reg,
                duration_ms,
            } => {
                write!(
                    f,
                    "{} = Lease {} for {}ms",
                    target_reg, source_reg, duration_ms
                )
            }
            SsaInstruction::EndLease {
                source_reg,
                duration_ms,
            } => {
                write!(f, "EndLease {} for {}ms", source_reg, duration_ms)
            }
            SsaInstruction::Split { parent, branches } => {
                write!(f, "Split {} into {:?}", parent, branches)
            }
            SsaInstruction::Merge {
                branches,
                target,
                resolution,
            } => {
                write!(f, "Merge {:?} into {} {:?}", branches, target, resolution)
            }
            SsaInstruction::Entangle { regs } => {
                let regs_str: Vec<String> =
                    regs.iter().map(|r| r.to_string()).collect();
                write!(f, "Entangle [{}]", regs_str.join(", "))
            }
            SsaInstruction::Anchor { name } => {
                write!(f, "Anchor {}", name)
            }
            SsaInstruction::Rewind { target, anchor } => {
                write!(f, "Rewind {} to {}", target, anchor)
            }
            SsaInstruction::Commit { vars } => {
                write!(f, "Commit {:?}", vars)
            }
            SsaInstruction::Watchdog {
                target,
                timeout_ms,
                recovery_jump: _,
            } => {
                write!(f, "Watchdog {} {}ms", target, timeout_ms)
            }
            SsaInstruction::Speculate {
                max_ms,
                fallback_target: _,
            } => {
                write!(f, "Speculate {}ms", max_ms)
            }
            SsaInstruction::EndSpeculate {
                max_ms,
                fallback_target: _,
            } => {
                write!(f, "EndSpeculate {}ms", max_ms)
            }
            SsaInstruction::Collapse => {
                write!(f, "Collapse")
            }
            SsaInstruction::Select {
                max_ms,
                cases,
                timeout_target: _,
            } => {
                let cases_str: Vec<String> = cases
                    .iter()
                    .map(|c| {
                        format!("{} -> {} to Block {}", c.chan_id, c.dest, c.target)
                    })
                    .collect();
                write!(f, "Select (max {}ms) [ {} ]", max_ms, cases_str.join(", "))
            }
            SsaInstruction::MatchEntropy { target, .. } => {
                write!(f, "MatchEntropy {}", target)
            }
            SsaInstruction::RelativisticBlock {
                target,
                block_pc,
                block_len,
            } => {
                write!(
                    f,
                    "RelativisticBlock {} pc: {} len: {}",
                    target, block_pc, block_len
                )
            }
            SsaInstruction::SpeculationMode { mode } => {
                write!(f, "SpeculationMode {:?}", mode)
            }
            SsaInstruction::OpenChan {
                name,
                capacity,
                decay_after_ms: _,
            } => {
                write!(f, "OpenChan {}({})", name, capacity)
            }
            SsaInstruction::ChanSend { chan_id, src } => {
                write!(f, "ChanSend {}, {}", chan_id, src)
            }
            SsaInstruction::ChanRecv { dest, chan_id } => {
                write!(f, "{} = ChanRecv {}", dest, chan_id)
            }
            SsaInstruction::AwaitChan { chan_id } => {
                write!(f, "AwaitChan {}", chan_id)
            }
            SsaInstruction::StructLit {
                dest,
                fields,
                type_name: _,
            } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} = StructLit {{ {} }}", dest, fields_str.join(", "))
            }
            SsaInstruction::TopologyLit { dest, fields } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} = TopologyLit {{ {} }}", dest, fields_str.join(", "))
            }
            SsaInstruction::ArrayLit { dest, elements } => {
                let elems_str: Vec<String> =
                    elements.iter().map(|e| e.to_string()).collect();
                write!(f, "{} = [ {} ]", dest, elems_str.join(", "))
            }
            SsaInstruction::FieldAccess {
                dest,
                target,
                field,
            } => {
                write!(f, "{} = {}.{}", dest, target, field)
            }
            SsaInstruction::FieldUpdate {
                target,
                old_target,
                field,
                src,
            } => {
                write!(f, "{} = {}.{} <- {}", target, old_target, field, src)
            }
            SsaInstruction::IndexAccess {
                dest,
                target,
                index,
            } => {
                write!(f, "{} = {}[{}]", dest, target, index)
            }
            SsaInstruction::IndexFieldUpdate {
                target,
                old_target,
                index,
                field,
                src,
            } => {
                write!(
                    f,
                    "{} = {}[{}].{} <- {}",
                    target, old_target, index, field, src
                )
            }
            SsaInstruction::AssertTime { op, limit_ms } => {
                write!(f, "AssertTime {:?} {}ms", op, limit_ms)
            }
            SsaInstruction::Capability { cap } => {
                write!(f, "Capability {:?}", cap)
            }
            SsaInstruction::For {
                dest_cond,
                item_reg,
                item_name: _,
                mode,
                source,
                pacing_ms: _,
                max_ms: _,
            } => {
                write!(
                    f,
                    "For {}, {} in {} (mode: {:?})",
                    dest_cond, item_reg, source, mode
                )
            }
            SsaInstruction::EndFor => {
                write!(f, "EndFor")
            }
            SsaInstruction::SplitMap {
                item_reg,
                item_name: _,
                mode,
                source,
                reconcile: _,
            } => {
                write!(f, "SplitMap {} in {} (mode: {:?})", item_reg, source, mode)
            }
            SsaInstruction::EndSplitMap => {
                write!(f, "EndSplitMap")
            }
            SsaInstruction::Defer {
                dest,
                cap,
                deadline_ms,
            } => {
                write!(f, "{} = Defer {:?} deadline: {}ms", dest, cap, deadline_ms)
            }
            SsaInstruction::Await { target } => {
                write!(f, "Await {}", target)
            }
            SsaInstruction::Loop { max_ms } => {
                write!(f, "Loop {}ms", max_ms)
            }
            SsaInstruction::EndLoop { max_ms } => {
                write!(f, "EndLoop {}ms", max_ms)
            }
            SsaInstruction::Break => {
                write!(f, "Break")
            }
            SsaInstruction::LoopTick => {
                write!(f, "LoopTick")
            }
            SsaInstruction::EndLoopTick => {
                write!(f, "EndLoopTick")
            }
            SsaInstruction::While { max_ms } => {
                write!(f, "While {}ms", max_ms)
            }
            SsaInstruction::EndWhile { max_ms } => {
                write!(f, "EndWhile {}ms", max_ms)
            }
            SsaInstruction::ForStep {
                dest_cond,
                item_reg,
                item_name: _,
                source,
                step_ms,
            } => {
                write!(
                    f,
                    "ForStep {}, {} in {} step {}ms",
                    dest_cond, item_reg, source, step_ms
                )
            }
            SsaInstruction::EndForStep => {
                write!(f, "EndForStep")
            }
            SsaInstruction::LoopTickOn { chan_id } => {
                write!(f, "LoopTickOn {}", chan_id)
            }
            SsaInstruction::NetworkRequest { domain } => {
                write!(f, "NetworkRequest {}", domain)
            }
            SsaInstruction::Jump { target } => {
                write!(f, "Jump Block {}", target)
            }
            SsaInstruction::JumpIf { cond, target } => {
                write!(f, "JumpIf {} to Block {}", cond, target)
            }
            SsaInstruction::JumpIfNot { cond, target } => {
                write!(f, "JumpIfNot {} to Block {}", cond, target)
            }
            SsaInstruction::Other(s) => {
                write!(f, "{}", s)
            }
        }
    }
}

impl std::fmt::Display for SsaTerminator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SsaTerminator::Jump { target } => {
                write!(f, "Jump Block {}", target)
            }
            SsaTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                write!(
                    f,
                    "Branch {} -> Block {} else Block {}",
                    cond, then_block, else_block
                )
            }
            SsaTerminator::Return { src } => {
                let src_str = src
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "void".to_string());
                write!(f, "Return {}", src_str)
            }
            SsaTerminator::MatchEntropy {
                target,
                valid_block,
                decayed_block,
                pending_block,
                consumed_block,
            } => {
                let mut cases = Vec::new();
                if let Some(b) = valid_block {
                    cases.push(format!("Valid: Block {}", b));
                }
                if let Some(b) = decayed_block {
                    cases.push(format!("Decayed: Block {}", b));
                }
                if let Some(b) = pending_block {
                    cases.push(format!("Pending: Block {}", b));
                }
                if let Some(b) = consumed_block {
                    cases.push(format!("Consumed: Block {}", b));
                }
                write!(f, "MatchEntropy {} -> [ {} ]", target, cases.join(", "))
            }
            SsaTerminator::Select {
                max_ms,
                cases,
                timeout_block,
            } => {
                let cases_str: Vec<String> = cases
                    .iter()
                    .map(|c| {
                        format!("{} -> {} to Block {}", c.chan_id, c.dest, c.target)
                    })
                    .collect();
                let timeout_str = timeout_block
                    .map(|b| format!(", Timeout: Block {}", b))
                    .unwrap_or_default();
                write!(
                    f,
                    "Select (max {}ms) [ {} ]{}",
                    max_ms,
                    cases_str.join(", "),
                    timeout_str
                )
            }
            SsaTerminator::Unreachable => {
                write!(f, "Unreachable")
            }
        }
    }
}

impl std::fmt::Display for SsaCFG {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut block_ids: Vec<&BlockId> = self.blocks.keys().collect();
        block_ids.sort();

        let mut copies: HashMap<SsaReg, SsaReg> = HashMap::new();
        let mut constants: HashMap<SsaReg, String> = HashMap::new();

        for block in self.blocks.values() {
            for inst in &block.instructions {
                match inst {
                    SsaInstruction::ConstInt { dest, value }
                    | SsaInstruction::LoadInt { dest, value } => {
                        constants.insert(*dest, value.to_string());
                    }
                    SsaInstruction::ConstFloat { dest, value }
                    | SsaInstruction::LoadFloat { dest, value } => {
                        constants.insert(*dest, value.to_string());
                    }
                    SsaInstruction::ConstBool { dest, value }
                    | SsaInstruction::LoadBool { dest, value } => {
                        constants.insert(*dest, value.to_string());
                    }
                    SsaInstruction::ConstString { dest, value }
                    | SsaInstruction::LoadString { dest, value } => {
                        constants.insert(*dest, format!("{:?}", value));
                    }
                    SsaInstruction::ConstNull { dest }
                    | SsaInstruction::LoadNull { dest } => {
                        constants.insert(*dest, "null".to_string());
                    }
                    SsaInstruction::Move { dest, src } => {
                        copies.insert(*dest, *src);
                    }
                    _ => {}
                }
            }
        }

        let resolve_reg = |reg: SsaReg| -> String {
            let mut curr = reg;
            while let Some(&next) = copies.get(&curr) {
                curr = next;
            }
            if let Some(val) = constants.get(&curr) {
                val.clone()
            } else {
                format!("{}", curr)
            }
        };

        let format_inst = |inst: &SsaInstruction| -> String {
            match inst {
                SsaInstruction::BinaryOp {
                    dest,
                    op,
                    left,
                    right,
                } => {
                    format!(
                        "{} = {} {:?} {}",
                        dest,
                        resolve_reg(*left),
                        op,
                        resolve_reg(*right)
                    )
                }
                SsaInstruction::UnaryOp { dest, op, src } => {
                    format!("{} = {:?} {}", dest, op, resolve_reg(*src))
                }
                SsaInstruction::LoadInt { dest, value }
                | SsaInstruction::ConstInt { dest, value } => {
                    format!("{} = {}", dest, value)
                }
                SsaInstruction::LoadFloat { dest, value }
                | SsaInstruction::ConstFloat { dest, value } => {
                    format!("{} = {}", dest, value)
                }
                SsaInstruction::LoadBool { dest, value }
                | SsaInstruction::ConstBool { dest, value } => {
                    format!("{} = {}", dest, value)
                }
                SsaInstruction::LoadString { dest, value }
                | SsaInstruction::ConstString { dest, value } => {
                    format!("{} = {:?}", dest, value)
                }
                SsaInstruction::LoadNull { dest }
                | SsaInstruction::ConstNull { dest } => {
                    format!("{} = null", dest)
                }
                SsaInstruction::Move { dest, src } => {
                    format!("{} = {}", dest, resolve_reg(*src))
                }
                SsaInstruction::Consume { src } => {
                    format!("Consume {}", resolve_reg(*src))
                }
                SsaInstruction::ConsumeField { src, field } => {
                    format!("ConsumeField {}.{}", resolve_reg(*src), field)
                }
                SsaInstruction::ConsumeFieldDynamic { target, index } => {
                    format!(
                        "ConsumeFieldDynamic {}[{}]",
                        resolve_reg(*target),
                        resolve_reg(*index)
                    )
                }
                SsaInstruction::Clone { dest, src } => {
                    format!("{} = Clone {}", dest, resolve_reg(*src))
                }
                SsaInstruction::Call {
                    routine,
                    args,
                    dest,
                } => {
                    let formatted_args: Vec<String> =
                        args.iter().map(|&a| resolve_reg(a)).collect();
                    format!(
                        "{} = Call {}({})",
                        dest,
                        routine,
                        formatted_args.join(", ")
                    )
                }
                SsaInstruction::DynamicCall {
                    method,
                    args,
                    dest,
                    budget,
                } => {
                    let formatted_args: Vec<String> =
                        args.iter().map(|&a| resolve_reg(a)).collect();
                    let budget_str = budget
                        .map(|b| format!(" [budget: {}ms]", b))
                        .unwrap_or_default();
                    format!(
                        "{} = DynamicCall {}({}){}",
                        dest,
                        method,
                        formatted_args.join(", "),
                        budget_str
                    )
                }
                SsaInstruction::TypeAssert {
                    dest,
                    src,
                    type_name,
                } => {
                    format!("{} = {} as {}", dest, resolve_reg(*src), type_name)
                }
                SsaInstruction::AssertState { src, state } => {
                    format!("AssertState {} is {}", resolve_reg(*src), state)
                }
                SsaInstruction::TryTypeAssert {
                    dest,
                    src,
                    type_name,
                    success,
                } => {
                    format!(
                        "{}, {} = TryTypeAssert {} as {}",
                        dest,
                        success,
                        resolve_reg(*src),
                        type_name
                    )
                }
                SsaInstruction::Print { src } => {
                    format!("Print {}", resolve_reg(*src))
                }
                SsaInstruction::Debug { src } => {
                    format!("Debug {}", resolve_reg(*src))
                }
                SsaInstruction::Slice { ms } => {
                    format!("Slice {}ms", ms)
                }
                SsaInstruction::Isolate { name, manifest } => {
                    format!("Isolate {} {:?}", name, manifest)
                }
                SsaInstruction::EndIsolate => "EndIsolate".to_string(),
                SsaInstruction::Lease {
                    target_reg,
                    source_reg,
                    duration_ms,
                } => {
                    format!(
                        "{} = Lease {} for {}ms",
                        target_reg,
                        resolve_reg(*source_reg),
                        duration_ms
                    )
                }
                SsaInstruction::EndLease {
                    source_reg,
                    duration_ms,
                } => {
                    format!(
                        "EndLease {} for {}ms",
                        resolve_reg(*source_reg),
                        duration_ms
                    )
                }
                SsaInstruction::Split { parent, branches } => {
                    format!("Split {} into {:?}", parent, branches)
                }
                SsaInstruction::Merge {
                    branches,
                    target,
                    resolution,
                } => {
                    format!("Merge {:?} into {} {:?}", branches, target, resolution)
                }
                SsaInstruction::Entangle { regs } => {
                    let formatted_regs: Vec<String> =
                        regs.iter().map(|&r| resolve_reg(r)).collect();
                    format!("Entangle [{}]", formatted_regs.join(", "))
                }
                SsaInstruction::Anchor { name } => {
                    format!("Anchor {}", name)
                }
                SsaInstruction::Rewind { target, anchor } => {
                    format!("Rewind {} to {}", target, anchor)
                }
                SsaInstruction::Commit { vars } => {
                    format!("Commit {:?}", vars)
                }
                SsaInstruction::Watchdog {
                    target,
                    timeout_ms,
                    recovery_jump: _,
                } => {
                    format!("Watchdog {} {}ms", target, timeout_ms)
                }
                SsaInstruction::Speculate {
                    max_ms,
                    fallback_target: _,
                } => {
                    format!("Speculate {}ms", max_ms)
                }
                SsaInstruction::EndSpeculate {
                    max_ms,
                    fallback_target: _,
                } => {
                    format!("EndSpeculate {}ms", max_ms)
                }
                SsaInstruction::Collapse => "Collapse".to_string(),
                SsaInstruction::Select {
                    max_ms,
                    cases,
                    timeout_target: _,
                } => {
                    let formatted_cases: Vec<String> = cases
                        .iter()
                        .map(|c| {
                            format!(
                                "{} -> {} to Block {}",
                                c.chan_id, c.dest, c.target
                            )
                        })
                        .collect();
                    format!(
                        "Select (max {}ms) [ {} ]",
                        max_ms,
                        formatted_cases.join(", ")
                    )
                }
                SsaInstruction::MatchEntropy { target, .. } => {
                    format!("MatchEntropy {}", resolve_reg(*target))
                }
                SsaInstruction::RelativisticBlock {
                    target,
                    block_pc,
                    block_len,
                } => {
                    format!(
                        "RelativisticBlock {} pc: {} len: {}",
                        target, block_pc, block_len
                    )
                }
                SsaInstruction::SpeculationMode { mode } => {
                    format!("SpeculationMode {:?}", mode)
                }
                SsaInstruction::OpenChan {
                    name,
                    capacity,
                    decay_after_ms: _,
                } => {
                    format!("OpenChan {}({})", name, capacity)
                }
                SsaInstruction::ChanSend { chan_id, src } => {
                    format!("ChanSend {}, {}", chan_id, resolve_reg(*src))
                }
                SsaInstruction::ChanRecv { dest, chan_id } => {
                    format!("{} = ChanRecv {}", dest, chan_id)
                }
                SsaInstruction::AwaitChan { chan_id } => {
                    format!("AwaitChan {}", chan_id)
                }
                SsaInstruction::StructLit {
                    dest,
                    fields,
                    type_name: _,
                } => {
                    let formatted_fields: Vec<String> = fields
                        .iter()
                        .map(|(k, &v)| format!("{}: {}", k, resolve_reg(v)))
                        .collect();
                    format!(
                        "{} = StructLit {{ {} }}",
                        dest,
                        formatted_fields.join(", ")
                    )
                }
                SsaInstruction::TopologyLit { dest, fields } => {
                    let formatted_fields: Vec<String> = fields
                        .iter()
                        .map(|(k, &v)| format!("{}: {}", k, resolve_reg(v)))
                        .collect();
                    format!(
                        "{} = TopologyLit {{ {} }}",
                        dest,
                        formatted_fields.join(", ")
                    )
                }
                SsaInstruction::ArrayLit { dest, elements } => {
                    let formatted_elems: Vec<String> =
                        elements.iter().map(|&e| resolve_reg(e)).collect();
                    format!("{} = [ {} ]", dest, formatted_elems.join(", "))
                }
                SsaInstruction::FieldAccess {
                    dest,
                    target,
                    field,
                } => {
                    format!("{} = {}.{}", dest, resolve_reg(*target), field)
                }
                SsaInstruction::FieldUpdate {
                    target,
                    old_target,
                    field,
                    src,
                } => {
                    format!(
                        "{} = {}.{} <- {}",
                        target,
                        resolve_reg(*old_target),
                        field,
                        resolve_reg(*src)
                    )
                }
                SsaInstruction::IndexAccess {
                    dest,
                    target,
                    index,
                } => {
                    format!(
                        "{} = {}[{}]",
                        dest,
                        resolve_reg(*target),
                        resolve_reg(*index)
                    )
                }
                SsaInstruction::IndexFieldUpdate {
                    target,
                    old_target,
                    index,
                    field,
                    src,
                } => {
                    format!(
                        "{} = {}[{}].{} <- {}",
                        target,
                        resolve_reg(*old_target),
                        resolve_reg(*index),
                        field,
                        resolve_reg(*src)
                    )
                }
                SsaInstruction::AssertTime { op, limit_ms } => {
                    format!("AssertTime {:?} {}ms", op, limit_ms)
                }
                SsaInstruction::Capability { cap } => {
                    format!("Capability {:?}", cap)
                }
                SsaInstruction::For {
                    dest_cond,
                    item_reg,
                    item_name: _,
                    mode,
                    source,
                    pacing_ms: _,
                    max_ms: _,
                } => {
                    format!(
                        "For {}, {} in {} (mode: {:?})",
                        dest_cond,
                        item_reg,
                        resolve_reg(*source),
                        mode
                    )
                }
                SsaInstruction::EndFor => "EndFor".to_string(),
                SsaInstruction::SplitMap {
                    item_reg,
                    item_name: _,
                    mode,
                    source,
                    reconcile: _,
                } => {
                    format!(
                        "SplitMap {} in {} (mode: {:?})",
                        item_reg,
                        resolve_reg(*source),
                        mode
                    )
                }
                SsaInstruction::EndSplitMap => "EndSplitMap".to_string(),
                SsaInstruction::Defer {
                    dest,
                    cap,
                    deadline_ms,
                } => {
                    format!("{} = Defer {:?} deadline: {}ms", dest, cap, deadline_ms)
                }
                SsaInstruction::Await { target } => {
                    format!("Await {}", resolve_reg(*target))
                }
                SsaInstruction::Loop { max_ms } => {
                    format!("Loop {}ms", max_ms)
                }
                SsaInstruction::EndLoop { max_ms } => {
                    format!("EndLoop {}ms", max_ms)
                }
                SsaInstruction::Break => "Break".to_string(),
                SsaInstruction::LoopTick => "LoopTick".to_string(),
                SsaInstruction::EndLoopTick => "EndLoopTick".to_string(),
                SsaInstruction::While { max_ms } => {
                    format!("While {}ms", max_ms)
                }
                SsaInstruction::EndWhile { max_ms } => {
                    format!("EndWhile {}ms", max_ms)
                }
                SsaInstruction::ForStep {
                    dest_cond,
                    item_reg,
                    item_name: _,
                    source,
                    step_ms,
                } => {
                    format!(
                        "ForStep {}, {} in {} step {}ms",
                        dest_cond,
                        item_reg,
                        resolve_reg(*source),
                        step_ms
                    )
                }
                SsaInstruction::EndForStep => "EndForStep".to_string(),
                SsaInstruction::LoopTickOn { chan_id } => {
                    format!("LoopTickOn {}", chan_id)
                }
                SsaInstruction::NetworkRequest { domain } => {
                    format!("NetworkRequest {}", domain)
                }
                SsaInstruction::Jump { target } => {
                    format!("Jump Block {}", target)
                }
                SsaInstruction::JumpIf { cond, target } => {
                    format!("JumpIf {} to Block {}", resolve_reg(*cond), target)
                }
                SsaInstruction::JumpIfNot { cond, target } => {
                    format!("JumpIfNot {} to Block {}", resolve_reg(*cond), target)
                }
                SsaInstruction::Other(s) => s.clone(),
            }
        };

        let format_term = |term: &SsaTerminator| -> String {
            match term {
                SsaTerminator::Jump { target } => {
                    format!("Jump Block {}", target)
                }
                SsaTerminator::Branch {
                    cond,
                    then_block,
                    else_block,
                } => {
                    format!(
                        "Branch {} -> Block {} else Block {}",
                        resolve_reg(*cond),
                        then_block,
                        else_block
                    )
                }
                SsaTerminator::Return { src } => {
                    let src_str = src
                        .map(|s| resolve_reg(s))
                        .unwrap_or_else(|| "void".to_string());
                    format!("Return {}", src_str)
                }
                SsaTerminator::MatchEntropy {
                    target,
                    valid_block,
                    decayed_block,
                    pending_block,
                    consumed_block,
                } => {
                    let mut cases = Vec::new();
                    if let Some(b) = valid_block {
                        cases.push(format!("Valid: Block {}", b));
                    }
                    if let Some(b) = decayed_block {
                        cases.push(format!("Decayed: Block {}", b));
                    }
                    if let Some(b) = pending_block {
                        cases.push(format!("Pending: Block {}", b));
                    }
                    if let Some(b) = consumed_block {
                        cases.push(format!("Consumed: Block {}", b));
                    }
                    format!(
                        "MatchEntropy {} -> [ {} ]",
                        resolve_reg(*target),
                        cases.join(", ")
                    )
                }
                SsaTerminator::Select {
                    max_ms,
                    cases,
                    timeout_block,
                } => {
                    let formatted_cases: Vec<String> = cases
                        .iter()
                        .map(|c| {
                            format!(
                                "{} -> {} to Block {}",
                                c.chan_id, c.dest, c.target
                            )
                        })
                        .collect();
                    let timeout_str = timeout_block
                        .map(|b| format!(", Timeout: Block {}", b))
                        .unwrap_or_default();
                    format!(
                        "Select (max {}ms) [ {} ]{}",
                        max_ms,
                        formatted_cases.join(", "),
                        timeout_str
                    )
                }
                SsaTerminator::Unreachable => "Unreachable".to_string(),
            }
        };

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
                writeln!(f, "    {}", format_inst(instr))?;
            }
            writeln!(f, "    Terminator: {}", format_term(&block.terminator))?;
        }
        Ok(())
    }
}

impl SsaCFG {
    pub fn to_dot(&self) -> String {
        let mut dot = String::new();
        dot.push_str("digraph SsaCFG {\n");
        dot.push_str("  node [shape=box, fontname=\"Courier\"];\n");

        let mut block_ids: Vec<&BlockId> = self.blocks.keys().collect();
        block_ids.sort();

        for &id in &block_ids {
            let block = &self.blocks[id];
            let mut label = format!("Block {}\\n", id);
            for phi in &block.phi_nodes {
                let incoming_str: Vec<String> = phi
                    .incoming
                    .iter()
                    .map(|(b, reg)| format!("{} from Block {}", reg, b))
                    .collect();
                label.push_str(&format!(
                    "  {} = phi [ {} ]\\n",
                    phi.dest,
                    incoming_str.join(", ")
                ));
            }
            for instr in &block.instructions {
                let clean_instr = format!("{}", instr)
                    .replace('"', "\\\"")
                    .replace('\n', "\\n");
                label.push_str(&format!("  {}\\n", clean_instr));
            }
            let clean_term = format!("{}", block.terminator)
                .replace('"', "\\\"")
                .replace('\n', "\\n");
            label.push_str(&format!("  Terminator: {}\\n", clean_term));

            dot.push_str(&format!("  block_{} [label=\"{}\"];\n", id, label));

            // Successors
            match &block.terminator {
                SsaTerminator::Jump { target } => {
                    dot.push_str(&format!("  block_{} -> block_{};\n", id, target));
                }
                SsaTerminator::Branch {
                    then_block,
                    else_block,
                    ..
                } => {
                    dot.push_str(&format!(
                        "  block_{} -> block_{} [label=\"then\"];\n",
                        id, then_block
                    ));
                    dot.push_str(&format!(
                        "  block_{} -> block_{} [label=\"else\"];\n",
                        id, else_block
                    ));
                }
                SsaTerminator::MatchEntropy {
                    valid_block,
                    decayed_block,
                    pending_block,
                    consumed_block,
                    ..
                } => {
                    if let Some(t) = valid_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"valid\"];\n",
                            id, t
                        ));
                    }
                    if let Some(t) = decayed_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"decayed\"];\n",
                            id, t
                        ));
                    }
                    if let Some(t) = pending_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"pending\"];\n",
                            id, t
                        ));
                    }
                    if let Some(t) = consumed_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"consumed\"];\n",
                            id, t
                        ));
                    }
                }
                SsaTerminator::Select {
                    cases,
                    timeout_block,
                    ..
                } => {
                    for case in cases {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"case {}\"];\n",
                            id, case.target, case.chan_id
                        ));
                    }
                    if let Some(t) = timeout_block {
                        dot.push_str(&format!(
                            "  block_{} -> block_{} [label=\"timeout\"];\n",
                            id, t
                        ));
                    }
                }
                SsaTerminator::Return { .. } | SsaTerminator::Unreachable => {}
            }
        }
        dot.push_str("}\n");
        dot
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
            Instruction::LoopTickOn { chan_id } => SsaInstruction::LoopTickOn {
                chan_id: chan_id.clone(),
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

// Helpers to extract dest register from flat Instruction
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
        Instruction::Call { dest, .. } => f(*dest),
        Instruction::DynamicCall { dest, .. } => f(*dest),
        Instruction::TypeAssert { dest, .. } => f(*dest),
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
        // Program structure for the SSA renaming and phi insertion unit test:
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
