# Specification: Z3 Formal Verification Guard (Correctness Kernel)

## 1. Introduction
The Causm Correctness Kernel is a formal verification pass that utilizes the Z3 SMT solver to rigorously prove temporal and entropic safety. Unlike the standard analyzer which uses forward-simulation, the Formal Verification Guard performs symbolic execution to ensure that invariants hold across all possible execution paths.

## 2. Symbolic Temporal Enforcement
The kernel models the `local_clock` as a Z3 integer variable.

### 2.1 Worst-Case Execution Time (WCET)
For every isolated block or routine, the kernel proves that:
`Path_Condition(P) => Local_Clock(P) <= Budget`
Where `Budget` is defined in the manifest or routine contract.

### 2.2 Relativistic Pacing
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
A "Causal Commitment" is any operation with irreversible external side effects (e.g., `chan_send`). When a commitment occurs at `Time(T)`, the symbolic `causal_horizon` is updated:
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
