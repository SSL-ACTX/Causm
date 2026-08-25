use crate::{cfg::BlockId, Reg};

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
    StrBytes {
        dest: SsaReg,
        src: SsaReg,
    },
    ToStr {
        dest: SsaReg,
        src: SsaReg,
    },
    ConditionalSelect {
        dest: SsaReg,
        cond: SsaReg,
        true_val: SsaReg,
        false_val: SsaReg,
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
    TypeCast {
        dest: SsaReg,
        src: SsaReg,
        target_type: causm_core::TypeName,
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
    TryEnumVariant {
        dest: SsaReg,
        src: SsaReg,
        enum_name: Option<String>,
        variant_name: String,
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
    SetEntropyMode {
        mode: causm_core::EntropyMode,
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
    ArrayRepeat {
        dest: SsaReg,
        value: SsaReg,
        count: SsaReg,
    },
    ArraySlice {
        dest: SsaReg,
        target: SsaReg,
        start: Option<SsaReg>,
        end: Option<SsaReg>,
        inclusive: bool,
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
        step_ms: Option<u64>,
    },
    EndForStep,
    ArrayLen {
        dest: SsaReg,
        src: SsaReg,
    },
    LoopTickOn {
        chan_id: String,
    },
    Syscall {
        dest: SsaReg,
        target: causm_core::SyscallTarget,
        args: Vec<SsaReg>,
        duration_ms: Option<u64>,
    },
    AutoDrop {
        target: SsaReg,
        spec: causm_core::types::AutoDropSpec,
    },
    SetSaturationPolicy {
        target: causm_core::PolicyTarget,
        policy: causm_core::SaturationPolicy,
    },
    PeriodicEpoch {
        interval_ms: u64,
        block_pc: usize,
        block_len: usize,
    },
    EndPeriodicEpoch {
        interval_ms: u64,
    },
    FreezeBaseWatermark,
    ResetBaseWatermark,
    ArenaIntrospect {
        dest: SsaReg,
        kind: causm_core::ArenaIntrospect,
    },
    CapabilityCheck {
        dest: SsaReg,
        capability: causm_core::Capability,
    },
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaCFG {
    pub entry_block: BlockId,
    pub blocks: std::collections::HashMap<BlockId, SsaBasicBlock>,
    pub original_pc_to_block_id: std::collections::HashMap<usize, BlockId>,
}
