# Proposal: Relational Entropic Fact-Based Analysis (Entropius-Causm), Staged Analysis Pipelines, and Handler-Decoupled Virtual Machine Architecture

**Status:** Proposed (Targeting Future Architecture Milestone / v0.2.0+)  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Compiler Architecture, Formal Methods & Virtual Machine Engine  
**Target Crates:** `causm-analysis` (Major Redesign), `causm-runtime` (Refactor), `causm-ir`, `causm-core`

---

## 1. Executive Summary & Motivation

During early prototyping and language evolution, compiler implementations inevitably incur **Semantic Complexity Debt**:
1. **Ad-Hoc Entropic State Tracking:** Tracking variable consumption (`consume`), structural decay (`struct.field`), temporal leases (`lease for Nms`), and timeline merges (`split`/`merge`) via procedural AST/CFG traversal loops leads to brittle edge-case logic and diagnostic ambiguity.
2. **Monolithic Virtual Machine Evaluation:** A singular, oversized instruction evaluation loop (`eval.rs`) tightly couples operand decoding, arithmetic evaluation, arena memory management, capability checks, and C-FFI dispatching, creating maintenance friction and inhibiting formal model-checking.
3. **Monolithic Analysis Passes:** Performing type resolution, capability verification, static WCET topological path summation, and entropic safety in an intertwined analysis pass prevents incremental compilation and complicates formal verification.

This proposal introduces a comprehensive architectural redesign:
* **The Entropius Model:** Formulating Causm’s entropic memory, temporal lifetimes, and linear ownership as a **declarative, relational logic system** (inspired by Rust's Polonius and Non-Lexical Lifetimes) solved via Datalog facts and the embedded `oxiz` SMT engine.
* **The 4-Stage Decoupled Analysis Pipeline:** Restructuring `causm-analysis` into discrete, composable compiler stages mirroring `rustc`’s High-Level Intermediate Representation (HIR) to Mid-Level Intermediate Representation (MIR) flow.
* **Handler-Decoupled Virtual Machine Architecture:** Refactoring `causm-runtime` into isolated, domain-specific instruction handlers with centralized VM context dispatching.

---

## 2. Theoretical Framework: The Entropius Relational Model

Rust’s Polonius borrow checker reformulates memory safety from lexical scope trees into a relational Datalog logic model over a Control Flow Graph (CFG). Because Causm introduces **Temporal Lifetimes ($t$)**, **Linear Entropic Consumption**, and **Relativistic Timeline Branches**, we formalize the **Entropius Model**.

```
                   Control Flow Graph (CFG) Points P ∈ Points
                                     │
                   ┌─────────────────┴─────────────────┐
                   ▼                                   ▼
        [ Spatial Origin Facts ]            [ Temporal Clock Facts ]
        - OriginContains(O, R)              - TimeAt(P, t)
        - ValueIntroduced(R, P)             - LeaseIssued(R, L, t_start, t_end)
        - ValueConsumed(R, P)               - ValueDecayed(R, t_decay)
                   │                                   │
                   └─────────────────┬─────────────────┘
                                     ▼
                     ┌───────────────────────────────┐
                     │   oxiz / SMT Relational Proof │
                     │   (Proves Zero Illegal Access)│
                     └───────────────┬───────────────┘
                                     ▼
                 [ Valid IR ]  OR  [ Precise Diagnostic Span ]
```

### 2.1 Formal Relational Facts (Base Inputs)

For every instruction and point $P \in \text{Points}$ in the SSA CFG, the compiler extracts a tuple of fundamental relations:

$$\begin{aligned}
\text{ValueIntroduced}(v, P) &\quad \text{Variable } v \text{ is allocated or bound at CFG point } P. \\
\text{LinearConsume}(v, P) &\quad \text{Variable } v \text{ is explicitly moved, consumed, or auto-dropped at } P. \\
\text{FieldConsume}(v, f, P) &\quad \text{Field } f \text{ of struct } v \text{ is destructured/consumed at } P. \\
\text{LeaseIssued}(v, \lambda, t_1, t_2) &\quad \text{Lease } \lambda \text{ on } v \text{ is active across virtual clock interval } [t_1, t_2]. \\
\text{TemporalDecay}(v, t_{\text{expire}}) &\quad \text{Variable } v \text{ has an intrinsic TTL expiring at timestamp } t_{\text{expire}}. \\
\text{AccessAt}(v, P, t_{\text{current}}) &\quad \text{Variable } v \text{ is read, peeked, or referenced at } P \text{ at clock } t_{\text{current}}.
\end{aligned}$$

### 2.2 Relational Invariance Rules & Safety Theorems

The analysis engine proves that no access occurs on a consumed, decayed, or expired register across any execution path:

#### Invariant 1: Absence of Use-After-Consume
$$\text{IllegalConsumeAccess}(v, P) \iff \text{AccessAt}(v, P, \_) \land \exists P_{\text{prior}} \prec P \text{ s.t. } \text{LinearConsume}(v, P_{\text{prior}}) \land \neg \text{Reintroduced}(v, P_{\text{prior}}, P)$$

#### Invariant 2: Absence of Use-After-Decay (Temporal Invariant)
$$\text{IllegalTemporalAccess}(v, P, t) \iff \text{AccessAt}(v, P, t) \land \exists t_{\text{expire}} \le t \text{ s.t. } \text{TemporalDecay}(v, t_{\text{expire}})$$

#### Invariant 3: Structural Integrity Invariant
$$\text{StructInvalidated}(v, P) \iff \exists f \in \text{Fields}(v) \text{ s.t. } \text{FieldConsume}(v, f, P_{\text{prior}})$$
$$\text{AccessAt}(v, P, \_) \land \text{StructInvalidated}(v, P) \implies \text{EmitCompileError}(\text{"Use of structurally decayed compound struct"})$$

### 2.3 Advantages Over Procedural AST Traversal
* **Mathematical Proof:** Solved directly by `oxiz` relational rules; zero recursive tracking heuristics.
* **Span-Precise Diagnostics:** If an invariant fails, the solver outputs the exact tuple $\langle v, P_{\text{consume}}, P_{\text{access}} \rangle$, generating rich compiler error messages showing where the variable was consumed and where it was illegally accessed.

---

## 3. The 4-Stage Decoupled Compiler Analysis Pipeline

`causm-analysis` is refactored into four self-contained, sequentially executed stages with standardized data boundaries.

```
+-------------------------------------------------------------------------+
|                              Raw Program AST                            |
+-------------------------------------------------------------------------+
                                     │
                                     ▼
+-------------------------------------------------------------------------+
| Stage 1: HIR Resolution & Capability Gating (causm-analysis::hir)       |
|  - Monomorphizes <T> and Const Generics (e.g. FixedPacketBuffer<64>)    |
|  - Expands compiler macros (@derive, user macros)                       |
|  - Resolves method call chaining and dot syntax (obj.method())          |
|  - Validates Isolate capability declarations against active manifests   |
+-------------------------------------------------------------------------+
                                     │
                                     ▼
+-------------------------------------------------------------------------+
| Stage 2: SSA Construction & Control Flow Graph (causm-analysis::ssa)   |
|  - Builds SSA Basic Blocks and places φ (Phi) nodes                     |
|  - Performs Non-Lexical Live-Range Computation per virtual register     |
|  - Eliminates structural AST nesting in favor of flat branch blocks     |
+-------------------------------------------------------------------------+
                                     │
                                     ▼
+-------------------------------------------------------------------------+
| Stage 3: The Entropius Solver (causm-analysis::solver)          |
|  - Extracts relational facts from SSA instructions                      |
|  - Runs `oxiz` SMT solver to prove Entropic Invariants 1, 2, and 3      |
|  - Solves `taking _` topological WCET bounds                            |
|  - Proves Actor Schedulability: Handler WCET ≤ Actor TimeSlice          |
+-------------------------------------------------------------------------+
                                     │
                                     ▼
+-------------------------------------------------------------------------+
| Stage 4: Optimization & Bytecode Lowering (causm-analysis::codegen)     |
|  - Runs Cascading Dead Code Elimination (DCE)                           |
|  - Performs Constant & Copy Propagation                                 |
|  - Prunes import-internal unnamespaced symbol duplicates                |
|  - Emits final TVM Opcode Vector / WASM Artifact                        |
+-------------------------------------------------------------------------+
```

---

## 4. Handler-Decoupled Virtual Machine Architecture

To eliminate the monolithic 1,500-line `match` loop in `causm-runtime::vm::eval`, instruction execution is partitioned into modular domain handlers.

### 4.1 Proposed Crate Structure (`causm-runtime`)

```
crates/causm-runtime/src/vm/
├── core.rs                 # Central VM loop & instruction dispatcher
├── context.rs              # VmContext (Arena pointer, registers, clocks)
├── state.rs                # Execution state & register file
├── error.rs                # Temporal & Runtime Execution Errors
└── handlers/               # Modular Domain Instruction Handlers
    ├── mod.rs
    ├── arithmetic.rs       # BinaryOp, UnaryOp, Bitwise, Intrinsic Math
    ├── memory.rs           # ArenaAlloc, StructLit, TupleAccess, Slicing
    ├── entropic.rs         # Consume, ConsumeField, Lease, AutoDrop
    ├── control_flow.rs     # Branch, Jump, Call, Return, Phi-Resolution
    ├── temporal.rs         # Slice checking, YieldPad, RelativisticBlock
    ├── actor.rs            # causm-concurrency mailbox & actor dispatch
    └── ffi.rs              # POSIX C-ABI dlopen/dlsym & WASI host shims
```

### 4.2 Standardized Handler Interface

Every instruction handler implements a uniform execution signature operating on a decoupled `VmContext`:

```rust
pub struct VmContext<'a> {
    pub registers: &'a mut RegisterFile,
    pub arena: &'a mut MemoryArena,
    pub local_clock: &'a mut u64,
    pub global_clock: &'a mut u64,
    pub capabilities: &'a CapabilityStack,
    pub time_slice: &'a mut Option<TimeSlice>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum StepAction {
    Continue,
    Jump(usize),
    Yield(Option<Payload>),
    Return(Option<Payload>),
    SuspendActor,
    Terminate,
}

pub trait InstructionHandler<I> {
    fn execute(ctx: &mut VmContext, instruction: &I) -> Result<StepAction, TemporalError>;
}
```

### 4.3 Centralized Clean Dispatcher

`vm::core` collapses into a readable, maintainable 25-line dispatch loop:

```rust
impl Vm {
    pub fn step(&mut self) -> Result<StepAction, TemporalError> {
        let instr = self.fetch_current_instruction()?;
        let mut ctx = self.build_context();

        let action = match instr {
            Instruction::BinaryOp(op) => handlers::arithmetic::execute(&mut ctx, op),
            Instruction::StructLit(op) => handlers::memory::execute(&mut ctx, op),
            Instruction::Consume(op) => handlers::entropic::execute(&mut ctx, op),
            Instruction::Branch(op) => handlers::control_flow::execute(&mut ctx, op),
            Instruction::RelativisticBlock(op) => handlers::temporal::execute(&mut ctx, op),
            Instruction::ActorSend(op) => handlers::actor::execute(&mut ctx, op),
            Instruction::Syscall(op) => handlers::ffi::execute(&mut ctx, op),
            Instruction::AutoDrop(op) => handlers::entropic::execute_drop(&mut ctx, op),
        }?;

        self.advance_pc(&action);
        Ok(action)
    }
}
```

---

## 5. Synergies with Formal Verification (Kani & Aeneas)

This architectural refactoring directly accelerates the long-term verification roadmap:

1. **Modular Kani Verification:**
   Instead of model-checking a monolithic 5,000-line VM, Kani harnesses can be applied in isolation to `handlers::arithmetic`, `handlers::memory`, and `handlers::entropic`.
2. **Simplified Aeneas / Lean 4 Soundness Proofs:**
   In Aeneas, proving that an SSA instruction handler preserves semantic validity requires proving small, decoupled theorems per handler module rather than reasoning about an entangled execution loop.

---

## 6. Implementation Roadmap

| Phase | Milestone | Deliverable |
| :--- | :--- | :--- |
| **Phase 1** | **VM Context & Handlers Extraction** | Extract `VmContext` and split `eval.rs` into `handlers::*` modules without semantic changes. |
| **Phase 2** | **Staged Analysis Pipeline** | Partition `causm-analysis` into `hir`, `ssa`, `solver`, and `codegen` sub-modules. |
| **Phase 3** | **Polonius Relation Extraction** | Implement SSA fact extraction in `causm-analysis::solver` for variables and timeline points. |
| **Phase 4** | **`oxiz` Invariant Solver** | Implement relational safety rules (Invariants 1, 2, 3) in `oxiz` replacing procedural CFG walks. |
| **Phase 5** | **Span Diagnostics & Verification** | Wire solver output to rich compiler error spans and verify against the full 257+ test suite. |

---

## 7. Conclusion

By unifying Causm’s entropic memory and temporal execution under the **Entropius Relational Model**, staging compiler passes into clean compiler tiers, and modularizing the Virtual Machine into domain handlers, Causm eliminates semantic debt and establishes the definitive architectural foundation for enterprise stability, lightning-fast compilation, and mathematical verification.
