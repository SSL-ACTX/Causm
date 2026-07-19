# Proposal: Causm Advanced Data Structures (ADTs, History Buffers, and Tuples)

This document specifies the design requirements and formal semantics for advanced data representation constructs within the Causm programming language, expanding capabilities beyond standard structs to support polymorphic states, causal timelines, and developer ergonomics.

---

## 1. Tagged Algebraic Enums (ADTs)

Conventional unions in Causm lack explicit tag structures, making complex state machine representations difficult. Tagged Enums (Algebraic Data Types) address this by coupling type safety with pattern matching.

### Grammar & Syntax
```causm
enum SystemState {
    Active,
    Calibrating(float),
    Fault { code: int, description: string }
}
```

### Entropic & Safety Semantics
*   **Ownership Rules**: Enums carry entropic ownership. Constructing an enum variant with a resource payload (e.g., a struct) consumes the payload.
*   **Pattern Matching Integration**: Match arms must be exhaustive. Match bindings inherit the payload's entropic state (Valid/Decayed/Consumed).
*   **Reconciliation**: Divergent enum variants across branching paths must specify reconciliation rules (e.g., `reconcile(first_wins)`).

---

## 2. Causal Time-Series History Buffers (`history<T>`)

Causm timelines often require querying values at past clock timestamps. Currently, this requires physically rewinding the system execution path. History Buffers solve this by storing a local, queryable window of values indexed by logical timeline offsets.

### Grammar & Syntax
```causm
// Declares a history log of integers keeping the last 100ms of data
let telemetry = history<int>(max_depth 100ms)

// Appends a data point, associated with the current timeline's local_clock
telemetry.push(42)

// Evaluates the value of the variable at a specific past logical timestamp
let past_val = telemetry.at(10ms)
```

### Entropic & Safety Semantics
*   **Temporal Boundaries**: Attempting to query a timestamp older than the history's `max_depth` or newer than the current `local_clock` triggers a `TemporalError`.
*   **Non-Consuming Reads**: Calling `.at(timestamp)` is a non-consuming query, returning a copy (for primitives) or a leased view (for resource types) of the historical state.

---

## 3. Lightweight Tuples

To avoid the overhead of declaring full schemas for simple structures (like returning multiple values from a function or routine), Causm introduces unnamed tuples.

### Grammar & Syntax
```causm
routine measure() -> (float, string) {
    yield (98.6, "fahrenheit")
}

let (temp, unit) = measure()
```

### Entropic & Safety Semantics
*   **Destructuring**: Tuples are destructured via pattern assignment. Destructuring a tuple consumes the tuple itself and moves its elements.
*   **Elements Ownership**: Each tuple field maintains its own entropic state. Consuming one element of a tuple does not automatically consume others, preventing unnecessary copies.

---

## Comparative Matrix of Data Structures

| Construct | Structural Definition | Temporal Dimension | Primary Use Case |
| :--- | :--- | :--- | :--- |
| **`struct`** | Named product type | Static (decay limits) | Domain models and configuration states |
| **`enum`** | Sum type (tagged payload) | Static | Polymorphic state machines and events |
| **`history<T>`** | Sliding-window buffer | Dynamic (logical history) | Local telemetry tracking without rewind |
| **`tuple`** | Anonymous product type | Static | Lightweight grouping and multi-returns |
