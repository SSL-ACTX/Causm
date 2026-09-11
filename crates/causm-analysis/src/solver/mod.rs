pub use causm_entropius::diagnostics;
pub use causm_entropius::diagnostics::EntropicDiagnostic;
pub use causm_entropius::facts;
pub use causm_entropius::facts::{
    extract_facts, extract_ssa_facts, EntropicFact, PointIndex, ProgramFacts,
    SsaPointIndex,
};
pub use causm_entropius::relational;
pub use causm_entropius::relational::RelationalInvariantSolver;
pub use causm_smt::backend;
pub use causm_smt::SolverBackend;
pub use causm_wcet::wcet;
pub use causm_wcet::wcet::WcetSolver;

use causm_core::Program;
use causm_types::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};

/// Stage 2a/2c of the analysis pipeline: the Entropius Solver.
pub struct SolverStage;

impl SolverStage {
    /// Stage 2a: Relational pre-pass. Extract ProgramFacts and verify Invariants 1–3.
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
            RelationalInvariantSolver::<causm_smt::OxiZBackend>::new(analyzer);
        if let Err(err) = relational_solver.solve_invariants(&facts) {
            return Err(
                analyzer.annotate(SemanticErrorKind::EntropiusDiagnostic(err.0))
            );
        }

        Ok(())
    }

    /// Stage 2c: Post-SSA checks. EGC unconsumed-variable enforcement + WcetSolver
    pub fn run_post_ssa(
        analyzer: &mut EntropicAnalyzer,
        program: &Program,
    ) -> Result<(), SemanticError> {
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

        if analyzer.use_z3 {
            let mut wcet_solver =
                WcetSolver::<causm_smt::OxiZBackend>::new(analyzer);
            wcet_solver.verify_and_compute(program)?;
        }

        Ok(())
    }
}
