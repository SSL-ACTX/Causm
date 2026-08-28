# Causm Agent Development Rules & Architecture

This document outlines the core architectural constraints, design patterns, development workflows, and AI agent execution mandates for the Causm project. These rules must be strictly adhered to during all development.

## 1. Architectural Constraints

### 1.1 Formal Entropic & Causal Verification
*   **Static Analysis First:** All code is subject to flow-sensitive static analysis utilizing the Z3 solver to verify the safety of temporal gates, entanglement invariants, and entropic decay.
*   **Safety Proofs:** Compiling or executing code is only permitted once static verification has mathematically proved the absence of logic conflicts or illegal access to consumed/decayed states.

### 1.2 Strict SSA & CFG Pipeline
*   **Flat IR to CFG:** Parser output is lowered to flat instructions and then mapped to Basic Blocks by identifying leader instructions and control flow terminators (`Jump`, `Branch`, `Return`).
*   **Dominance Tree & Frontier:** The dominance relationship is computed using the Cooper-Harvey-Kennedy algorithm to identify dominance frontiers.
*   **Phi Insertion & Versioning:** SSA form is enforced by inserting $\phi$ (Phi) nodes at dominance frontiers and renaming register variables into versioned `SsaReg` instances (e.g., `R1_2`).

### 1.3 Timeline Budget & VM Isochronous Loop
*   **Isochronous Loop Bounds:** The execution runtime enforces absolute temporal constraints (WCET budgets). 
*   **Deterministic Clock Progression:** All clock advancements, yield paces, and time-slice budgets are tracked and verified at compile-time and runtime.

### 1.4 Memory Arena & State Decay
*   **Entropic States:** Variable values reside in the memory arena and transition between states: `Valid`, `Leased`, `Decayed`, `Pending`, and `Consumed`.
*   **No Unsafe Aliasing:** The VM must prevent dereferencing or mutating variables that have been consumed or have passed their temporal lease without formal reconciliation.

---

## 2. Rust Codebase Rules

### 2.1 Fast Feedback Loop & Build Pipelines
*   **Default Workspace Members (Exclude Plugins from Default Builds):** The root `Cargo.toml` sets `default-members = ["crates/*"]`. Standard `cargo check` and `cargo build` will ONLY build core crates and will NOT build plugins to preserve CPU and build time.
*   **Specialized Cargo Aliases (in `.cargo/config.toml`):**
    *   `cargo check-core` / `cargo build-core`: Fast check/build of compiler crates only (`-j 3`).
    *   `cargo build-plugins`: Builds WASM plugins (`wasm32-unknown-unknown`).
    *   `cargo check-all` / `cargo build-all`: Builds entire workspace including plugins (`--workspace -j 3`).
    *   `cargo test-fast` / `cargo test-all`: Runs test suites with `-j 3` parallel limits.
*   **Parallel Compilation Limit:** If executing on Termux or other resource-constrained host environments, limit cargo processes to 3 jobs (`-j 3`) to prevent CPU exhaustion. On standard developer environments, do NOT use `-j` limits.
*   **Dev Mode Speed:** Utilize `cargo check-core` (or `cargo check`) for fast feedback during iteration. Only invoke the full `cargo test-all` suite when modifications are stable and ready for validation.

### 2.2 Zero Warnings Policy
*   **Clean Builds:** We maintain a strict zero-warning policy. All unused imports, variables, and dead code must be resolved or removed before committing.
*   **Lints:** Run `cargo clippy -- -D warnings` before finalized commits.

### 2.3 Adding New Features & Instructions
When adding new syntax, instructions, or VM capabilities:
1.  Extend the grammar in `crates/causm-frontend/src/causm.pest`.
2.  Implement AST types in `crates/causm-core/src/lib.rs` and parser visitors in `crates/causm-frontend/src/parser/`.
3.  Add IR lowering mapping in `crates/causm-frontend/src/lower/`.
4.  Update SSA/CFG transformation logic in `crates/causm-ir/src/ssa/` if registers or control flow terminators change.
5.  Add entropic verification, cost estimation, type inference, and validation passes in `crates/causm-analysis/src/`.
6.  Wire VM execution logic in `crates/causm-runtime/src/vm/`.
7.  Add isolated, modular test cases in `crates/causm-cli/tests/integration/` (or relevant crate unit tests) adhering to strict naming, category, and target precision guidelines.

### 2.4 Strict Test Case Mandates
*   **Target Precision:** Every new feature, syntax addition, or bug fix MUST have dedicated, isolated test functions targeting that specific feature individually. Never bundle multiple distinct features into a single generic test suite function.
*   **Clear Category & Naming Conventions:** Test functions must follow descriptive, structured naming conventions that declare the feature category and test target: `test_<category>_<feature>_<scenario>` (e.g., `test_syntax_uninitialized_let_definite_assignment`, `test_entropy_lifetime_annotation_decayed_lease`, `test_enum_variant_pattern_matching`, `test_temporal_paced_range_step_loop`).
*   **Assertion Precision:** Each test must assert exact expected behaviors, values, and error states directly without relying on side-effect approximations.

---

## 3. AI Agent Execution Mandates (Strict & Non-Negotiable)

### 3.1 Obey the User Exactly & Absolute Truthfulness
*   **Absolute Law:** If the user points to a specific bug, file, or line, investigate that path immediately. The user's diagnosis is the highest priority signal.
*   **No False Completion Claims:** NEVER claim a feature, syntax, or bug fix is done or fully implemented until every single layer of the pipeline (AST, parser, analyzer passes, IR lowering, VM execution, and tests) is completely wired up and empirically verified.
*   **No Obfuscated Command Output:** NEVER pipe commands with `2>&1 | tail -N` or swallow error outputs. Run build and test commands cleanly so all compiler diagnostics and test backtraces are fully visible.
*   **Do Not Stall or Loop:** Do not run redundant check loops, fallback on generic boilerplate diagnostics, or examine irrelevant folders to stall. Follow user directions proactively.

### 3.2 Read Rules Before Acting
*   **Check Copilot Instructions:** Always read `.github/copilot-instructions.md` and this `AGENTS.md` document first to understand project constraints before making changes.

### 3.3 Strict Tooling & Scope Isolation
*   **Formatter vs Compiler Isolation:** When working on developer tooling (such as `causm fmt` or `crates/causm-devtools`), NEVER modify the compiler, parser, AST types, analysis, IR lowering, or runtime crates (`causm-core`, `causm-frontend`, `causm-analysis`, `causm-ir`, `causm-runtime`) unless explicitly ordered by the user. Formatter modifications MUST remain strictly isolated within `crates/causm-devtools`.
*   **Preserve Working Systems:** Never destabilize or rewrite working compiler subsystems to suit peripheral tooling. Tooling must adapt to the compiler architecture, never the other way around.

---

## 4. Git Commit Standards

### 4.1 Conventional Commits Format
*   **Subject Line:** Commit messages must follow the conventional format: `<type>(<scope>): <description>` (or `<type>: <description>` if scope is absent).
*   **Case Sensitivity:** The description must be in all lowercase.
*   **Structure:** Follow the subject line with a blank line, and then a detailed body with bullet points starting with `-` describing the modifications.
*   **Examples:**
    *   ```
        feat(ssa): implement Cooper-Harvey-Kennedy dominator tree and phi insertion
        
        - Added cfg leader block identification and branch terminators
        - Implemented dominance frontier calculation and variable renaming
        - Integrated --dump ssa CLI flag to print versioned CFG blocks
        ```
    *   ```
        fix(analysis): resolve use-after-consume validation bypass in match entropy
        
        - Added in_entropy_match check to analyze_expression_nonconsuming
        - Allowed compile-time querying of consumed variables within match expressions
        - Added regression integration tests for auto-reconciliation Success
        ```
