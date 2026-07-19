use causm_core::TimeCoordinate;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(pub u32);

impl std::fmt::Display for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            Move {
                dest: $crate::Reg,
                src: $crate::Reg
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
                item_name: String,
                mode: causm_core::ParamMode,
                source: $crate::Reg,
                pacing_ms: Option<u64>,
                max_ms: Option<u64>
            },
            EndFor,
            SplitMap {
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
                item_name: String,
                source: $crate::Reg,
                step_ms: u64
            },
            EndForStep,
            NetworkRequest {
                domain: String
            }
        }
    };
}

macro_rules! define_instruction_enum {
    ($($name:ident $({ $($field:ident: $type:ty),* })?),*) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Instruction {
            $($name $({ $($field: $type),* })?),*
        }
    };
}

instructions!(define_instruction_enum);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrProgram {
    pub blocks: Vec<IrBlock>,
    pub routines: HashMap<String, IrRoutine>,
    pub symbols: HashMap<String, Reg>,
    pub type_decay_limits: HashMap<String, u64>,
    pub struct_extends: HashMap<String, String>,
    pub decay_handlers: HashMap<String, Vec<Instruction>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrRoutine {
    pub params: Vec<(causm_core::ParamMode, String, causm_core::types::Type)>,
    pub return_type: causm_core::types::Type,
    pub taking_ms: Option<u64>,
    pub instructions: Vec<Instruction>,
    pub spans: Vec<Option<causm_core::Span>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrBlock {
    pub time: TimeCoordinate,
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
pub use optimize::optimize_program;
