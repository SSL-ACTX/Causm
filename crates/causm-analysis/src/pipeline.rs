use causm_core::Program;
use causm_types::analyzer::{EntropicAnalyzer, SemanticError};

/// The 4-Stage Decoupled Analysis Pipeline.
pub struct AnalysisPipeline<'a> {
    analyzer: &'a mut EntropicAnalyzer,
}

impl<'a> AnalysisPipeline<'a> {
    pub fn new(analyzer: &'a mut EntropicAnalyzer) -> Self {
        Self { analyzer }
    }

    pub fn run(self, program: &Program) -> Result<(), SemanticError> {
        // Stage 1: HIR resolution & Capability Gating
        causm_types::ResolveStage::run(self.analyzer, program);

        // Stage 2a (Entropius Relational Pre-pass): Invariants 1–3
        crate::solver::SolverStage::run_relational(self.analyzer, program)?;

        // Stage 2b: SSA semantic analysis
        causm_types::SsaStage::run(self.analyzer, program)?;

        // Stage 2c (Post-SSA Solver): EGC check + symbolic WCET contracts
        crate::solver::SolverStage::run_post_ssa(self.analyzer, program)?;

        // Stage 4: Codegen
        crate::codegen::CodegenStage::run(self.analyzer, program)?;

        Ok(())
    }

    pub fn run_hir(self, hir: &causm_core::HirProgram) -> Result<(), SemanticError> {
        self.run(hir)
    }
}

pub fn run_pipeline(
    analyzer: &mut EntropicAnalyzer,
    program: &Program,
) -> Result<(), SemanticError> {
    AnalysisPipeline::new(analyzer).run(program)
}
