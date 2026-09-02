use crate::analyzer::{EntropicAnalyzer, SemanticError};
use causm_core::Program;

/// The 4-Stage Decoupled Analysis Pipeline.
///
/// Drives the complete semantic analysis of a Causm program through four
/// composable, sequentially-executed stages:
///
/// ```text
/// Stage 1  hir::HirStage     — HIR Resolution & Capability Gating
/// Stage 2  ssa::SsaStage     — SSA Construction & Control Flow Graph
/// Stage 3  solver::SolverStage — Entropius Solver (EGC + SMT)
/// Stage 4  codegen::CodegenStage — Optimization & Bytecode Lowering
/// ```
///
/// Each stage receives a mutable reference to the shared `EntropicAnalyzer`
/// and the immutable program AST. Stages communicate solely through the
/// analyzer's symbol tables and branch contexts — no opaque shared state.
pub struct AnalysisPipeline<'a> {
    analyzer: &'a mut EntropicAnalyzer,
}

impl<'a> AnalysisPipeline<'a> {
    pub fn new(analyzer: &'a mut EntropicAnalyzer) -> Self {
        Self { analyzer }
    }

    pub fn run(self, program: &Program) -> Result<(), SemanticError> {
        // Stage 1: HIR resolution — intrinsics, types, routines, interfaces.
        crate::resolve::ResolveStage::run(self.analyzer, program);

        // Stage 2a (Entropius Relational Pre-pass):
        //   Extract facts from the raw AST and verify Invariants 1, 2, 3.
        //   This runs BEFORE SsaStage so the rich multi-span diagnostic fires
        //   before any legacy procedural error can mask it.
        crate::solver::SolverStage::run_relational(self.analyzer, program)?;

        // Stage 2b: SSA semantic analysis — timeline traversal + live-range tracking.
        //   Populates branch_contexts.produced, routines WCET, entanglement sets.
        crate::ssa::SsaStage::run(self.analyzer, program)?;

        // Stage 2c (Post-SSA Solver): EGC unconsumed-variable check + full symbolic
        //   SMT WcetSolver (WCET, temporal contracts, and isolate budgets) that depend on
        //   the populated branch_contexts and routines from SsaStage.
        crate::solver::SolverStage::run_post_ssa(self.analyzer, program)?;

        // Stage 4: Codegen — optimization coordination (stub; optimizer runs in causm-ir).
        crate::codegen::CodegenStage::run(self.analyzer, program)?;

        Ok(())
    }
}
