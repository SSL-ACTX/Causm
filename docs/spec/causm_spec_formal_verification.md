# Specification: SMT Formal Verification Guard (Correctness Kernel)

## 1. Introduction
The Causm Correctness Kernel is a formal verification pass that utilizes an SMT solver backend (featuring pure-Rust **OxiZ** as the default lightweight solver and optional **Z3** integration) to rigorously prove temporal and entropic safety. Unlike the standard analyzer which uses forward-simulation, the Formal Verification Guard performs symbolic execution to ensure that invariants hold across all possible execution paths.

## 2. Symbolic Temporal Enforcement
The kernel models the `local_clock` as a symbolic integer variable in the solver.

### 2.1 Worst-Case Execution Time (WCET)
For every isolated block or routine, the kernel proves that:
`Path_Condition(P) => Local_Clock(P) <= Budget`
Where `Budget` is defined in the manifest or routine contract.

### 2.2 WCET Bound Extraction
Beyond proving that a WCET budget is *not violated*, the kernel also computes and reports the **concrete upper bound** of each routine or global timeline using an SMT binary search (`find_max_value`):
1. The kernel queries the solver for the maximum satisfiable clock value under path constraints.
2. The extracted bound is stored per-routine and per-timeline.
3. The bound is printed in the CLI output when running with `-v` (verbose mode).

This allows users to observe exact worst-case bounds even for code without explicit `taking Nms` annotations.

### 2.3 Relativistic Pacing
In `loop tick` constructs, the kernel proves that the cumulative cost of the body plus deterministic temporal padding exactly equals the `slice_ms` defined in the context.

## 3. Inductive Loop Proofs
To ensure safety in iterative constructs, the kernel employs **Bounded Symbolic Unrolling**.

### 3.1 Induction Step
The kernel simulates the loop body twice:
1.  **Iteration 1**: Verifies safety from the loop entry state.
2.  **Iteration 2**: Verifies that the state produced by Iteration 1 is safe for a subsequent iteration.

This proves that entropic operations (like `yield` or `move`) do not result in double-consumption of non-refreshed variables (e.g., variables declared outside the loop).

## 4. Formal Paradox Prevention
Causal consistency is enforced through symbolic tracking of the **Causal Horizon**.

### 4.1 Commitment Logic
A "Causal Commitment" is any operation with irreversible external side effects (e.g., cross-branch state synchronizations, hardware I/O). When a commitment occurs at `Time(T)`, the symbolic `causal_horizon` is updated:
`New_Horizon = ite(Path_Taken, T, Old_Horizon)`

### 4.2 Paradox Invariant
The kernel proves that for any `rewind_to(Anchor)` operation:
`Path_Taken => Time(Anchor) >= Causal_Horizon`
If Z3 finds a model where an anchor precedes a commitment on the same path, the program is rejected as acausal.

## 5. Entanglement Graph Validation
Variables can be logically coupled using the `entangle` primitive.

### 5.1 Symbolic State Propagation
Entangled variables share a unified entropic lifecycle. The kernel models this as a set of logical implications:
`Is_Consumed(Var_A) => Is_Consumed(Var_B) AND Is_Consumed(Var_C)`
This ensures that destructive operations on any member of the entanglement group are immediately reflected across the entire group, preventing use-after-decay violations.

## 6. Precise Diagnostics
The kernel leverages Z3's **Unsat Cores** to identify the minimal set of statements responsible for a violation, providing the user with pinpointed error locations rather than generic analysis failures.

---

## 7. IR Optimization Passes (Pre-Execution)

Before TVM execution, the IR is processed through a sequential pass pipeline. These passes complement the formal kernel by eliminating dead code and validating structural IR invariants.

### 7.1 `CfgSimplificationPass`
Simplifies the Control Flow Graph with three sub-phases:
- **Dead Block Elimination**: Removes basic blocks that are unreachable from the entry. Temporal entry points (`@Nms` blocks) and decay handlers are always considered reachable.
- **Branch Folding**: Resolves constant-condition branches at compile time, replacing `Branch(true, A, B)` with an unconditional `Jump(A)`.
- **Empty Jump Bypass**: Collapses basic blocks that consist solely of an unconditional jump, redirecting predecessors directly to the target.

### 7.2 `ChannelLivenessPass`
Eliminates dead `OpenChan` instructions where the opened channel is never referenced by any `ChanSend`, `ChanRecv`, or `LoopTickOn` instruction. Performs a **global scan** across all IR blocks, routines, and decay handlers before per-block elimination to prevent incorrect removal of channels whose uses reside in a different temporal block.

### 7.3 `LeaseOptimizationPass`
Eliminates unnecessary lease instructions:
- **Zero-duration leases**: `Lease(reg, 0ms)` instructions have no effect and are removed.
- **Unused register leases**: Lease instructions on registers that are never subsequently used are pruned.

### 7.4 `ConcurrencyAnalysisPass`
Verifies that split/merge branch patterns maintain consistent register lifetimes. Checks that registers live at a `Split` point are still valid (not consumed) at the corresponding `Merge` point across all branch paths.

### 7.5 `VerifierPass`
Validates structural SSA invariants after all optimization passes:
- **Phi node predecessor validity**: Every predecessor listed in a `Phi` node must be a real predecessor block in the CFG.
- **Temporal block structure**: Verifies that isochronous blocks (`@Nms`) conform to required entry/exit structural constraints.
