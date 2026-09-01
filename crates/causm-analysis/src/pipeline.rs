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
/// Stage 3  solver::SolverStage — Entropic-Polonius Solver (EGC + SMT)
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

        // Stage 2: SSA semantic analysis — timeline traversal + live-range tracking.
        crate::ssa::SsaStage::run(self.analyzer, program)?;

        // Stage 3: Entropic-Polonius solver — EGC invariant + SMT formal verification.
        crate::solver::SolverStage::run(self.analyzer, program)?;

        // Stage 4: Codegen — optimization coordination (stub; optimizer runs in causm-ir).
        crate::codegen::CodegenStage::run(self.analyzer, program)?;

        Ok(())
    }
}
