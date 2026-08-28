// src/ast.rs

use crate::types::AutoDropSpec;
use serde::{Deserialize, Serialize};
pub mod types;
pub mod value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub timelines: Vec<TimelineBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedStatement {
    pub stmt: Statement,
    pub span: Span,
    pub attributes: Vec<Attribute>,
}

impl SpannedStatement {
    pub fn new(stmt: Statement, span: Span) -> Self {
        Self {
            stmt,
            span,
            attributes: Vec::new(),
        }
    }

    pub fn with_attributes(
        stmt: Statement,
        span: Span,
        attributes: Vec<Attribute>,
    ) -> Self {
        Self {
            stmt,
            span,
            attributes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineBlock {
    pub time: TimeCoordinate,
    pub no_z3: bool,
    pub entropy_mode: Option<EntropyMode>,
    pub statements: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeCoordinate {
    Global(u64),
    Relative(u64),
    Branch(String),
    Periodic(u64),
}

impl std::fmt::Display for TimeCoordinate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeCoordinate::Global(t) => write!(f, "Global({})", t),
            TimeCoordinate::Relative(t) => write!(f, "+{}ms", t),
            TimeCoordinate::Branch(b) => write!(f, "{}", b),
            TimeCoordinate::Periodic(t) => write!(f, "@every {}ms", t),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SaturationPolicy {
    EvictDecayed,
    RingBuffer,
    Throttle,
    FailFast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyTarget {
    OnFull,
    OnDeadlineBreach,
    OnOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArenaIntrospect {
    Remaining,
    UsedBytes,
    Capacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntropyMode {
    Deterministic,
    Chaos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeculationCommitMode {
    Selective,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyscallTarget {
    Number(i64),
    Symbol(String),
}

/// The fragment specifier for a macro capture: $name:kind
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MacroParamKind {
    Ident,
    Expr,
    Type,
    Literal,
}

/// One named capture slot in a declarative macro pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroParam {
    pub name: String,
    pub kind: MacroParamKind,
}

/// Compiler attribute kind: @derive, @must_use, @inline, @test, or custom
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttributeKind {
    Derive(Vec<String>),
    MustUse(Option<String>),
    Inline,
    Test,
    Custom { name: String, args: Vec<String> },
}

/// A parsed compiler attribute attached to items: @name(args)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    pub kind: AttributeKind,
    pub span: Span,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamDecl {
    pub mode: ParamMode,
    pub name: String,
    pub typ: Option<TypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<ParamDecl>,
    pub return_type: Option<TypeName>,
    pub taking_ms: Option<u64>,
    #[serde(skip)]
    pub default_body: Option<Vec<SpannedStatement>>,
    pub state_constraint: Option<(String, String)>,
    #[serde(default)]
    pub required_capabilities: Vec<Capability>,
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
            DestructureAssignment {
                fields: Vec<(String, String)>,
                mutable: bool,
                expr: Expression
            },
            Using {
                binding: String,
                resource: Expression,
                body: Vec<SpannedStatement>
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
            RelativisticBlock {
                time: TimeCoordinate,
                body: Vec<SpannedStatement>
            },
            DirectiveBlock {
                directives: Vec<BlockDirective>,
                body: Vec<SpannedStatement>
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
            Match {
                target: Expression,
                arms: Vec<MatchArm>
            },
            IfLet {
                pattern: Pattern,
                expr: Expression,
                then_branch: Vec<SpannedStatement>,
                else_branch: Option<Vec<SpannedStatement>>,
                reconcile: Option<MergeResolution>
            },
            Await(String),
            If {
                binding: Option<String>,
                condition: Expression,
                then_branch: Vec<SpannedStatement>,
                else_branch: Option<Vec<SpannedStatement>>,
                reconcile: Option<MergeResolution>
            },
            Break,
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
            While {
                condition: Expression,
                is_valid_check: bool,
                max_ms: u64,
                body: Vec<SpannedStatement>
            },
            ForStep {
                item_name: String,
                source: Expression,
                step_ms: Option<u64>,
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
            Yield(Option<Expression>),
            Print(Vec<Expression>),
            Debug(Expression),
            RoutineDef {
                name: String,
                params: Vec<ParamDecl>,
                return_type: Option<TypeName>,
                taking_ms: Option<u64>,
                state_constraint: Option<(String, String)>,
                required_capabilities: Vec<Capability>,
                body: Vec<SpannedStatement>
            },
            Return(Option<Expression>),
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
            },
            StateDecl {
                target: String,
                var_type: Option<TypeName>,
                expr: Expression
            },
            PolicyStmt {
                target: PolicyTarget,
                policy: SaturationPolicy
            },
            LoopOn {
                target: Expression,
                body: Vec<SpannedStatement>
            },
            MacroDef {
                name: String,
                params: Vec<MacroParam>,
                body_template: String
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
        let base: u64 = 1;
        let extra = match self {
            Statement::Split { .. }
            | Statement::Merge { .. }
            | Statement::Anchor(_)
            | Statement::Rewind(_)
            | Statement::Commit(_)
            | Statement::Send { .. }
            | Statement::Capability(_)
            | Statement::Assignment { .. }
            | Statement::DestructureAssignment { .. }
            | Statement::TypeDecl { .. }
            | Statement::EnumDecl { .. }
            | Statement::InterfaceDecl { .. }
            | Statement::FieldUpdate { .. }
            | Statement::StateDecl { .. }
            | Statement::PolicyStmt { .. }
            | Statement::Expression(_)
            | Statement::Print(_) => 0,
            Statement::Using { body, .. } => estimate_block(body),
            Statement::DecayHandler { body, .. } => estimate_block(body),
            Statement::AssertTime { fallback, .. } => {
                fallback.as_ref().map(|b| estimate_block(b)).unwrap_or(0)
            }
            Statement::RelativisticBlock { body, .. }
            | Statement::DirectiveBlock { body, .. } => estimate_block(body),
            Statement::Isolate(block) => estimate_block(&block.body),
            Statement::Debug(_) => 1,
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => estimate_block(then_branch)
                .max(else_branch.as_ref().map(|b| estimate_block(b)).unwrap_or(0)),
            Statement::For { pacing_ms, .. } => pacing_ms.unwrap_or(1),
            Statement::ForStep { step_ms, .. } => step_ms.unwrap_or(1),
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
                if *max_ms == u64::MAX {
                    case_max_cost.max(timeout_cost)
                } else {
                    *max_ms
                }
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
                    .map(|b| estimate_block(&b.2))
                    .unwrap_or(0);
                let decayed_cost = decayed_branch
                    .as_ref()
                    .map(|b| estimate_block(&b.2))
                    .unwrap_or(0);
                let pending_cost = pending_branch
                    .as_ref()
                    .map(|b| estimate_block(&b.2))
                    .unwrap_or(0);
                let consumed_cost = consumed_branch
                    .as_ref()
                    .map(|b| estimate_block(&b.1))
                    .unwrap_or(0);
                valid_cost
                    .max(decayed_cost)
                    .max(pending_cost)
                    .max(consumed_cost)
            }
            Statement::Match { arms, .. } => arms
                .iter()
                .map(|a| estimate_block(&a.body))
                .max()
                .unwrap_or(0),
            Statement::IfLet {
                then_branch,
                else_branch,
                ..
            } => estimate_block(then_branch)
                .max(else_branch.as_ref().map(|b| estimate_block(b)).unwrap_or(0)),
            Statement::Collapse => 0,
            Statement::SplitMap { body, .. } => 1 + estimate_block(body),
            Statement::Lease { duration_ms, .. } => *duration_ms,
            Statement::RoutineDef { body, .. } => estimate_block(body),
            Statement::Loop { max_ms, body } => {
                if *max_ms == u64::MAX {
                    estimate_block(body)
                } else {
                    *max_ms
                }
            }
            Statement::LoopTick { .. } => 1,
            Statement::LoopOn { body, .. } => estimate_block(body),
            Statement::While { max_ms, body, .. } => {
                if *max_ms == u64::MAX {
                    estimate_block(body)
                } else {
                    *max_ms
                }
            }
            Statement::Slice { .. } => 0,
            Statement::SpeculationMode(_) => 0,
            Statement::Await(_) => 1,
            Statement::Yield(_)
            | Statement::Break
            | Statement::Entangle { .. }
            | Statement::Import { .. }
            | Statement::FromImport { .. }
            | Statement::ForeignBlock { .. }
            | Statement::MacroDef { .. }
            | Statement::Return(_) => 0,
        };
        base.saturating_add(extra)
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
            StrBytes(Box<Expression>),
            ToStr(Box<Expression>),
            Len(Box<Expression>),
            RefOp(Box<Expression>),
            StructLit(std::cell::RefCell<Option<String>>, std::collections::HashMap<String, Expression>),
            TopologyLit(std::collections::HashMap<String, Expression>),
            IndexAccess {
                target: Box<Expression>,
                index: Box<Expression>
            },
            ArraySlice {
                target: Box<Expression>,
                start: Option<Box<Expression>>,
                end: Option<Box<Expression>>,
                inclusive: bool
            },
            ArrayLiteral(Vec<Expression>),
            ArrayRepeat {
                value: Box<Expression>,
                count: Box<Expression>
            },
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
            EnumVariant {
                enum_name: String,
                variant_name: String,
                args: Vec<Expression>
            },
            TryUnwrap(Box<Expression>),
            Turbofish {
                expr: Box<Expression>,
                type_args: Vec<TypeParam>
            },
            GenericStaticCall {
                type_name: String,
                type_args: Vec<TypeParam>,
                method: String,
                args: Vec<Expression>
            },
            FString(Vec<FStringPart>),
            If {
                condition: Box<Expression>,
                then_branch: Box<Expression>,
                else_branch: Box<Expression>
            },
            Match {
                target: Box<Expression>,
                arms: Vec<MatchExprArm>
            },
            ArenaIntrospect(ArenaIntrospect),
            CapabilityCheck(Capability),
            Tuple(Vec<Expression>),
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

impl Expression {
    pub fn for_each_child_expr<'a>(&'a self, f: &mut impl FnMut(&'a Expression)) {
        match self {
            Expression::FieldAccess { target, .. } => f(target),
            Expression::MethodCall { target, args, .. } => {
                f(target);
                for arg in args {
                    f(arg);
                }
            }
            Expression::Call { args, .. } => {
                for arg in args {
                    f(arg);
                }
            }
            Expression::BinaryOp { left, right, .. } => {
                f(left);
                f(right);
            }
            Expression::UnaryOp { expr, .. } => f(expr),
            Expression::IndexAccess { target, index } => {
                f(target);
                f(index);
            }
            Expression::ArraySlice {
                target, start, end, ..
            } => {
                f(target);
                if let Some(s) = start {
                    f(s);
                }
                if let Some(e) = end {
                    f(e);
                }
            }
            Expression::ArrayLiteral(elements) => {
                for el in elements {
                    f(el);
                }
            }
            Expression::ArrayRepeat { value, count } => {
                f(value);
                f(count);
            }
            Expression::StructLit(_, fields) | Expression::TopologyLit(fields) => {
                for expr in fields.values() {
                    f(expr);
                }
            }
            Expression::Syscall { args, .. } => {
                for arg in args {
                    f(arg);
                }
            }
            Expression::EnumVariant { args, .. } => {
                for arg in args {
                    f(arg);
                }
            }
            Expression::TryUnwrap(expr) => f(expr),
            Expression::Turbofish { expr, .. } => f(expr),
            Expression::GenericStaticCall { args, .. } => {
                for arg in args {
                    f(arg);
                }
            }
            Expression::FString(parts) => {
                for part in parts {
                    if let FStringPart::Expr(expr) = part {
                        f(expr);
                    }
                }
            }
            Expression::If {
                condition,
                then_branch,
                else_branch,
            } => {
                f(condition);
                f(then_branch);
                f(else_branch);
            }
            Expression::Match { target, arms } => {
                f(target);
                for arm in arms {
                    if let Some(ref guard) = arm.guard {
                        f(guard);
                    }
                    f(&arm.body);
                }
            }
            Expression::TypeAssertion { target, .. } => f(target),
            Expression::TypeCast { expr, .. } => f(expr),
            Expression::Tuple(elems) => {
                for el in elems {
                    f(el);
                }
            }
            _ => {}
        }
    }
}

impl Statement {
    pub fn for_each_child_expr<'a>(&'a self, f: &mut impl FnMut(&'a Expression)) {
        match self {
            Statement::Assignment { expr, .. } => f(expr),
            Statement::DestructureAssignment { expr, .. } => f(expr),
            Statement::Using { resource, .. } => f(resource),
            Statement::Expression(expr) => f(expr),
            Statement::Debug(expr) => f(expr),
            Statement::Print(args) => {
                for arg in args {
                    f(arg);
                }
            }
            Statement::FieldUpdate { value, .. } => f(value),
            Statement::If { condition, .. } => f(condition),
            Statement::IfLet { expr, .. } => f(expr),
            Statement::While { condition, .. } => f(condition),
            Statement::ForStep { source, .. } => f(source),
            Statement::Yield(expr_opt) => {
                if let Some(e) = expr_opt {
                    f(e);
                }
            }
            Statement::MatchEntropy {
                target,
                valid_branch,
                decayed_branch,
                pending_branch,
                consumed_branch,
            } => {
                f(target);
                if let Some((_, Some(ref g), _)) = valid_branch {
                    f(g);
                }
                if let Some((_, Some(ref g), _)) = decayed_branch {
                    f(g);
                }
                if let Some((_, Some(ref g), _)) = pending_branch {
                    f(g);
                }
                if let Some((Some(ref g), _)) = consumed_branch {
                    f(g);
                }
            }
            Statement::Match { target, arms } => {
                f(target);
                for arm in arms {
                    if let Some(ref g) = arm.guard {
                        f(g);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn for_each_child_block<'a>(
        &'a self,
        f: &mut impl FnMut(&'a [SpannedStatement]),
    ) {
        match self {
            Statement::Using { body, .. }
            | Statement::DecayHandler { body, .. }
            | Statement::RelativisticBlock { body, .. }
            | Statement::DirectiveBlock { body, .. } => f(body),
            Statement::Isolate(block) => f(&block.body),
            Statement::Commit(stmts) => f(stmts),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                f(then_branch);
                if let Some(else_b) = else_branch {
                    f(else_b);
                }
            }
            Statement::While { body, .. }
            | Statement::For { body, .. }
            | Statement::ForStep { body, .. }
            | Statement::SplitMap { body, .. } => f(body),
            Statement::Speculate { body, fallback, .. } => {
                f(body);
                if let Some(fb) = fallback {
                    f(fb);
                }
            }
            Statement::Select { cases, timeout, .. } => {
                for c in cases {
                    f(&c.body);
                }
                if let Some(to) = timeout {
                    f(to);
                }
            }
            Statement::MatchEntropy {
                valid_branch,
                decayed_branch,
                pending_branch,
                consumed_branch,
                ..
            } => {
                if let Some((_, _, ref b)) = valid_branch {
                    f(b);
                }
                if let Some((_, _, ref b)) = decayed_branch {
                    f(b);
                }
                if let Some((_, _, ref b)) = pending_branch {
                    f(b);
                }
                if let Some((_, ref b)) = consumed_branch {
                    f(b);
                }
            }
            Statement::Match { arms, .. } => {
                for arm in arms {
                    f(&arm.body);
                }
            }
            Statement::RoutineDef { body, .. } => f(body),
            _ => {}
        }
    }
}

/// A segment within an f-string literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FStringPart {
    /// A literal text chunk between `{…}` interpolations.
    Text(String),
    /// An embedded expression `{expr}`.
    Expr(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IsolateBlock {
    pub name: Option<String>,
    pub manifest: Manifest,
    #[serde(skip)]
    pub body: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub cpu_budget_ms: Option<u64>,
    pub slice_ms: Option<u64>,
    pub memory_budget_bytes: Option<u64>,
    pub resource_budgets: std::collections::HashMap<String, u64>,
    pub capabilities: Vec<Capability>,
    pub mode: Option<EntropyMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub path: String,
    pub parameters: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeResolution {
    pub rules: std::collections::HashMap<String, ResolutionStrategy>,
    pub auto: bool,
    #[serde(skip)]
    pub fallback: Option<Vec<SpannedStatement>>,
    pub taking_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalReversion {
    pub branch: String,
    pub anchor: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifetimeAnnotation {
    Valid,
    Decayed(u64),
    DecayRate(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariantDef {
    pub name: String,
    pub payload_types: Vec<TypeName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pattern {
    Wildcard,
    Identifier(String),
    Literal(Expression),
    Tuple(Vec<Pattern>),
    EnumVariant {
        enum_name: Option<String>,
        variant_name: String,
        args: Vec<Pattern>,
    },
    TypeAssert {
        binding: String,
        target_type: TypeName,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub body: Vec<SpannedStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchExprArm {
    pub pattern: Pattern,
    pub guard: Option<Expression>,
    pub body: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeFieldDef {
    pub typ: TypeName,
    pub is_const: bool,
    pub default_value: Option<Expression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockDirective {
    NoZ3,
    Chaos,
    Deterministic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeName {
    Builtin(BuiltinType),
    Custom(String),
    Generic(String, Vec<TypeParam>),
    Optional(Box<TypeName>),
    Union(Vec<TypeName>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeParam {
    Type(TypeName),
    Amount(u64),
    Duration(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParamMode {
    Consume,
    Clone,
    Decay,
    Peek,
    Lease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Neg,
    Not,
    BitwiseNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    LogicalAnd,
    LogicalOr,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Shl,
    Shr,
    NullCoalesce,
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

pub fn ast_statements_eq(a: &[SpannedStatement], b: &[SpannedStatement]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (s1, s2) in a.iter().zip(b.iter()) {
        if !ast_statement_eq(&s1.stmt, &s2.stmt) {
            return false;
        }
    }
    true
}

pub fn ast_statement_eq(a: &Statement, b: &Statement) -> bool {
    match (a, b) {
        (
            Statement::Assignment {
                target: t1,
                mutable: m1,
                var_type: vt1,
                lifetime: lt1,
                expr: e1,
            },
            Statement::Assignment {
                target: t2,
                mutable: m2,
                var_type: vt2,
                lifetime: lt2,
                expr: e2,
            },
        ) => t1 == t2 && m1 == m2 && vt1 == vt2 && lt1 == lt2 && e1 == e2,
        (
            Statement::DestructureAssignment {
                fields: f1,
                mutable: m1,
                expr: e1,
            },
            Statement::DestructureAssignment {
                fields: f2,
                mutable: m2,
                expr: e2,
            },
        ) => f1 == f2 && m1 == m2 && e1 == e2,
        (
            Statement::Using {
                binding: b1,
                resource: r1,
                body: bd1,
            },
            Statement::Using {
                binding: b2,
                resource: r2,
                body: bd2,
            },
        ) => b1 == b2 && r1 == r2 && ast_statements_eq(bd1, bd2),
        (
            Statement::RoutineDef {
                name: n1,
                params: p1,
                return_type: rt1,
                taking_ms: t1,
                state_constraint: sc1,
                required_capabilities: rc1,
                body: bd1,
            },
            Statement::RoutineDef {
                name: n2,
                params: p2,
                return_type: rt2,
                taking_ms: t2,
                state_constraint: sc2,
                required_capabilities: rc2,
                body: bd2,
            },
        ) => {
            n1 == n2
                && p1 == p2
                && rt1 == rt2
                && t1 == t2
                && sc1 == sc2
                && rc1 == rc2
                && ast_statements_eq(bd1, bd2)
        }
        (
            Statement::TypeDecl {
                name: n1,
                extends: ex1,
                fields: f1,
                decay_after_ms: d1,
                auto_drop: ad1,
                scoped_branch: sb1,
            },
            Statement::TypeDecl {
                name: n2,
                extends: ex2,
                fields: f2,
                decay_after_ms: d2,
                auto_drop: ad2,
                scoped_branch: sb2,
            },
        ) => {
            n1 == n2
                && ex1 == ex2
                && f1 == f2
                && d1 == d2
                && ad1 == ad2
                && sb1 == sb2
        }
        (
            Statement::EnumDecl {
                name: n1,
                variants: v1,
            },
            Statement::EnumDecl {
                name: n2,
                variants: v2,
            },
        ) => n1 == n2 && v1 == v2,
        (
            Statement::InterfaceDecl {
                name: n1,
                extends: ex1,
                methods: m1,
            },
            Statement::InterfaceDecl {
                name: n2,
                extends: ex2,
                methods: m2,
            },
        ) => {
            if n1 != n2 || ex1 != ex2 || m1.len() != m2.len() {
                return false;
            }
            for (meth1, meth2) in m1.iter().zip(m2.iter()) {
                if meth1.name != meth2.name
                    || meth1.params != meth2.params
                    || meth1.return_type != meth2.return_type
                    || meth1.taking_ms != meth2.taking_ms
                    || meth1.state_constraint != meth2.state_constraint
                    || meth1.required_capabilities != meth2.required_capabilities
                {
                    return false;
                }
                match (&meth1.default_body, &meth2.default_body) {
                    (Some(b1), Some(b2)) => {
                        if !ast_statements_eq(b1, b2) {
                            return false;
                        }
                    }
                    (None, None) => {}
                    _ => return false,
                }
            }
            true
        }
        (
            Statement::Split {
                parent: p1,
                branches: b1,
            },
            Statement::Split {
                parent: p2,
                branches: b2,
            },
        ) => p1 == p2 && b1 == b2,
        (
            Statement::Merge {
                branches: b1,
                target: t1,
                resolutions: r1,
            },
            Statement::Merge {
                branches: b2,
                target: t2,
                resolutions: r2,
            },
        ) => b1 == b2 && t1 == t2 && r1 == r2,
        (Statement::Anchor(n1), Statement::Anchor(n2)) => n1 == n2,
        (Statement::Rewind(n1), Statement::Rewind(n2)) => n1 == n2,
        (Statement::Commit(b1), Statement::Commit(b2)) => ast_statements_eq(b1, b2),
        (Statement::Isolate(iso1), Statement::Isolate(iso2)) => {
            iso1.name == iso2.name && ast_statements_eq(&iso1.body, &iso2.body)
        }
        (
            Statement::DecayHandler {
                type_name: t1,
                body: b1,
            },
            Statement::DecayHandler {
                type_name: t2,
                body: b2,
            },
        ) => t1 == t2 && ast_statements_eq(b1, b2),
        (
            Statement::AssertTime {
                operator: op1,
                limit_ms: l1,
                fallback: fb1,
            },
            Statement::AssertTime {
                operator: op2,
                limit_ms: l2,
                fallback: fb2,
            },
        ) => {
            op1 == op2
                && l1 == l2
                && match (fb1, fb2) {
                    (Some(x), Some(y)) => ast_statements_eq(x, y),
                    (None, None) => true,
                    _ => false,
                }
        }
        (
            Statement::Send {
                value_id: v1,
                target_branch: t1,
            },
            Statement::Send {
                value_id: v2,
                target_branch: t2,
            },
        ) => v1 == v2 && t1 == t2,
        (Statement::Expression(e1), Statement::Expression(e2)) => e1 == e2,
        (Statement::Capability(c1), Statement::Capability(c2)) => c1 == c2,
        (
            Statement::RelativisticBlock { time: t1, body: b1 },
            Statement::RelativisticBlock { time: t2, body: b2 },
        ) => t1 == t2 && ast_statements_eq(b1, b2),
        (
            Statement::DirectiveBlock {
                directives: d1,
                body: b1,
            },
            Statement::DirectiveBlock {
                directives: d2,
                body: b2,
            },
        ) => d1 == d2 && ast_statements_eq(b1, b2),
        (
            Statement::Speculate {
                max_ms: m1,
                body: b1,
                fallback: fb1,
            },
            Statement::Speculate {
                max_ms: m2,
                body: b2,
                fallback: fb2,
            },
        ) => {
            m1 == m2
                && ast_statements_eq(b1, b2)
                && match (fb1, fb2) {
                    (Some(x), Some(y)) => ast_statements_eq(x, y),
                    (None, None) => true,
                    _ => false,
                }
        }
        (Statement::Collapse, Statement::Collapse) => true,
        (Statement::SpeculationMode(m1), Statement::SpeculationMode(m2)) => m1 == m2,
        (
            Statement::Select {
                max_ms: m1,
                cases: c1,
                timeout: to1,
                reconcile: r1,
            },
            Statement::Select {
                max_ms: m2,
                cases: c2,
                timeout: to2,
                reconcile: r2,
            },
        ) => {
            if m1 != m2 || r1 != r2 || c1.len() != c2.len() {
                return false;
            }
            for (case1, case2) in c1.iter().zip(c2.iter()) {
                if case1.binding != case2.binding
                    || case1.source != case2.source
                    || !ast_statements_eq(&case1.body, &case2.body)
                {
                    return false;
                }
            }
            match (to1, to2) {
                (Some(x), Some(y)) => ast_statements_eq(x, y),
                (None, None) => true,
                _ => false,
            }
        }
        (
            Statement::MatchEntropy {
                target: t1,
                valid_branch: vb1,
                decayed_branch: db1,
                pending_branch: pb1,
                consumed_branch: cb1,
            },
            Statement::MatchEntropy {
                target: t2,
                valid_branch: vb2,
                decayed_branch: db2,
                pending_branch: pb2,
                consumed_branch: cb2,
            },
        ) => {
            if t1 != t2 {
                return false;
            }
            let check_branch = |b1: &Option<(
                DecayedPattern,
                Option<Expression>,
                Vec<SpannedStatement>,
            )>,
                                b2: &Option<(
                DecayedPattern,
                Option<Expression>,
                Vec<SpannedStatement>,
            )>| {
                match (b1, b2) {
                    (Some((p1, g1, bd1)), Some((p2, g2, bd2))) => {
                        p1 == p2 && g1 == g2 && ast_statements_eq(bd1, bd2)
                    }
                    (None, None) => true,
                    _ => false,
                }
            };
            if !check_branch(vb1, vb2)
                || !check_branch(db1, db2)
                || !check_branch(pb1, pb2)
            {
                return false;
            }
            match (cb1, cb2) {
                (Some((g1, bd1)), Some((g2, bd2))) => {
                    g1 == g2 && ast_statements_eq(bd1, bd2)
                }
                (None, None) => true,
                _ => false,
            }
        }
        (
            Statement::Match {
                target: t1,
                arms: a1,
            },
            Statement::Match {
                target: t2,
                arms: a2,
            },
        ) => {
            if t1 != t2 || a1.len() != a2.len() {
                return false;
            }
            for (arm1, arm2) in a1.iter().zip(a2.iter()) {
                if arm1.pattern != arm2.pattern
                    || arm1.guard != arm2.guard
                    || !ast_statements_eq(&arm1.body, &arm2.body)
                {
                    return false;
                }
            }
            true
        }
        (
            Statement::IfLet {
                pattern: p1,
                expr: e1,
                then_branch: tb1,
                else_branch: eb1,
                reconcile: r1,
            },
            Statement::IfLet {
                pattern: p2,
                expr: e2,
                then_branch: tb2,
                else_branch: eb2,
                reconcile: r2,
            },
        ) => {
            p1 == p2
                && e1 == e2
                && r1 == r2
                && ast_statements_eq(tb1, tb2)
                && match (eb1, eb2) {
                    (Some(x), Some(y)) => ast_statements_eq(x, y),
                    (None, None) => true,
                    _ => false,
                }
        }
        (Statement::Await(a1), Statement::Await(a2)) => a1 == a2,
        (
            Statement::If {
                binding: b1,
                condition: c1,
                then_branch: tb1,
                else_branch: eb1,
                reconcile: r1,
            },
            Statement::If {
                binding: b2,
                condition: c2,
                then_branch: tb2,
                else_branch: eb2,
                reconcile: r2,
            },
        ) => {
            b1 == b2
                && c1 == c2
                && r1 == r2
                && ast_statements_eq(tb1, tb2)
                && match (eb1, eb2) {
                    (Some(x), Some(y)) => ast_statements_eq(x, y),
                    (None, None) => true,
                    _ => false,
                }
        }
        (Statement::Break, Statement::Break) => true,
        (
            Statement::Lease {
                binding: b1,
                source: s1,
                duration_ms: d1,
                body: bd1,
                reconcile: r1,
            },
            Statement::Lease {
                binding: b2,
                source: s2,
                duration_ms: d2,
                body: bd2,
                reconcile: r2,
            },
        ) => {
            b1 == b2
                && s1 == s2
                && d1 == d2
                && r1 == r2
                && ast_statements_eq(bd1, bd2)
        }
        (
            Statement::Loop {
                max_ms: m1,
                body: b1,
            },
            Statement::Loop {
                max_ms: m2,
                body: b2,
            },
        ) => m1 == m2 && ast_statements_eq(b1, b2),
        (Statement::LoopTick { body: b1 }, Statement::LoopTick { body: b2 }) => {
            ast_statements_eq(b1, b2)
        }
        (
            Statement::While {
                condition: c1,
                is_valid_check: v1,
                max_ms: m1,
                body: b1,
            },
            Statement::While {
                condition: c2,
                is_valid_check: v2,
                max_ms: m2,
                body: b2,
            },
        ) => c1 == c2 && v1 == v2 && m1 == m2 && ast_statements_eq(b1, b2),
        (
            Statement::ForStep {
                item_name: n1,
                source: s1,
                step_ms: st1,
                body: b1,
            },
            Statement::ForStep {
                item_name: n2,
                source: s2,
                step_ms: st2,
                body: b2,
            },
        ) => n1 == n2 && s1 == s2 && st1 == st2 && ast_statements_eq(b1, b2),
        (
            Statement::Slice { milliseconds: m1 },
            Statement::Slice { milliseconds: m2 },
        ) => m1 == m2,
        (
            Statement::For {
                item_name: n1,
                mode: m1,
                source: s1,
                body: b1,
                pacing_ms: p1,
                max_ms: mx1,
            },
            Statement::For {
                item_name: n2,
                mode: m2,
                source: s2,
                body: b2,
                pacing_ms: p2,
                max_ms: mx2,
            },
        ) => {
            n1 == n2
                && m1 == m2
                && s1 == s2
                && p1 == p2
                && mx1 == mx2
                && ast_statements_eq(b1, b2)
        }
        (
            Statement::SplitMap {
                item_name: n1,
                mode: m1,
                source: s1,
                body: b1,
                reconcile: r1,
            },
            Statement::SplitMap {
                item_name: n2,
                mode: m2,
                source: s2,
                body: b2,
                reconcile: r2,
            },
        ) => {
            n1 == n2 && m1 == m2 && s1 == s2 && r1 == r2 && ast_statements_eq(b1, b2)
        }
        (Statement::Yield(y1), Statement::Yield(y2)) => y1 == y2,
        (Statement::Print(p1), Statement::Print(p2)) => p1 == p2,
        (Statement::Debug(d1), Statement::Debug(d2)) => d1 == d2,
        (Statement::Return(r1), Statement::Return(r2)) => r1 == r2,
        (
            Statement::Entangle { variables: v1 },
            Statement::Entangle { variables: v2 },
        ) => v1 == v2,
        (
            Statement::Import {
                path: p1,
                alias: a1,
            },
            Statement::Import {
                path: p2,
                alias: a2,
            },
        ) => p1 == p2 && a1 == a2,
        (
            Statement::FromImport {
                path: p1,
                symbols: s1,
            },
            Statement::FromImport {
                path: p2,
                symbols: s2,
            },
        ) => p1 == p2 && s1 == s2,
        (
            Statement::ForeignBlock {
                lib_name: l1,
                abi: a1,
                routines: r1,
            },
            Statement::ForeignBlock {
                lib_name: l2,
                abi: a2,
                routines: r2,
            },
        ) => l1 == l2 && a1 == a2 && ast_statements_eq(r1, r2),
        (
            Statement::FieldUpdate {
                target: t1,
                field: f1,
                value: v1,
            },
            Statement::FieldUpdate {
                target: t2,
                field: f2,
                value: v2,
            },
        ) => t1 == t2 && f1 == f2 && v1 == v2,
        (
            Statement::StateDecl {
                target: t1,
                var_type: ty1,
                expr: e1,
            },
            Statement::StateDecl {
                target: t2,
                var_type: ty2,
                expr: e2,
            },
        ) => t1 == t2 && ty1 == ty2 && e1 == e2,
        (
            Statement::PolicyStmt {
                target: t1,
                policy: p1,
            },
            Statement::PolicyStmt {
                target: t2,
                policy: p2,
            },
        ) => t1 == t2 && p1 == p2,
        (
            Statement::LoopOn {
                target: t1,
                body: b1,
            },
            Statement::LoopOn {
                target: t2,
                body: b2,
            },
        ) => t1 == t2 && ast_statements_eq(b1, b2),
        _ => false,
    }
}

pub fn programs_ast_eq(a: &Program, b: &Program) -> bool {
    if a.timelines.len() != b.timelines.len() {
        return false;
    }
    for (t1, t2) in a.timelines.iter().zip(b.timelines.iter()) {
        if t1.time != t2.time
            || t1.no_z3 != t2.no_z3
            || t1.entropy_mode != t2.entropy_mode
            || !ast_statements_eq(&t1.statements, &t2.statements)
        {
            return false;
        }
    }
    true
}
