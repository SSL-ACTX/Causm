/// Relational fact extracted from an SSA instruction or CFG point.
///
/// These facts are the base inputs to the Entropic-Polonius invariant solver
/// (Phase 3). In Phase 2 the variants are defined but extraction is not yet
/// wired; `extract_facts` returns an empty set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntropicFact {
    /// Variable `v` was allocated or bound at CFG point `point`.
    ValueIntroduced { var: String, point: usize },
    /// Variable `v` was explicitly moved, consumed, or auto-dropped at `point`.
    LinearConsume { var: String, point: usize },
    /// Field `field` of struct variable `var` was destructured/consumed at `point`.
    FieldConsume { var: String, field: String, point: usize },
    /// Lease on variable `var` is active over virtual clock interval `[t_start, t_end]`.
    LeaseIssued { var: String, lease_id: String, t_start: u64, t_end: u64 },
    /// Variable `var` has a TTL expiring at `t_expire`.
    TemporalDecay { var: String, t_expire: u64 },
    /// Variable `var` was read, peeked, or referenced at `point` at clock `t`.
    AccessAt { var: String, point: usize, t_current: u64 },
}

/// Extract relational facts from the SSA instructions of a compiled program.
///
/// Phase 3 will populate this. For Phase 2 it is a stub returning an empty vec.
pub fn extract_facts(_program: &causm_core::Program) -> Vec<EntropicFact> {
    Vec::new()
}
