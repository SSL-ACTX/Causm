# Causm Core Semantic Model

This document specifies the fundamental semantic regulations governing the Causm.

---

## 1. Entropic Memory Model

The Entropic memory model constitutes the foundational architecture of Causm. It conceptualizes values not as static data structures, but as dynamic energy states that undergo evolution, movement, and decay during the computational lifecycle.

### Entropic States
- **Valid**: The value is owned by the active branch and is fully accessible for computation.
- **Pending**: The value represents a promise (e.g., resulting from an asynchronous operation) that will be resolved at a future `local_clock` coordinate.
- **Decayed**: The value (typically a structure) has undergone partial consumption of its internal fields. The parent structure is no longer "sealed" and cannot be moved or transmitted as a unified entity.
- **Consumed**: The value has been destructively read (e.g., via a `let` assignment, move, or routine consumption) and is no longer present within the memory arena.
- **Leased**: The value is temporarily borrowed under a time-bounded lease; moves are prohibited during the lease duration.

### Consumption Regulations
1. **Movement by Default**: Assignments (e.g., `let a = b`) transfer ownership of `b` to `a`. Subsequently, `b` transitions to the `Consumed` state.
2. **Structural Decay**: Accessing a sub-field `s.f` consumes `f` and transitions the parent structure `s` to the `Decayed` state.
3. **Explicit Replication**: Reusing a value without consumption requires the `clone(x)` operation, which incurs a deterministic temporal cost.
4. **Non-Consuming Borrow**: Reading via `&x` or passing `peek` parameters does not consume `x` or decay parent structures.

---

## 2. Deterministic Temporal Execution

Causm enforces rigorous, predictable execution durations for all computational operations.

### Local Temporal Clock and Resource Budget
- Every execution branch maintains a `local_clock`, measured in milliseconds.
- Every instruction possesses a deterministic temporal cost (base cost: 1ms).
- Branches are initialized with a defined `cpu_budget_ms`. Exceeding this budget triggers a runtime `BudgetExhausted` fault.

### Pacing and Deterministic Padding
- Iterative constructs (e.g., `for` loops) and routine invocations enforce temporal contracts.
- **Deterministic Padding**: If an execution block completes prior to its contracted duration (e.g., `taking 20ms`), the Register-based Temporal Virtual Machine (TVM) automatically pads the `local_clock` to satisfy the contract, ensuring that execution duration is independent of the source environment.
- **Temporal Watchdogs**: If an execution block exceeds its allocated duration, a `WatchdogBite` is triggered, facilitating the execution of recovery logic.

---

## 3. Timeline Isolation and Concurrency

Concurrency in Causm is modeled through isolated **Timelines** (branches).

### Split and Reconciliation
- **`split`**: Generates child timelines initialized with a snapshot of the parent arena and temporal clock.
- **`merge`**: Recombines child timelines into the parent context. Conflicts (e.g., concurrent modification or consumption of shared state) must be resolved via explicit `reconcile` or `resolving` protocols.
- **`entangle`**: Binds variables across split timelines to share a unified entropic state with zero-latency state reflection.

---

## 4. Isochronous Scheduling Matrix

For applications requiring high-precision timing, Causm supports the **Isochronous Matrix** scheduling model.

- **Temporal Slices**: The `slice Nms` primitive establishes a fixed tick frequency.
- **Phase Commits**: `loop tick` blocks ensure that operations occur within deterministic time boundaries and commit state at tick borders.

---

## 5. Causal Reversion and Paradox Mitigation

Causm facilitates high-assurance state recovery through the `anchor` and `rewind_to` primitives.

### Temporal Integrity Maintenance
- **Temporal Restoration**: Reverting to an anchor restores the branch to the precise `local_clock` coordinate of that anchor.
- **State Restoration**: The memory arena is restored to the exact snapshot recorded at the anchor point.

### Paradox Prevention Mechanisms
To maintain temporal consistency, the TVM prevents the occurrence of **Causal Paradoxes**:
1. **Unconsumed Side Effects**: A branch may rewind to an anchor as long as state invariants remain mathematically satisfiable.
2. **Causal Horizon Locking**: If state modifications have been irrevocably entangled and merged, attempts to rewind past that boundary trigger a `Causal Paradox` error.

---

## 6. Expression Evaluation and Coercion

### Binary Addition and Concatenation
The `+` operator exhibits polymorphic behavior based on the inferred types of its operands:
1. **Numeric Addition**: If both operands are numeric (Integer or Float), standard arithmetic addition is performed.
2. **String Concatenation**: If either operand is a **String**, the other operand is coerced into its string representation (using the internal `Display` implementation), and the two are concatenated.

### Operator Semantics
- **Modulo (`%`)**: Returns the remainder of division. For floats, this follows IEEE 754 remainder semantics.
- **Power (`^`)**: Performs exponentiation. Negative exponents for integers result in a transition to **Float** types to preserve precision.
- **Logical NOT (`!`)**: Strcausmy operates on **Bool** types.

### Index Access and Type Decay
- Accessing a field via index access (e.g., `top[idx]`) triggers **Structural Decay** on the target if it is an entropic structure or topology, similar to static field access.
- Since the index is dynamic, the static analyzer may assign an `Unknown` type to the result if the specific field type cannot be determined at compile time.
