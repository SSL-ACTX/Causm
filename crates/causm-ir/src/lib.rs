use causm_core::TimeCoordinate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reg(pub u32);

impl std::fmt::Display for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrSelectCase {
    pub chan_id: String,
    pub dest: Reg,
    pub target: usize,
}

#[macro_export]
macro_rules! instructions {
    ($macro:ident) => {
        $macro! {
            // Arithmetic & Logic
            BinaryOp {
                dest: $crate::Reg,
                op: causm_core::BinaryOperator,
                left: $crate::Reg,
                right: $crate::Reg
            },
            UnaryOp {
                dest: $crate::Reg,
                op: causm_core::UnaryOperator,
                src: $crate::Reg
            },

            // Data Movement
            LoadInt {
                dest: $crate::Reg,
                value: i64
            },
            LoadFloat {
                dest: $crate::Reg,
                value: u64
            },
            LoadBool {
                dest: $crate::Reg,
                value: bool
            },
            LoadString {
                dest: $crate::Reg,
                value: String
            },
            LoadNull {
                dest: $crate::Reg
            },
            ConstInt {
                dest: $crate::Reg,
                value: i64
            },
            ConstFloat {
                dest: $crate::Reg,
                value: u64
            },
            ConstBool {
                dest: $crate::Reg,
                value: bool
            },
            ConstString {
                dest: $crate::Reg,
                value: String
            },
            ConstNull {
                dest: $crate::Reg
            },
            Move {
                dest: $crate::Reg,
                src: $crate::Reg
            },
            TupleLit {
                dest: $crate::Reg,
                elems: Vec<$crate::Reg>
            },
            TupleAccess {
                dest: $crate::Reg,
                tuple: $crate::Reg,
                index: usize
            },

            // Entropic Operations
            Consume {
                src: $crate::Reg
            },
            ConsumeField {
                src: $crate::Reg,
                field: String
            },
            ConsumeFieldDynamic {
                target: $crate::Reg,
                index: $crate::Reg
            },
            Clone {
                dest: $crate::Reg,
                src: $crate::Reg
            },
            StrBytes {
                dest: $crate::Reg,
                src: $crate::Reg
            },
            ToStr {
                dest: $crate::Reg,
                src: $crate::Reg
            },
            ConditionalSelect {
                dest: $crate::Reg,
                cond: $crate::Reg,
                true_val: $crate::Reg,
                false_val: $crate::Reg
            },

            // Control Flow
            Jump {
                target: usize
            },
            JumpIf {
                cond: $crate::Reg,
                target: usize
            },
            JumpIfNot {
                cond: $crate::Reg,
                target: usize
            },
            Call {
                routine: String,
                args: Vec<$crate::Reg>,
                dest: $crate::Reg
            },
            DynamicCall {
                method: String,
                args: Vec<$crate::Reg>,
                dest: $crate::Reg,
                budget: Option<u64>
            },
            Return {
                src: Option<$crate::Reg>
            },

            // Causm Temporal & Isolated Concurrency
            Isolate {
                name: String,
                manifest: causm_core::Manifest
            },
            EndIsolate,
            Lease {
                target_reg: $crate::Reg,
                source_reg: $crate::Reg,
                duration_ms: u64
            },
            EndLease {
                source_reg: $crate::Reg,
                duration_ms: u64
            },
            Split {
                parent: String,
                branches: Vec<String>
            },
            Merge {
                branches: Vec<String>,
                target: String,
                resolution: causm_core::MergeResolution
            },
            Entangle {
                regs: Vec<$crate::Reg>
            },
            SetEntropyMode {
                mode: causm_core::EntropyMode
            },
            Anchor {
                name: String
            },
            Rewind {
                target: String,
                anchor: String
            },
            Commit {
                vars: Vec<String>
            },
            Watchdog {
                target: String,
                timeout_ms: u64,
                recovery_jump: Option<usize>
            },
            Speculate {
                max_ms: u64,
                fallback_target: usize
            },
            EndSpeculate {
                max_ms: u64,
                fallback_target: usize
            },
            Collapse,
            Select {
                max_ms: u64,
                cases: Vec<$crate::IrSelectCase>,
                timeout_target: Option<usize>
            },
            MatchEntropy {
                target: $crate::Reg,
                valid_target: Option<usize>,
                decayed_target: Option<usize>,
                pending_target: Option<usize>,
                consumed_target: Option<usize>
            },
            RelativisticBlock {
                target: String,
                block_pc: usize,
                block_len: usize
            },
            PeriodicEpoch {
                interval_ms: u64,
                block_pc: usize,
                block_len: usize
            },
            EndPeriodicEpoch {
                interval_ms: u64
            },
            FreezeBaseWatermark,
            ResetBaseWatermark,
            SetSaturationPolicy {
                target: causm_core::PolicyTarget,
                policy: causm_core::SaturationPolicy
            },
            ArenaIntrospect {
                dest: $crate::Reg,
                kind: causm_core::ArenaIntrospect
            },
            CapabilityCheck {
                dest: $crate::Reg,
                capability: causm_core::Capability
            },
            SpeculationMode {
                mode: causm_core::SpeculationCommitMode
            },

            // Channels & Communication
            OpenChan {
                name: String,
                capacity: usize,
                decay_after_ms: Option<u64>
            },
            ChanSend {
                chan_id: String,
                src: $crate::Reg
            },
            ChanRecv {
                dest: $crate::Reg,
                chan_id: String
            },
            AwaitChan {
                chan_id: String
            },

            // Structural Access
            StructLit {
                dest: $crate::Reg,
                fields: std::collections::HashMap<String, $crate::Reg>,
                type_name: Option<String>
            },
            TopologyLit {
                dest: $crate::Reg,
                fields: std::collections::HashMap<String, $crate::Reg>
            },
            ArrayLit {
                dest: $crate::Reg,
                elements: Vec<$crate::Reg>
            },
            ArrayRepeat {
                dest: $crate::Reg,
                value: $crate::Reg,
                count: $crate::Reg
            },
            ArraySlice {
                dest: $crate::Reg,
                target: $crate::Reg,
                start: Option<$crate::Reg>,
                end: Option<$crate::Reg>,
                inclusive: bool
            },
            FieldAccess {
                dest: $crate::Reg,
                target: $crate::Reg,
                field: String
            },
            FieldUpdate {
                target: $crate::Reg,
                field: String,
                src: $crate::Reg
            },
            Syscall {
                dest: $crate::Reg,
                target: causm_core::SyscallTarget,
                args: Vec<$crate::Reg>,
                duration_ms: Option<u64>
            },
            AutoDrop {
                target: $crate::Reg,
                spec: causm_core::types::AutoDropSpec
            },
            IndexAccess {
                dest: $crate::Reg,
                target: $crate::Reg,
                index: $crate::Reg
            },
            IndexFieldUpdate {
                target: $crate::Reg,
                index: $crate::Reg,
                field: String,
                src: $crate::Reg
            },
            TypeAssert {
                dest: $crate::Reg,
                src: $crate::Reg,
                type_name: String
            },
            TypeCast {
                dest: $crate::Reg,
                src: $crate::Reg,
                target_type: causm_core::TypeName
            },
            AssertState {
                src: $crate::Reg,
                state: String
            },
            TryTypeAssert {
                dest: $crate::Reg,
                src: $crate::Reg,
                type_name: String,
                success: $crate::Reg
            },
            TryEnumVariant {
                dest: $crate::Reg,
                src: $crate::Reg,
                enum_name: Option<String>,
                variant_name: String,
                success: $crate::Reg
            },

            // Misc
            Print {
                src: $crate::Reg
            },
            Debug {
                src: $crate::Reg
            },
            AssertTime {
                op: causm_core::BinaryOperator,
                limit_ms: u64
            },
            Slice {
                ms: u64
            },
            Break,
            LoopTick,
            EndLoopTick,
            Capability {
                cap: causm_core::Capability
            },
            For {
                dest_cond: $crate::Reg,
                item_reg: $crate::Reg,
                item_name: String,
                mode: causm_core::ParamMode,
                source: $crate::Reg,
                pacing_ms: Option<u64>,
                max_ms: Option<u64>
            },
            EndFor,
            SplitMap {
                item_reg: $crate::Reg,
                item_name: String,
                mode: causm_core::ParamMode,
                source: $crate::Reg,
                reconcile: Option<causm_core::MergeResolution>
            },
            EndSplitMap,
            Defer {
                dest: $crate::Reg,
                cap: causm_core::Capability,
                deadline_ms: u64
            },
            Await {
                target: $crate::Reg
            },
            Loop {
                max_ms: u64
            },
            EndLoop {
                max_ms: u64
            },
            LoopTickOn {
                chan_id: String
            },
            While {
                max_ms: u64
            },
            EndWhile {
                max_ms: u64
            },
            ForStep {
                dest_cond: $crate::Reg,
                item_reg: $crate::Reg,
                item_name: String,
                source: $crate::Reg,
                step_ms: Option<u64>
            },
            EndForStep,
            ArrayLen {
                dest: $crate::Reg,
                src: $crate::Reg
            }
        }
    };
}

macro_rules! define_instruction_enum {
    ($($name:ident $({ $($field:ident: $type:ty),* })?),*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Instruction {
            $($name $({ $($field: $type),* })?),*
        }
    };
}

instructions!(define_instruction_enum);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrProgram {
    pub blocks: Vec<IrBlock>,
    pub routines: HashMap<String, IrRoutine>,
    pub symbols: HashMap<String, Reg>,
    pub type_decay_limits: HashMap<String, u64>,
    pub auto_drop_specs: HashMap<String, causm_core::types::AutoDropSpec>,
    pub struct_extends: HashMap<String, String>,
    pub decay_handlers: HashMap<String, Vec<Instruction>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignBinding {
    pub lib_name: String,
    pub abi: String,
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrRoutine {
    pub params: Vec<(causm_core::ParamMode, String, causm_core::types::Type)>,
    pub return_type: causm_core::types::Type,
    pub taking_ms: Option<u64>,
    pub foreign_binding: Option<ForeignBinding>,
    pub instructions: Vec<Instruction>,
    pub spans: Vec<Option<causm_core::Span>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrBlock {
    pub time: TimeCoordinate,
    pub entropy_mode: Option<causm_core::EntropyMode>,
    pub instructions: Vec<Instruction>,
    pub spans: Vec<Option<causm_core::Span>>,
}

impl std::fmt::Display for IrProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, routine) in &self.routines {
            writeln!(f, "routine {} taking {:?}ms:", name, routine.taking_ms)?;
            for (i, instr) in routine.instructions.iter().enumerate() {
                let span_str = match routine.spans.get(i) {
                    Some(Some(span)) => {
                        format!(" // span: {}-{}", span.start, span.end)
                    }
                    _ => "".to_string(),
                };
                writeln!(f, "  {:>3}: {:?}{}", i, instr, span_str)?;
            }
        }
        for block in &self.blocks {
            writeln!(f, "@{}:", block.time)?;
            for (i, instr) in block.instructions.iter().enumerate() {
                let span_str = match block.spans.get(i) {
                    Some(Some(span)) => {
                        format!(" // span: {}-{}", span.start, span.end)
                    }
                    _ => "".to_string(),
                };
                writeln!(f, "  {:>3}: {:?}{}", i, instr, span_str)?;
            }
        }
        Ok(())
    }
}

pub mod cfg;
pub use cfg::{BasicBlock, Terminator, CFG};

pub mod ssa;
pub use ssa::{
    SsaBasicBlock, SsaCFG, SsaInstruction, SsaPhiNode, SsaReg, SsaTerminator,
    SsaTransformer,
};

pub mod optimize;
pub mod properties;
pub use optimize::optimize_program;
