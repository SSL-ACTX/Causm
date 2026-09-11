use crate::symbol::Symbol;
use crate::{BinaryOperator, Span, UnaryOperator};
use serde::{Deserialize, Serialize};

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct ExprId(pub u32);

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct StmtId(pub u32);

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct TypeId(pub u32);

#[derive(
    Copy, Clone, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize,
)]
pub struct SliceRange<T> {
    pub start: u32,
    pub end: u32,
    _marker: std::marker::PhantomData<T>,
}

impl<T> SliceRange<T> {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        (self.end.saturating_sub(self.start)) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    pub fn as_range(&self) -> std::ops::Range<usize> {
        self.start as usize..self.end as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralKind {
    Integer(i64),
    Float(String),
    String(String),
    Boolean(bool),
    Duration(u64),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchArmNode {
    pub pattern: Symbol,
    pub guard: Option<ExprId>,
    pub body: SliceRange<StmtId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FStringPartNode {
    Text(String),
    Expr(ExprId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldAssignNode {
    pub field: Symbol,
    pub expr: ExprId,
    pub type_name: Option<Symbol>,
    pub is_const: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArenaMergeResolution {
    pub rules: std::collections::HashMap<String, crate::ResolutionStrategy>,
    pub auto: bool,
    pub fallback: Option<SliceRange<StmtId>>,
    pub taking_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExprNode {
    Literal(LiteralKind),
    Identifier(Symbol),
    BinaryOp {
        left: ExprId,
        right: ExprId,
        op: BinaryOperator,
    },
    UnaryOp {
        expr: ExprId,
        op: UnaryOperator,
    },
    FieldAccess {
        target: ExprId,
        field: Symbol,
    },
    MethodCall {
        target: ExprId,
        method: Symbol,
        args: SliceRange<ExprId>,
    },
    EnumVariant {
        enum_name: Symbol,
        variant_name: Symbol,
        args: SliceRange<ExprId>,
    },
    If {
        cond: ExprId,
        then_branch: StmtId,
        else_branch: Option<StmtId>,
    },
    Match {
        target: ExprId,
        arms: SliceRange<MatchArmNode>,
    },
    ArrayRepeat {
        value: ExprId,
        count: ExprId,
    },
    ArraySlice {
        target: ExprId,
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },
    Tuple(SliceRange<ExprId>),
    StructLit {
        type_sym: Option<Symbol>,
        fields: SliceRange<FieldAssignNode>,
    },
    Turbofish {
        expr: ExprId,
        type_args: SliceRange<TypeId>,
    },
    Call {
        routine: ExprId,
        args: SliceRange<ExprId>,
    },
    Try(ExprId),
    ArrayLit(SliceRange<ExprId>),
    IndexAccess {
        target: ExprId,
        index: ExprId,
    },
    Pipeline {
        target: ExprId,
        stage: ExprId,
    },
    TypeCast {
        expr: ExprId,
        target_type: Symbol,
    },
    TypeAssertion {
        target: ExprId,
        cast_type: Symbol,
    },
    FString(SliceRange<FStringPartNode>),
    Ref(ExprId),
    Syscall {
        target: Symbol,
        args: SliceRange<ExprId>,
        duration_ms: Option<u64>,
    },
    ChanRecv(Symbol),
    ArenaIntrospect(Symbol),
    Await(ExprId),
    Defer {
        capability: Symbol,
        duration_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StmtNode {
    Expr(ExprId),
    Let {
        target: Symbol,
        is_mut: bool,
        type_annotation: Option<Symbol>,
        init: Option<ExprId>,
        lifetime: Option<crate::LifetimeAnnotation>,
    },
    Destructure {
        fields: SliceRange<Symbol>,
        expr: ExprId,
    },
    Assign {
        target: Symbol,
        value: ExprId,
    },
    FieldUpdate {
        target: ExprId,
        field: Symbol,
        value: ExprId,
    },
    Block(SliceRange<StmtId>),
    Return(Option<ExprId>),
    Yield(ExprId),
    RoutineDef {
        name: Symbol,
        params: SliceRange<Symbol>,
        return_type: Option<Symbol>,
        taking_ms: Option<u64>,
        state_constraint: Option<(Symbol, Symbol)>,
        required_capabilities: Vec<Symbol>,
        body: SliceRange<StmtId>,
    },
    Isolate {
        name: Symbol,
        body: SliceRange<StmtId>,
    },
    Send {
        target: Symbol,
        payload: ExprId,
    },
    Import {
        path: Symbol,
        alias: Option<Symbol>,
    },
    FromImport {
        path: Symbol,
        symbols: SliceRange<Symbol>,
    },
    TypeDecl {
        name: Symbol,
        extends: Option<Symbol>,
        fields: SliceRange<FieldAssignNode>,
        decay_after_ms: Option<u64>,
        auto_drop: Option<crate::types::AutoDropSpec>,
    },
    InterfaceDecl {
        name: Symbol,
        extends: SliceRange<Symbol>,
        methods: SliceRange<StmtId>,
    },
    EnumDecl {
        name: Symbol,
        variants: Vec<crate::EnumVariantDef>,
    },
    MacroDef {
        name: Symbol,
        params: Vec<crate::MacroParam>,
        body_template: String,
    },
    EnableResource {
        resource: Symbol,
        amount: u64,
        unit: Option<Symbol>,
    },
    Loop {
        max_ms: Option<u64>,
        step_ms: Option<u64>,
        is_tick: bool,
        body: SliceRange<StmtId>,
    },
    LoopOn {
        target: ExprId,
        body: SliceRange<StmtId>,
    },
    While {
        cond: ExprId,
        max_ms: Option<u64>,
        step_ms: Option<u64>,
        body: SliceRange<StmtId>,
    },
    If {
        cond: ExprId,
        then_branch: SliceRange<StmtId>,
        else_branch: Option<SliceRange<StmtId>>,
        reconcile_auto: bool,
    },
    IfLet {
        pattern: Symbol,
        expr: ExprId,
        then_branch: SliceRange<StmtId>,
        else_branch: Option<SliceRange<StmtId>>,
        reconcile_auto: bool,
    },
    Match {
        target: ExprId,
        arms: SliceRange<MatchArmNode>,
    },
    Using {
        binding: Symbol,
        resource: ExprId,
        body: SliceRange<StmtId>,
    },
    Print(SliceRange<ExprId>),
    Debug(ExprId),
    AssertTime {
        operator: crate::BinaryOperator,
        limit_ms: u64,
        fallback: Option<SliceRange<StmtId>>,
    },
    Slice(ExprId),
    Entangle(SliceRange<Symbol>),
    Commit(SliceRange<StmtId>),
    Speculate {
        max_ms: u64,
        body: SliceRange<StmtId>,
        fallback: Option<SliceRange<StmtId>>,
    },
    Collapse,
    Capability(crate::Capability),
    Break,
    Continue,
    Split {
        parent: Symbol,
        branches: SliceRange<Symbol>,
    },
    Merge {
        branches: SliceRange<Symbol>,
        target: Symbol,
        resolutions: ArenaMergeResolution,
    },
    ForeignBlock {
        lib_name: Symbol,
        abi: Symbol,
        routines: SliceRange<StmtId>,
    },
    For {
        var_name: Symbol,
        mode: crate::ParamMode,
        iter_expr: ExprId,
        pacing_ms: Option<u64>,
        max_ms: Option<u64>,
        body: SliceRange<StmtId>,
    },
    DecayHandler {
        type_name: Symbol,
        body: SliceRange<StmtId>,
    },
    ForStep {
        var_name: Symbol,
        start_expr: ExprId,
        end_expr: ExprId,
        step_ms: u64,
        body: SliceRange<StmtId>,
    },
    Lease {
        binding: Symbol,
        source: Symbol,
        duration_ms: u64,
        body: SliceRange<StmtId>,
        reconcile_auto: bool,
    },
    Anchor(Symbol),
    RewindTo(Symbol),
    OnDecay {
        target: Symbol,
        body: SliceRange<StmtId>,
    },
    State {
        name: Symbol,
        value: ExprId,
    },
    Policy {
        target: Symbol,
        kind: Symbol,
    },
    Select {
        max_ms: u64,
        cases: SliceRange<StmtId>,
    },
    TimelineBlock {
        coord: crate::TimeCoordinate,
        directives: Vec<crate::BlockDirective>,
        body: SliceRange<StmtId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeNode {
    Named(Symbol),
    Reference(TypeId),
    Array(TypeId),
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstArena {
    pub expressions: Vec<ExprNode>,
    pub statements: Vec<StmtNode>,
    pub types: Vec<TypeNode>,
    pub expr_pool: Vec<ExprId>,
    pub stmt_pool: Vec<StmtId>,
    pub root_statements: Vec<StmtId>,
    pub type_pool: Vec<TypeId>,
    pub symbol_pool: Vec<Symbol>,
    pub match_arms_pool: Vec<MatchArmNode>,
    pub field_assigns_pool: Vec<FieldAssignNode>,
    pub fstring_parts_pool: Vec<FStringPartNode>,
    pub spans: Vec<Span>,
    pub stmt_spans: Vec<Span>,
    pub stmt_attributes: std::collections::HashMap<u32, Vec<crate::Attribute>>,
}

impl AstArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_expr(&mut self, node: ExprNode, span: Span) -> ExprId {
        let id = ExprId(self.expressions.len() as u32);
        self.expressions.push(node);
        self.spans.push(span);
        id
    }

    pub fn alloc_stmt(&mut self, node: StmtNode, span: Span) -> StmtId {
        let id = StmtId(self.statements.len() as u32);
        self.statements.push(node);
        self.stmt_spans.push(span);
        id
    }

    pub fn alloc_type(&mut self, node: TypeNode) -> TypeId {
        let id = TypeId(self.types.len() as u32);
        self.types.push(node);
        id
    }
}
