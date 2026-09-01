pub mod backend;
pub mod facts;

pub use backend::SolverBackend;
pub use facts::EntropicFact;

use crate::analyzer::{EntropicAnalyzer, SemanticError, SemanticErrorKind};
use causm_core::Program;

/// Stage 3 of the analysis pipeline: the Entropic-Polonius Solver.
///
/// Enforces the Entropic Garbage Collection invariant and runs the
/// formal SMT verifier over the analysed program to prove Invariants 1-3.
pub struct SolverStage;

impl SolverStage {
    pub fn run(
        analyzer: &mut EntropicAnalyzer,
        program: &Program,
    ) -> Result<(), SemanticError> {
        // Invariant: every produced variable must be consumed (EGC mode).
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

        // Formal verification via oxiz / Z3 backend.
        if analyzer.use_z3 {
            let mut verifier = crate::verifier::FormalVerifier::<
                crate::oxiz::OxiZBackend,
            >::new(analyzer);
            verifier.verify(program)?;
        }

        Ok(())
    }
}
