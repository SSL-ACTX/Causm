# Causm Specification: Temporal Leases

This document specifies the behavior of **Temporal Leases** in Causm. A lease provides transient, read-only, time-bound access to an entropic memory structure without triggering destructive consumption or requiring a full `clone()`.

---

## 1. Concepts and Motivation

The Entropic Memory Model typically requires `clone()` for non-destructive access, which incurs both CPU (temporal) and memory (spatial) costs. Temporal Leases introduce a "borrowing" mechanism where:
1. Access is granted for a strcausmy defined **temporal duration**.
2. The original variable is "locked" and cannot be moved or mutated during the lease.
3. The lease is **read-only** and does not generate a new allocation.
4. Deterministic padding ensures the lease always occupies its full allocated time.

---

## 2. Syntax

The `lease` primitive establishes a block-scoped binding for a specified duration, with optional state reconciliation.

**Syntax:**
```causm
lease <binding> = <source> <duration>ms {
    <statements>
} [reconcile (auto | <rules>)]
```

**Example:**
```causm
@0ms: {
    let system_state = struct { status = "OK", metrics = 42 }
    
    // Establishing a 20ms lease with auto reconciliation.
    // 'system_state' transitions to 'Leased' state.
    lease view = system_state 20ms {
        print("Status is: " + view.status)
    } reconcile auto
    
    // After exactly 20ms (plus setup), 'system_state' is 'Valid' again.
    debug(system_state)
}
```

---

## 3. Semantics and Constraints

### 3.1 Entropic State Transitions
- **Entry**: Upon entering the `lease` block, the `<source>` variable transitions from `Valid` to `Leased`.
- **Exit**: Upon exiting the block (after the duration expires), the `<source>` variable is restored to `Valid`.
- **Recursive Leasing**: A variable that is already `Leased` cannot be the source of another `lease` operation (No Nested Leasing).

### 3.2 Read-Only Invariant
The `<binding>` created by the lease is strcausmy immutable. Any attempt to mutate a field of a leased structure, or to pass a leased variable to a routine expecting a `consume` or `clone` parameter, will result in a `Semantic Error`.

### 3.3 Temporal Determinism (Padding Rule)
To preserve the deterministic timing guarantees of Causm, the `lease` block always advances the `local_clock` by exactly the specified `<duration>`.
- If the logic within the block executes in `T < duration`, the Register-based Temporal Virtual Machine (TVM) injects `duration - T` milliseconds of padding.
- If the static analyzer determines that the Worst-Case Execution Time (WCET) of the block exceeds the `<duration>`, it will emit a `Lease Duration Exceeded` error.

### 3.4 Control Flow Restrictions
To ensure the atomicity of the temporal lock, the following control flow operations are prohibited within a `lease` block:
- `break`
- `return`
- `split` / `merge`

### 3.5 Structural Decay
If a specific field of a leased structure is accessed (e.g., `let x = view.field`), the binding `view` remains valid, but the system tracks that the parent structure is currently borrowed. Unlike normal structures, accessing a field within a lease does **not** trigger destructive decay of the parent, as the entire structure is already "locked" for the duration.

---

## 4. Implementation Details

- **Memory Arena**: The `Arena` tracks leases as a specialized `EntropicState::Leased` variant, storing the original state and the expiration clock coordinate.
- **TVM Execution**: The TVM records the `local_clock` upon entry and enforces the padding or fault logic upon exit.
