pub mod backend;
pub mod diagnostics;
pub mod facts;
pub mod relational;
pub mod wcet;

pub use backend::SolverBackend;
pub use diagnostics::EntropicDiagnostic;
pub use facts::{
    extract_facts, extract_ssa_facts, EntropicFact, PointIndex, ProgramFacts,
    SsaPointIndex,
};
pub use relational::RelationalInvariantSolver;
pub use wcet::WcetSolver;

use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::Program;

/// Stage 2a/2c of the analysis pipeline: the Entropius Solver.
///
/// Split into two halves that bracket SsaStage:
///
/// `run_relational` — Pure relational fact extraction and Invariant 1/2/3 proofs.
///   Runs BEFORE SsaStage so rich multi-span diagnostics fire before any
///   legacy procedural error can mask them. No dependency on SsaStage output.
///
/// `run_post_ssa`   — EGC unconsumed-variable check and full symbolic WcetSolver
///   (WCET, temporal contracts, isolate budgets). Runs AFTER SsaStage because it needs
///   `branch_contexts.produced` and `routines` that SsaStage populates.
pub struct SolverStage;

impl SolverStage {
    /// Stage 2a: Relational pre-pass. Extract ProgramFacts and verify Invariants 1–3.
    /// Must run before SsaStage.
    pub fn run_relational(
        analyzer: &mut EntropicAnalyzer,
        program: &Program,
    ) -> Result<(), SemanticError> {
        let source = analyzer.source.clone().unwrap_or_default();
        let filename = analyzer
            .filename
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        let facts = extract_facts(program, &source, &filename);

        let mut relational_solver =
            RelationalInvariantSolver::<crate::oxiz::OxiZBackend>::new(analyzer);
        relational_solver.solve_invariants(&facts)?;

        Ok(())
    }

    /// Stage 2c: Post-SSA checks. EGC unconsumed-variable enforcement +
    /// full symbolic SMT FormalVerifier (WCET path conditions, entanglement invariants,
    /// causal horizons, lease constraint proofs). Must run after SsaStage.
    pub fn run_post_ssa(
        analyzer: &mut EntropicAnalyzer,
        program: &Program,
    ) -> Result<(), SemanticError> {
        // EGC: every produced variable must be consumed in egc mode.
        if analyzer.enforce_egc {
            for state in analyzer.branch_contexts.values() {
                for var in &state.produced {
                    if var != "_"
                        && !var.starts_with('_')
                        && !state.consumed.contains(var)
                    {
                        return Err(analyzer.annotate(
                            SemanticErrorKind::UnconsumedVariable(var.clone()),
                        ));
                    }
                }
            }
        }

        // Full symbolic SMT verification: WCET path conditions, temporal contracts,
        // and isolate bounds via WcetSolver.
        if analyzer.use_z3 {
            let mut wcet_solver =
                WcetSolver::<crate::oxiz::OxiZBackend>::new(analyzer);
            wcet_solver.verify_and_compute(program)?;
        }

        Ok(())
    }
}
