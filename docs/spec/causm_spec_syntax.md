# Causm Formal Syntax Reference

This document provides the formal specification of the Causm syntax. Causm is a research-oriented language designed for deterministic, time-aware concurrency utilizing an entropic memory model.

---

## 1. Program Structure

An Causm program is composed of one or more **Timeline Blocks**.

### Timeline Blocks
A timeline block defines an execution context at a specified temporal coordinate.

**Syntax:**
```causm
@<time_coordinate>: <statement_block>
```

**Temporal Coordinates:**
- **Absolute Temporal Marker**: `@0ms:`, `@100ms:` (Time relative to the root timeline initialization).
- **Relative Temporal Offset**: `@+10ms:` (Temporal offset relative to the current timeline entry point).
- **Branch Identifier**: `@branch_name:` (Execution within a designated named branch).

**Example:**
```causm
@0ms: {
  split main into [worker]
  @worker: {
    let x = "system_initialization"
  }
}
```

---

## 2. Declarations and Expressions

### Variable Initialization (`let`)
Initializes a new binding within the entropic arena of the current execution branch.

**Syntax:**
```causm
let <identifier> = <expression>
```

**Entropic Implications:**
- If the `<expression>` evaluates to a variable, the value is **consumed** (moved) from its source, rendering it unavailable unless an explicit `clone()` operation is performed.
- If the `<expression>` is a literal, it is allocated within the local memory arena.

### Type Specifications (`type`)
Defines structured data types with advanced temporal and entropic constraints.

**Syntax:**
```causm
type <name> = struct [decay_after <amount>ms] [scoped(@<branch>)] {
    <field_list>
}
```

**Advanced Primitives:**
- **Automated Decay (`decay_after`)**: The instance automatically transitions to the `Decayed` state when its age (measured from instantiation) exceeds the specified duration.
- **Timeline Scoping (`scoped`)**: Restricts the type to a specific execution branch. Unauthorized movement or instantiation across branch boundaries results in a `Timeline Violation`.

**Example:**
```causm
type SessionToken = struct decay_after 50ms {
    id: int
}
```

### Entropic Transition Handlers (`decay_handler`)
Specifies a logic block that executes automatically prior to the decay transition of a variable of a designated type.

**Syntax:**
```causm
decay_handler for <type_name> {
    <statements>
}
```

---

## 3. Control Flow Primitives

### Temporal Assertions (`assert_time`)
Enforces strict temporal constraints during execution.

**Syntax:**
```causm
assert_time(elapsed <relop> <amount>ms) [else <statement_block>]
```

**Semantics:**
- **Static Analysis**: The analyzer calculates the Worst-Case Execution Time (WCET). If the limit is statically exceeded, a `Temporal Assertion Violation` occurs.
- **Dynamic Verification**: The Register-based Temporal Virtual Machine (TVM) verifies the local clock during runtime. If the assertion fails, the `else` block is executed; otherwise, a runtime fault is triggered.

### Conditional Execution (`if`)
Facilitates speculative path evaluation followed by deterministic state reconciliation.

**Syntax:**
```causm
if (<expression>) <statement_block> [else <statement_block>] [reconcile (<resolution_rules> | auto)]
```

**Semantics:**
- Both execution paths undergo speculative analysis for entropic consistency.
- `reconcile` rules define the resolution mechanism for variables consumed within a single path. The `auto` keyword automatically merges decayed states and enforces type consistency.

### Speculative Execution (`speculate`)
Creates a transient micro-timeline for trial computations with guaranteed zero-leakage rollback.

**Syntax:**
```causm
speculate (max <amount>ms) {
    <statements>
    commit { <statements> }
} [fallback { <statements> }]
```

**Semantics:**
- **Rollback**: Upon failure (due to `collapse` or timeout), the system state is restored to the pre-speculation configuration.
- **Commit**: Upon successful completion, explicitly tagged variables (Selective mode) or the comprehensive state (Full mode) are merged into the parent timeline.

### Paced Iteration (`for`)
Iterates over collections subject to strict temporal and entropic constraints.

**Syntax:**
```causm
for <item> <mode> <source> [pacing <amount>ms] [(max <amount>ms)] { <statements> }
```

**Modes:**
- `consume`: The source is destroyed; items are moved into the loop scope.
- `clone`: The source remains valid; items are replicated into the loop scope.

**Pacing Mechanism:**
- Ensures each iteration occupies an exact duration (`Nms`). Temporal overruns trigger a `WatchdogBite`.

### Parallel Mapping (`split_map`)
A scatter-gather construct that initializes independent timelines for each element within a collection.

**Syntax:**
```causm
split_map <item> <mode> <source> { <statements> } reconcile (<resolution_rules>)
```

### Temporal Leases (`lease`)
Provides transient, read-only access to an entropic structure for a fixed duration.

**Syntax:**
```causm
lease <binding> = <source> for <amount>ms { <statements> }
```

**See also**: [Temporal Leases Specification](./causm_spec_leases.md)

---

## 4. Routines and Execution Contracts

### Routine Specification (`routine`)
Defines a procedure governed by a deterministic execution contract.

**Syntax:**
```causm
routine <name>(<params>) taking (<amount>ms | _) { <statements> }
```

**Parameter Passing Modes:**
- `consume`: The argument is moved into the routine's scope.
- `clone`: The argument is replicated.
- `peek`: Read-only access is granted; the caller's state remains unaffected.
- `decay`: The caller's value transitions to the `Decayed` state following the call.

---

## 5. Timeline Management

### Branching Operations (`split` / `merge`)
- **`split <parent> into [<branches>]`**: Initializes isolated child timelines.
- **`merge [<branches>] into <target> [resolving (<rules>)]`**: Recombines branch states into a target timeline.

### Resets and Anchors
- **`anchor <name>`**: Snapshots the current state of the timeline, including the local clock and memory arena.
- **`rewind_to(<name>)`**: Restores the timeline to a designated anchor point.
- **`watchdog <target> timeout <amount>ms [recovery <block>]`**: Monitors a specific branch and executes recovery logic upon timeout.
- **`reset <branch> to <anchor>`**: Facilitates an acausal reset at the implementation level.

### Entropic Entanglement
Synchronizes the entropic states of variables across isolated timelines with zero-tick latency.

**Syntax:**
```causm
entangle(<variable_list>)
```

**Semantics:**
- Variables within the entanglement group share a unified entropic state.
- The consumption or decay of one variable in any branch causes an immediate state transition for all entangled variables across all branches.
- Entanglement must be established in the parent timeline prior to the `split` operation.

---

## 6. Communication Channels and Concurrency

### Communication Primitives
- **`open_chan <name>(<capacity>)`**: Initializes a buffered communication channel. (Requires `Chan.Manage` if inside an `isolate`).
- **`chan_send <chan>(<value>)`**: Moves a value into the designated channel buffer. (Requires `Chan.Outbound` for the specific ID or `id="*"`).
- **`chan_recv(<chan>)`**: Extracts a value from the channel. (Requires `Chan.Inbound` for the specific ID or `id="*"`).

### Isochronous Slicing
- **`slice <amount>ms`**: Establishes a fixed-duration execution slice for the active isolate.
- **`loop tick { <statements> }`**: Executes logic within a single slice, incorporating deterministic padding and channel buffer commits.

---

## 7. Diagnostics and Capability Manifests

### Observability
- **`print(<expression>)`**: Consumptive output (utilizing standard entropic evaluation).
- **`debug(<expression>)`** / **`log(<expression>)`**: Non-consumptive inspection (peek) of a value.

### Capability Manifests (`isolate`)
Sandboxes an execution block with specific resource requirements and functional capabilities.

**Syntax:**
```causm
isolate [<identifier>] {
    [enable <resource>(<amount>)]
    [require <capability>(<params>)]
    [slice <amount>ms]
    <statements>
}
```

---

## 8. Low-Level System Statements
- **`network_request <url>`**: Triggers a simulated network effect with a deterministic cost of 5ms.
- **`collapse`**: Terminates the current speculative block immediately.

---

## 9. Operators and Expressions

### Arithmetic Operators
- `+` : Addition or **String Concatenation**.
- `-` : Subtraction or Unary Negation.
- `*` : Multiplication.
- `/` : Division.
- `%` : Modulo.
- `^` : Exponentiation (Power).

### Logical and Relational Operators
- `!` : Logical NOT (Unary).
- `==`, `!=` : Equality and Inequality.
- `<`, `>`, `<=`, `>=` : Comparison.

### Ergonomic String Concatenation
The `+` operator supports automatic coercion to string when at least one operand is a string literal or string-typed variable.
**Example:**
```causm
let msg = "Balance: " + 1000 // Results in "Balance: 1000"
```

### Index Access
Topologies and arrays support dynamic indexing.
**Syntax:**
```causm
let val = source[index_expression]
```
- For topologies, the index must evaluate to a **String** (field name).
- For arrays, the index must evaluate to an **Integer**.
