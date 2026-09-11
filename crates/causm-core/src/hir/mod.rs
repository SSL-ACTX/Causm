//! Canonical High-Level Intermediate Representation (HIR)
//!
//! HIR and AST share the unified canonical representation types in `causm_core`.
//! Frontends desugar surface constructs (e.g. `using`, `f"..."`, macros) into
//! canonical `Statement` / `Expression` HIR nodes without requiring expensive
//! bidirectional roundtrip conversion passes.

pub type HirProgram = crate::Program;
pub type HirTimelineBlock = crate::TimelineBlock;
pub type HirSpannedStatement = crate::SpannedStatement;
pub type HirStatement = crate::Statement;
pub type HirExpression = crate::Expression;
pub type HirMatchArm = crate::MatchArm;
pub type HirSelectCase = crate::SelectCase;
pub type HirMatchExprArm = crate::MatchExprArm;
pub type HirExprMatchArm = crate::MatchExprArm;

/// Backward-compatible identity helper
#[inline]
pub fn hir_to_program(hir: &HirProgram) -> crate::Program {
    hir.clone()
}

/// Backward-compatible identity helper
#[inline]
pub fn sync_resolved_calls(_from_ast: &crate::Program, _to_hir: &HirProgram) {}
