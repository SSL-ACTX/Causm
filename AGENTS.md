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

### 2.1 Fast Feedback Loop (Compile Limits)
*   **Parallel Compilation Limit:** If executing on Termux or other resource-constrained host environments, limit cargo processes to 2 jobs (`-j 2`) (e.g., `cargo check -j 2`, `cargo test -j 2`) to prevent CPU exhaustion. On standard developer environments, do NOT use `-j` limits.
*   **Dev Mode Speed:** Utilize `cargo check` (optionally with `-j 2` if on Termux) for fast feedback during iteration. Only invoke the full `cargo test` suite when modifications are stable and ready for validation.

### 2.2 Zero Warnings Policy
*   **Clean Builds:** We maintain a strict zero-warning policy. All unused imports, variables, and dead code must be resolved or removed before committing.
*   **Lints:** Run `cargo clippy -- -D warnings` before finalized commits.

### 2.3 Adding New Features & Instructions
When adding new syntax, instructions, or VM capabilities:
1.  Extend the grammar in `src/frontend/causm.pest`.
2.  Implement AST traversal/visitor in `src/frontend/parser.rs`.
3.  Add IR lowering mapping in `src/frontend/ir.rs` or `causm-ir`.
4.  Update dominance/SSA transformation logic in `causm-ir/src/ssa.rs` if registers or control flow terminators change.
5.  Add entropic verification checks in `crates/causm-analysis/src/analyzer.rs`.
6.  Wire VM execution logic in `crates/causm-runtime/src/vm/core.rs`.
7.  Add targeted parser, analyzer, and integration tests to verify the pipeline.

---

## 3. AI Agent Execution Mandates (Strict & Non-Negotiable)

### 3.1 Obey the User Exactly
*   **Absolute Law:** If the user points to a specific bug, file, or line, investigate that path immediately. The user's diagnosis is the highest priority signal.
*   **Do Not Stall or Loop:** Do not run redundant check loops, fallback on generic boilerplate diagnostics, or examine irrelevant folders to stall. Follow user directions proactively.

### 3.2 Read Rules Before Acting
*   **Check Copilot Instructions:** Always read `.github/copilot-instructions.md` and this `AGENTS.md` document first to understand project constraints before making changes.

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
