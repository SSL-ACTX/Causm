// src/ast.rs

use crate::types::AutoDropSpec;
pub mod types;
pub mod value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub timelines: Vec<TimelineBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedStatement {
    pub stmt: Statement,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineBlock {
    pub time: TimeCoordinate,
    pub no_z3: bool,
    pub entropy_mode: Option<EntropyMode>,
    pub statements: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeCoordinate {
    Global(u64),
    Relative(u64),
    Branch(String),
}

impl std::fmt::Display for TimeCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeCoordinate::Global(t) => write!(f, "Global({})", t),
            TimeCoordinate::Relative(t) => write!(f, "+{}ms", t),
            TimeCoordinate::Branch(b) => write!(f, "{}", b),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyMode {
    Deterministic,
    Chaos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculationCommitMode {
    Selective,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyscallTarget {
    Number(i64),
    Symbol(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamDecl {
    pub mode: ParamMode,
    pub name: String,
    pub typ: Option<TypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<ParamDecl>,
    pub return_type: Option<TypeName>,
    pub taking_ms: Option<u64>,
    pub default_body: Option<Vec<SpannedStatement>>,
    pub state_constraint: Option<(String, String)>,
}

#[macro_export]
macro_rules! statements {
    ($macro:ident) => {
        $macro! {
            Isolate(IsolateBlock),
            Split {
                parent: String,
                branches: Vec<String>
            },
            Merge {
                branches: Vec<String>,
                target: String,
                resolutions: MergeResolution
            },
            Anchor(String),
            Rewind(String),
            Commit(Vec<SpannedStatement>),
            Assignment {
                target: String,
                mutable: bool,
                var_type: Option<TypeName>,
                lifetime: Option<LifetimeAnnotation>,
                expr: Expression
            },
            EnumDecl {
                name: String,
                variants: Vec<EnumVariantDef>
            },
            TypeDecl {
                name: String,
                extends: Option<String>,
                fields: std::collections::HashMap<String, TypeFieldDef>,
                decay_after_ms: Option<u64>,
                auto_drop: Option<AutoDropSpec>,
                scoped_branch: Option<String>
            },
            InterfaceDecl {
                name: String,
                extends: Vec<String>,
                methods: Vec<InterfaceMethod>
            },
            DecayHandler {
                type_name: String,
                body: Vec<SpannedStatement>
            },
            AssertTime {
                operator: BinaryOperator,
                limit_ms: u64,
                fallback: Option<Vec<SpannedStatement>>
            },
            Send {
                value_id: String,
                target_branch: String
            },
            Expression(Expression),
            Capability(Capability),
            ChannelOpen {
                name: String,
                capacity: usize,
                decay_after_ms: Option<u64>
            },
            ChannelSend {
                chan_id: String,
                value_id: String
            },
            RelativisticBlock {
                time: TimeCoordinate,
                body: Vec<SpannedStatement>
            },
            DirectiveBlock {
                directives: Vec<BlockDirective>,
                body: Vec<SpannedStatement>
            },
            NetworkRequest {
                domain: String
            },
            Watchdog {
                target: String,
                timeout_ms: u64,
                recovery: Vec<SpannedStatement>
            },
            Speculate {
                max_ms: u64,
                body: Vec<SpannedStatement>,
                fallback: Option<Vec<SpannedStatement>>
            },
            Collapse,
            SpeculationMode(SpeculationCommitMode),
            Select {
                max_ms: u64,
                cases: Vec<SelectCase>,
                timeout: Option<Vec<SpannedStatement>>,
                reconcile: Option<MergeResolution>
            },
            MatchEntropy {
                target: Expression,
                valid_branch: Option<(DecayedPattern, Option<Expression>, Vec<SpannedStatement>)>,
                decayed_branch: Option<(DecayedPattern, Option<Expression>, Vec<SpannedStatement>)>,
                pending_branch: Option<(DecayedPattern, Option<Expression>, Vec<SpannedStatement>)>,
                consumed_branch: Option<(Option<Expression>, Vec<SpannedStatement>)>
            },
            Await(String),
            AwaitChan(String),
            If {
                binding: Option<String>,
                condition: Expression,
                then_branch: Vec<SpannedStatement>,
                else_branch: Option<Vec<SpannedStatement>>,
                reconcile: Option<MergeResolution>
            },
            Break,
            Inspect {
                binding: String,
                target: String,
                body: Vec<SpannedStatement>
            },
            Lease {
                binding: String,
                source: String,
                duration_ms: u64,
                body: Vec<SpannedStatement>,
                reconcile: Option<MergeResolution>
            },
            Loop {
                max_ms: u64,
                body: Vec<SpannedStatement>
            },
            LoopTick {
                body: Vec<SpannedStatement>
            },
            LoopTickOn {
                channel: String,
                body: Vec<SpannedStatement>
            },
            While {
                condition: Expression,
                is_valid_check: bool,
                max_ms: u64,
                body: Vec<SpannedStatement>
            },
            ForStep {
                item_name: String,
                source: Expression,
                step_ms: u64,
                body: Vec<SpannedStatement>
            },
            Slice {
                milliseconds: u64
            },
            For {
                item_name: String,
                mode: ParamMode,
                source: String,
                body: Vec<SpannedStatement>,
                pacing_ms: Option<u64>,
                max_ms: Option<u64>
            },
            SplitMap {
                item_name: String,
                mode: ParamMode,
                source: String,
                body: Vec<SpannedStatement>,
                reconcile: Option<MergeResolution>
            },
            Yield(String),
            Print(Expression),
            Debug(Expression),
            RoutineDef {
                name: String,
                params: Vec<ParamDecl>,
                return_type: Option<TypeName>,
                taking_ms: Option<u64>,
                state_constraint: Option<(String, String)>,
                body: Vec<SpannedStatement>
            },
            Return(Option<String>),
            AcausalReset {
                target: String,
                anchor_name: String
            },
            Entangle {
                variables: Vec<String>
            },
            Import {
                path: String,
                alias: Option<String>
            },
            FromImport {
                path: String,
                symbols: Vec<(String, Option<String>)>
            },
            ForeignBlock {
                lib_name: String,
                abi: String,
                routines: Vec<SpannedStatement>
            },
            FieldUpdate {
                target: Expression,
                field: String,
                value: Expression
            }
        }
    };
}

macro_rules! define_statement_enum {
    ($($name:ident $({ $($field:ident: $type:ty),* })? $(( $($tuple_type:ty),* ))?),*) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        #[allow(clippy::large_enum_variant)]
        pub enum Statement {
            $($name $({ $($field: $type),* })? $(( $($tuple_type),* ))?),*
        }
    };
}

statements!(define_statement_enum);

impl Statement {
    pub fn estimate_cost<F>(&self, mut estimate_block: F) -> u64
    where
        F: FnMut(&[SpannedStatement]) -> u64,
    {
        let base = 1;
        let extra = match self {
            Statement::NetworkRequest { .. } => 5,
            Statement::Split { .. }
            | Statement::Merge { .. }
            | Statement::Anchor(_)
            | Statement::Rewind(_)
            | Statement::Commit(_)
            | Statement::Send { .. }
            | Statement::ChannelOpen { .. }
            | Statement::ChannelSend { .. }
            | Statement::AcausalReset { .. }
            | Statement::Capability(_)
            | Statement::Assignment { .. }
            | Statement::TypeDecl { .. }
            | Statement::EnumDecl { .. }
            | Statement::InterfaceDecl { .. }
            | Statement::FieldUpdate { .. }
            | Statement::Expression(_)
            | Statement::Print(_) => 0,
            Statement::DecayHandler { body, .. } => estimate_block(body),
            Statement::AssertTime { fallback, .. } => {
                fallback.as_ref().map(|b| estimate_block(b)).unwrap_or(0)
            }
            Statement::RelativisticBlock { body, .. }
            | Statement::DirectiveBlock { body, .. } => estimate_block(body),
            Statement::Isolate(block) => estimate_block(&block.body),
            Statement::Watchdog { recovery, .. } => estimate_block(recovery),
            Statement::Debug(_) => 1,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => estimate_block(then_branch)
                .max(else_branch.as_ref().map(|b| estimate_block(b)).unwrap_or(0)),
            Statement::For { pacing_ms, .. } => pacing_ms.unwrap_or(1),
            Statement::ForStep { step_ms, .. } => *step_ms,
            Statement::Speculate { body, fallback, .. } => {
                let body_cost = estimate_block(body);
                let fallback_cost =
                    fallback.as_ref().map(|b| estimate_block(b)).unwrap_or(0);
                body_cost + fallback_cost
            }
            Statement::Select {
                max_ms,
                cases,
                timeout,
                ..
            } => {
                let case_max_cost = cases
                    .iter()
                    .map(|c| estimate_block(&c.body))
                    .max()
                    .unwrap_or(0);
                let timeout_cost =
                    timeout.as_ref().map(|b| estimate_block(b)).unwrap_or(0);
                *max_ms + case_max_cost.max(timeout_cost)
            }
            Statement::MatchEntropy {
                valid_branch,
                decayed_branch,
                pending_branch,
                consumed_branch,
                ..
            } => {
                let valid_cost = valid_branch
                    .as_ref()
                    .map(|(_, _, b)| estimate_block(b))
                    .unwrap_or(0);
                let decayed_cost = decayed_branch
                    .as_ref()
                    .map(|(_, _, b)| estimate_block(b))
                    .unwrap_or(0);
                let pending_cost = pending_branch
                    .as_ref()
                    .map(|(_, _, b)| estimate_block(b))
                    .unwrap_or(0);
                let consumed_cost = consumed_branch
                    .as_ref()
                    .map(|(_, b)| estimate_block(b))
                    .unwrap_or(0);
                valid_cost
                    .max(decayed_cost)
                    .max(pending_cost)
                    .max(consumed_cost)
            }
            Statement::Collapse => 0,
            Statement::SplitMap { body, .. } => 1 + estimate_block(body),
            Statement::Inspect { body, .. } => estimate_block(body),
            Statement::Lease { duration_ms, .. } => *duration_ms,
            Statement::RoutineDef { taking_ms, .. } => taking_ms.unwrap_or(0),
            Statement::Loop { max_ms, .. } => *max_ms,
            Statement::LoopTick { .. } => 1,
            Statement::LoopTickOn { .. } => 1,
            Statement::While { max_ms, .. } => *max_ms,
            Statement::Slice { .. } => 0,
            Statement::SpeculationMode(_) => 0,
            Statement::Await(_) => 1,
            Statement::AwaitChan(_) => 1,
            Statement::Yield(_)
            | Statement::Break
            | Statement::Entangle { .. }
            | Statement::Import { .. }
            | Statement::FromImport { .. }
            | Statement::ForeignBlock { .. }
            | Statement::Return(_) => 0,
        };
        base + extra
    }
}

#[macro_export]
macro_rules! expressions {
    ($macro:ident) => {
        $macro! {
            Call {
                routine: String,
                args: Vec<Expression>
            },
            MethodCall {
                target: Box<Expression>,
                method: String,
                args: Vec<Expression>,
                resolved_routine: std::cell::RefCell<Option<String>>,
                resolved_budget: std::cell::RefCell<Option<u64>>
            },
            Literal(String),
            Identifier(String),
            FieldAccess {
                target: Box<Expression>,
                field: String
            },
            CloneOp(String),
            RefOp(Box<Expression>),
            StructLit(std::cell::RefCell<Option<String>>, std::collections::HashMap<String, Expression>),
            TopologyLit(std::collections::HashMap<String, Expression>),
            IndexAccess {
                target: Box<Expression>,
                index: Box<Expression>
            },
            ArrayLiteral(Vec<Expression>),
            ChannelReceive(String),
            Integer(i64),
            Float(u64), // Using u64 bits to preserve Eq
            Boolean(bool),
            BinaryOp {
                left: Box<Expression>,
                op: BinaryOperator,
                right: Box<Expression>
            },
            UnaryOp {
                op: UnaryOperator,
                expr: Box<Expression>
            },
            Deferred {
                capability: String,
                params: std::collections::HashMap<String, String>,
                deadline_ms: u64
            },
            TypeAssertion {
                target: Box<Expression>,
                cast_type: TypeName
            },
            TypeCast {
                expr: Box<Expression>,
                target_type: TypeName
            },
            Syscall {
                target: SyscallTarget,
                args: Vec<Expression>,
                duration_ms: Option<u64>
            },
            TryUnwrap(Box<Expression>),
            Null
        }
    };
}

macro_rules! define_expression_enum {
    ($($name:ident $({ $($field:ident: $type:ty),* })? $(( $($tuple_type:ty),* ))?),*) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Expression {
            $($name $({ $($field: $type),* })? $(( $($tuple_type),* ))?),*
        }
    };
}

expressions!(define_expression_enum);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolateBlock {
    pub name: Option<String>,
    pub manifest: Manifest,
    pub body: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    pub cpu_budget_ms: Option<u64>,
    pub slice_ms: Option<u64>,
    pub memory_budget_bytes: Option<u64>,
    pub resource_budgets: std::collections::HashMap<String, u64>,
    pub capabilities: Vec<Capability>,
    pub mode: Option<EntropyMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    pub path: String,
    pub parameters: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResolution {
    pub rules: std::collections::HashMap<String, ResolutionStrategy>,
    pub auto: bool,
    pub fallback: Option<Vec<SpannedStatement>>,
    pub taking_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalReversion {
    pub branch: String,
    pub anchor: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionStrategy {
    FirstWins,
    Priority(String),
    Decay,
    Auto,
    Custom(String),
    TopologyUnion {
        key_rules: std::collections::HashMap<String, ResolutionStrategy>,
        default: Box<ResolutionStrategy>,
        on_invalid: Option<CausalReversion>,
    },
    TopologyIntersect {
        key_rules: std::collections::HashMap<String, ResolutionStrategy>,
        default: Box<ResolutionStrategy>,
        on_invalid: Option<CausalReversion>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectCase {
    pub binding: String,
    pub source: Expression,
    pub body: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifetimeAnnotation {
    Valid,
    Decayed(u64),
    DecayRate(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariantDef {
    pub name: String,
    pub payload_types: Vec<TypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeFieldDef {
    pub typ: TypeName,
    pub is_const: bool,
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockDirective {
    NoZ3,
    Chaos,
    Deterministic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeName {
    Builtin(BuiltinType),
    Custom(String),
    Generic(String, Vec<TypeParam>),
    Optional(Box<TypeName>),
    Union(Vec<TypeName>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeParam {
    Type(TypeName),
    Amount(u64),
    Duration(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinType {
    Integer,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Float,
    F32,
    F64,
    Bool,
    String,
    Struct,
    Topology,
    Array,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamMode {
    Consume,
    Clone,
    Decay,
    Peek,
    Lease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternValue {
    State(String),
    Expr(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecayedPattern {
    Binding(String),
    Fields(std::collections::HashMap<String, PatternValue>),
}
