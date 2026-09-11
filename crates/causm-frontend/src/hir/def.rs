//! Canonical High-Level Intermediate Representation (HIR) Definitions
//!
//! Complete, strongly-typed representation where all surface syntax sugar
//! (`using`, `f"..."`, macros, `@derive`) has been lowered into canonical forms.

use causm_core::{
    Attribute, AutoDropSpec, BinaryOperator, Capability, DecayedPattern,
    EntropyMode, LifetimeAnnotation, MergeResolution, ParamDecl, ParamMode,
    Pattern, PolicyTarget, SaturationPolicy, Span, SpeculationCommitMode,
    SyscallTarget, TimeCoordinate, TypeFieldDef, TypeName, TypeParam,
    UnaryOperator,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirProgram {
    pub timelines: Vec<HirTimelineBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirTimelineBlock {
    pub time: TimeCoordinate,
    pub no_z3: bool,
    pub entropy_mode: Option<EntropyMode>,
    pub statements: Vec<HirSpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirSpannedStatement {
    pub stmt: HirStatement,
    pub span: Span,
    pub attributes: Vec<Attribute>,
}

impl HirSpannedStatement {
    pub fn new(stmt: HirStatement, span: Span) -> Self {
        Self {
            stmt,
            span,
            attributes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum HirStatement {
    Assignment {
        target: String,
        mutable: bool,
        var_type: Option<TypeName>,
        lifetime: Option<LifetimeAnnotation>,
        expr: HirExpression,
    },
    DestructureAssignment {
        fields: Vec<(String, String)>,
        mutable: bool,
        expr: HirExpression,
    },
    Expression(HirExpression),
    Return(Option<HirExpression>),
    Yield(Option<HirExpression>),
    Print(Vec<HirExpression>),
    Debug(HirExpression),
    AssertTime {
        operator: BinaryOperator,
        limit_ms: u64,
        fallback: Option<Vec<HirSpannedStatement>>,
    },
    Break,
    Collapse,
    If {
        binding: Option<String>,
        condition: HirExpression,
        then_branch: Vec<HirSpannedStatement>,
        else_branch: Option<Vec<HirSpannedStatement>>,
        reconcile: Option<MergeResolution>,
    },
    IfLet {
        pattern: Pattern,
        expr: HirExpression,
        then_branch: Vec<HirSpannedStatement>,
        else_branch: Option<Vec<HirSpannedStatement>>,
        reconcile: Option<MergeResolution>,
    },
    Match {
        target: HirExpression,
        arms: Vec<HirMatchArm>,
    },
    MatchEntropy {
        target: HirExpression,
        valid_branch: Option<(DecayedPattern, Option<HirExpression>, Vec<HirSpannedStatement>)>,
        decayed_branch: Option<(DecayedPattern, Option<HirExpression>, Vec<HirSpannedStatement>)>,
        pending_branch: Option<(DecayedPattern, Option<HirExpression>, Vec<HirSpannedStatement>)>,
        consumed_branch: Option<(Option<HirExpression>, Vec<HirSpannedStatement>)>,
    },
    While {
        condition: HirExpression,
        is_valid_check: bool,
        max_ms: u64,
        body: Vec<HirSpannedStatement>,
    },
    Loop {
        max_ms: u64,
        body: Vec<HirSpannedStatement>,
    },
    LoopTick {
        body: Vec<HirSpannedStatement>,
    },
    LoopOn {
        target: HirExpression,
        body: Vec<HirSpannedStatement>,
    },
    For {
        item_name: String,
        mode: ParamMode,
        source: String,
        body: Vec<HirSpannedStatement>,
        pacing_ms: Option<u64>,
        max_ms: Option<u64>,
    },
    ForStep {
        item_name: String,
        source: HirExpression,
        step_ms: Option<u64>,
        body: Vec<HirSpannedStatement>,
    },
    RoutineDef {
        name: String,
        params: Vec<ParamDecl>,
        return_type: Option<TypeName>,
        taking_ms: Option<u64>,
        state_constraint: Option<(String, String)>,
        required_capabilities: Vec<Capability>,
        body: Vec<HirSpannedStatement>,
    },
    TypeDecl {
        name: String,
        extends: Option<String>,
        fields: HashMap<String, TypeFieldDef>,
        decay_after_ms: Option<u64>,
        auto_drop: Option<AutoDropSpec>,
        scoped_branch: Option<String>,
    },
    EnumDecl {
        name: String,
        variants: Vec<causm_core::EnumVariantDef>,
    },
    InterfaceDecl {
        name: String,
        extends: Vec<String>,
        methods: Vec<causm_core::InterfaceMethod>,
    },
    DecayHandler {
        type_name: String,
        body: Vec<HirSpannedStatement>,
    },
    Commit(Vec<HirSpannedStatement>),
    Anchor(String),
    Rewind(String),
    Lease {
        binding: String,
        source: String,
        duration_ms: u64,
        body: Vec<HirSpannedStatement>,
        reconcile: Option<MergeResolution>,
    },
    Select {
        max_ms: u64,
        cases: Vec<HirSelectCase>,
        timeout: Option<Vec<HirSpannedStatement>>,
        reconcile: Option<MergeResolution>,
    },
    Entangle {
        variables: Vec<String>,
    },
    Speculate {
        max_ms: u64,
        body: Vec<HirSpannedStatement>,
        fallback: Option<Vec<HirSpannedStatement>>,
    },
    SpeculationMode(SpeculationCommitMode),
    Slice {
        milliseconds: u64,
    },
    Split {
        parent: String,
        branches: Vec<String>,
    },
    Merge {
        branches: Vec<String>,
        target: String,
        resolutions: MergeResolution,
    },
    SplitMap {
        item_name: String,
        mode: ParamMode,
        source: String,
        body: Vec<HirSpannedStatement>,
        reconcile: Option<MergeResolution>,
    },
    Send {
        value_id: String,
        target_branch: String,
    },
    FieldUpdate {
        target: HirExpression,
        field: String,
        value: HirExpression,
    },
    StateDecl {
        target: String,
        var_type: Option<TypeName>,
        expr: HirExpression,
    },
    PolicyStmt {
        target: PolicyTarget,
        policy: SaturationPolicy,
    },
    AutoDrop {
        target: String,
    },
    Consume {
        target: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirMatchArm {
    pub pattern: Pattern,
    pub guard: Option<HirExpression>,
    pub body: Vec<HirSpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirSelectCase {
    pub binding: String,
    pub source: HirExpression,
    pub body: Vec<HirSpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum HirExpression {
    Integer(i64),
    Float(u64),
    Boolean(bool),
    Literal(String),
    Identifier(String),
    Null,
    BinaryOp {
        op: BinaryOperator,
        left: Box<HirExpression>,
        right: Box<HirExpression>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<HirExpression>,
    },
    Call {
        routine: String,
        args: Vec<HirExpression>,
    },
    MethodCall {
        target: Box<HirExpression>,
        method: String,
        args: Vec<HirExpression>,
    },
    GenericStaticCall {
        type_name: String,
        type_args: Vec<TypeParam>,
        method: String,
        args: Vec<HirExpression>,
    },
    FieldAccess {
        target: Box<HirExpression>,
        field: String,
    },
    IndexAccess {
        target: Box<HirExpression>,
        index: Box<HirExpression>,
    },
    ArrayLiteral(Vec<HirExpression>),
    ArrayRepeat {
        value: Box<HirExpression>,
        count: Box<HirExpression>,
    },
    ArraySlice {
        target: Box<HirExpression>,
        start: Option<Box<HirExpression>>,
        end: Option<Box<HirExpression>>,
        inclusive: bool,
    },
    Tuple(Vec<HirExpression>),
    StructLit(Option<String>, HashMap<String, HirExpression>),
    TopologyLit(HashMap<String, HirExpression>),
    EnumVariant {
        enum_name: String,
        variant_name: String,
        args: Vec<HirExpression>,
    },
    If {
        condition: Box<HirExpression>,
        then_branch: Box<HirExpression>,
        else_branch: Box<HirExpression>,
    },
    Match {
        target: Box<HirExpression>,
        arms: Vec<HirExprMatchArm>,
    },
    ToStr(Box<HirExpression>),
    StrBytes(Box<HirExpression>),
    Len(Box<HirExpression>),
    RefOp(Box<HirExpression>),
    CloneOp(String),
    ChannelReceive(String),
    TypeAssertion {
        target: Box<HirExpression>,
        cast_type: TypeName,
    },
    TypeCast {
        expr: Box<HirExpression>,
        target_type: TypeName,
    },
    TryUnwrap(Box<HirExpression>),
    Syscall {
        target: SyscallTarget,
        args: Vec<HirExpression>,
        duration_ms: Option<u64>,
    },
    ArenaIntrospect(causm_core::ArenaIntrospect),
    CapabilityCheck(Capability),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HirExprMatchArm {
    pub pattern: Pattern,
    pub guard: Option<HirExpression>,
    pub body: Box<HirExpression>,
}
