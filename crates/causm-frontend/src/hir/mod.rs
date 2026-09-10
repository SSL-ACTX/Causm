//! HIR (High-Level IR) Desugaring Pass
//!
//! This pass runs on a `causm_core::Program` **after** parsing and macro/derive
//! expansion, but **before** entropic analysis and IR lowering.
//!
//! Transformations applied:
//! 1. **Pipeline `|>`** — `x |> f(y)` → `f(x, y)` (first arg injection)
//! 2. **`using` resource scope** — already lowered by `lower/statements.rs`;
//!    HIR validates the binding and flags missing `drop` implementations.
//! 3. **`f"..."` string interpolation** — already handled in lowering; HIR
//!    canonicalises into `Expression::FString` for uniform treatment.
//!
//! Each sub-pass is isolated in its own submodule.

pub mod desugar;

use causm_core::Program;

/// Run all HIR desugaring passes on `program` in-place.
/// Call this after `macro_expand::expand_program` and `derive::expand_derives`,
/// before `lower::lower_program`.
pub fn desugar_program(program: &mut Program) {
    desugar::pipeline::desugar_pipeline(program);
}
