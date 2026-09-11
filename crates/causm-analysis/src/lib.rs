pub mod analyzer {
    pub use causm_types::analyzer::BranchState;
    pub use causm_types::analyzer::RoutineInfo;
    pub use causm_types::analyzer::SemanticError;
    pub use causm_types::analyzer::SemanticErrorKind;

    pub type InnerAnalyzer = causm_types::analyzer::EntropicAnalyzer;

    #[derive(Clone)]
    pub struct EntropicAnalyzer {
        pub inner: InnerAnalyzer,
    }

    impl std::ops::Deref for EntropicAnalyzer {
        type Target = InnerAnalyzer;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl std::ops::DerefMut for EntropicAnalyzer {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    impl Default for EntropicAnalyzer {
        fn default() -> Self {
            Self::new()
        }
    }

    impl EntropicAnalyzer {
        pub fn new() -> Self {
            let mut inner = InnerAnalyzer::new();
            inner.pipeline_runner = Some(crate::pipeline::run_pipeline);
            Self { inner }
        }

        pub fn analyze_program(
            &mut self,
            program: &causm_core::Program,
        ) -> Result<(), SemanticError> {
            crate::pipeline::AnalysisPipeline::new(&mut self.inner).run(program)
        }

        pub fn analyze_hir(
            &mut self,
            hir: &causm_core::HirProgram,
        ) -> Result<(), SemanticError> {
            crate::pipeline::AnalysisPipeline::new(&mut self.inner).run_hir(hir)
        }

        pub fn analyze_program_with_source(
            &mut self,
            program: &causm_core::Program,
            source: &str,
            filename: &str,
        ) -> Result<(), SemanticError> {
            self.source = Some(source.to_string());
            self.filename = Some(filename.to_string());
            let result = self.analyze_program(program);
            self.source = None;
            self.filename = None;
            result
        }

        pub fn analyze_hir_with_source(
            &mut self,
            hir: &causm_core::HirProgram,
            source: &str,
            filename: &str,
        ) -> Result<(), SemanticError> {
            self.source = Some(source.to_string());
            self.filename = Some(filename.to_string());
            let result = self.analyze_hir(hir);
            self.source = None;
            self.filename = None;
            result
        }
    }
}

pub use causm_types::expression;
pub use causm_types::resolve;
pub use causm_types::ssa;
pub use causm_types::statement;
pub use causm_types::statements;

pub use causm_smt as smt;
pub use causm_smt::oxiz;
#[cfg(feature = "z3")]
pub use causm_smt::z3;

pub use causm_entropius as entropius;
pub use causm_wcet as wcet;

pub mod codegen;
pub mod pipeline;
pub mod solver;

pub use pipeline::{run_pipeline, AnalysisPipeline};
