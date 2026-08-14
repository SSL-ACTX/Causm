use causm_core::types::Type;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum SemanticErrorKind {
    #[error("Compile-Time Entropic Violation: '{0}' has been consumed or decayed and cannot be moved/reused.")]
    UseAfterConsume(String),
    #[error("Entropy Violation: Variable '{0}' has decayed after {1}ms (instantiated at {2}ms, currently at {3}ms)")]
    UsedDecayedValue(String, u64, u64, u64),
    #[error("Timeline Violation: Variable '{0}' is scoped to branch '@{1}' and cannot be moved to branch '@{2}'.")]
    InvalidTimelineMove(String, String, String),
    #[error("Merge Collision: Variable '{0}' produced in multiple branches requires a resolution strategy.")]
    UnresolvedMerge(String),
    #[error("Branch Leak: Variable '{0}' is consumed in one branch but accessed in a parallel timeline.")]
    CrossBranchViolation(String),
    #[error("Entropy Mismatch: variables require reconcile: {0}")]
    EntropyMismatch(String),
    #[error("Invalid 'loop' budget: max must be >0")]
    InvalidLoopBudget,
    #[error("Tick loop requires a fixed slice via slice <N>ms")]
    TickLoopWithoutSlice,
    #[error("Type mismatch: {0}")]
    TypeMismatch(String),
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("Tick loop body cost {0}ms exceeds slice budget {1}ms")]
    TickLoopBudgetExceeded(u64, u64),
    #[error("Tick loop must include a break statement")]
    TickLoopNeedsBreak,
    #[error("Routine temporal contract violated: {0} requires {1}ms but body costs {2}ms")]
    RoutineBudgetExceeded(String, u64, u64),
    #[error("Pacing violation: loop body exceeds pacing window")]
    PacingViolation,
    #[error("Invalid Access: '{0}' is not a structure or has decayed.")]
    InvalidStructuralAccess(String),
    #[error("Capability violation: Required capability '{0}' is not declared in this isolate.")]
    MissingCapability(String),
    #[error("Forbidden library path: '{0}' is not in the allowed paths whitelist.")]
    ForbiddenLibraryPath(String),
    #[error("Temporal Assertion Violation: WCET to this point is {0}ms, which exceeds the limit of {1}ms")]
    TemporalAssertionViolation(u64, u64),
    #[error("Chaos Mode enabled: Rewinds and anchors are disabled because non-deterministic entropy was requested.")]
    ChaosModePreventsRewind,
    #[error(
        "Lease Violation: Attempted to mutate or transmit leased variable '{0}'"
    )]
    LeaseViolation(String),
    #[error("Lease Duration Exceeded: WCET of lease block ({0}ms) exceeds requested duration ({1}ms)")]
    LeaseDurationExceeded(u64, u64),
    #[error(
        "Nested Leasing: Cannot lease a variable '{0}' that is already leased."
    )]
    NestedLeasing(String),
    #[error("Illegal Control Flow: Lease blocks cannot contain 'break' or 'return' statements.")]
    IllegalLeaseControlFlow,
    #[error("Compile-Time Entropic Leak: Variable '{0}' remains Valid or Decayed at program termination without being consumed.")]
    UnconsumedVariable(String),
    #[error("Argument count mismatch: {0}")]
    ArgumentCountMismatch(String),
    #[error("Timeline Violation: Branch '@{0}' is inactive, has been merged, or has not been split yet.")]
    InactiveTimeline(String),
    #[error("Temporal Contract Violated: routine '{0}' inferred cost {1}ms exceeds interface budget {2}ms")]
    TemporalContractViolated(String, u64, u64),
}

#[derive(Debug)]
pub struct SemanticError {
    pub kind: Box<SemanticErrorKind>,
    pub branch: String,
    pub statement: Option<String>,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let location_prefix = match (&self.file, self.line, self.column) {
            (Some(file), Some(line), Some(col)) => {
                format!("{}:{}:{}", file, line, col)
            }
            _ => "<unknown>".to_string(),
        };

        write!(f, "error: {}\n  --> {}\n   |\n", self.kind, location_prefix)?;

        if let Some(ref stmt) = self.statement {
            writeln!(f, "{:>4} | {}", self.line.unwrap_or(0), stmt)?;
            if let Some(col) = self.column {
                let marker_line = " ".repeat(col.saturating_sub(1));
                writeln!(f, "   | {}^", marker_line)?;
            }
        }

        write!(f, "   |\n   = note: branch '{}'\n", self.branch)
    }
}

impl std::error::Error for SemanticError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[derive(Clone, Default)]
pub struct BranchState {
    pub consumed: HashSet<String>,
    pub decayed: HashSet<String>,
    pub yields: HashSet<String>,
    pub produced: HashSet<String>,
    pub leased: HashSet<String>,
    pub lease_bindings: HashSet<String>,
    pub mutables: HashSet<String>,
    pub types: HashMap<String, Type>,
    pub custom_types: HashMap<String, Type>,
    pub accumulated_cost: u64,
    pub instantiated_at: HashMap<String, u64>,
}

impl BranchState {
    pub fn remove_variable_scope(&mut self, name: &str) {
        self.types.remove(name);
        self.consumed.remove(name);
        self.decayed.remove(name);
        self.leased.remove(name);
        self.lease_bindings.remove(name);
        self.yields.remove(name);
        self.produced.remove(name);
        self.mutables.remove(name);
        self.instantiated_at.remove(name);

        let prefix = format!("{}.", name);
        self.types.retain(|k, _| !k.starts_with(&prefix));
        self.consumed.retain(|k| !k.starts_with(&prefix));
        self.decayed.retain(|k| !k.starts_with(&prefix));
        self.leased.retain(|k| !k.starts_with(&prefix));
        self.lease_bindings.retain(|k| !k.starts_with(&prefix));
        self.yields.retain(|k| !k.starts_with(&prefix));
        self.produced.retain(|k| !k.starts_with(&prefix));
        self.mutables.retain(|k| !k.starts_with(&prefix));
        self.instantiated_at.retain(|k, _| !k.starts_with(&prefix));
    }
}

#[derive(Clone, Debug)]
pub struct RoutineInfo {
    pub params: Vec<(causm_core::ParamMode, String, Type)>,
    pub return_type: Type,
    pub taking_ms: u64,
    pub state_constraint: Option<(String, String)>,
}
