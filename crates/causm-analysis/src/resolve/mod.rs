pub mod declarations;

pub use declarations::run_resolve_stage;

/// Stage 1 of the analysis pipeline: HIR Resolution & Capability Gating.
///
/// Registers built-in intrinsics, monomorphised type/routine/interface
/// declarations, and global state declarations so that later stages have a
/// complete symbol table before any expression is analysed.
pub struct ResolveStage;

impl ResolveStage {
    pub fn run(
        analyzer: &mut crate::analyzer::EntropicAnalyzer,
        program: &causm_core::Program,
    ) {
        analyzer.register_intrinsics();
        declarations::pre_register_program_declarations(analyzer, program);
    }
}
