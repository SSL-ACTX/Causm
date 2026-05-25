# Contributing to Causm

Thank you for contributing. Please follow these guidelines.

## Workflow
1. Open issues for feature requests or bugs.
2. Create a feature branch: `git checkout -b feat/<summary>`.
3. Make small, focused commits; prefer one feature per PR.
4. Add a targeted test case for every behavior change.
5. Run:
   - `cargo test`
   - `cargo fmt`

## Testing
- Unit tests live in `src/` and integration tests in `tests/`.
- Keep tests minimal and deterministic.
- Use the same small sample programs that demonstrate the behavior.

## Commits
- Use Conventional Commits:
  - `feat:` for new language constructs.
  - `fix:` for bugfixes.
  - `chore:` for tooling/docs.
  - `test:` for adding tests.

## Code structure
The project is organized into several crates within the `crates/` directory:
- `crates/causm-frontend`: Parser (Pest), AST, and IR lowering.
- `crates/causm-analysis`: Entropic static analyzer and Z3 correctness kernel.
- `crates/causm-runtime`: Temporal Virtual Machine (TVM) and Entropic GC.
- `crates/causm-core`: Common types, values, and the facade for integration.
- `crates/causm-cli`: The primary command-line interface for the Causm toolchain.
- `lsp/`: Language Server Protocol implementation for IDE support.

## Review
- Ensure static analyzer invariants are preserved.
- Ensure `split` / `merge` memory semantics are unchanged unless intentional.
- Document temporal costs and branch interactions in comments.
