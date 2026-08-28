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

### Temporal Block Markers and Directives (`@`)

The `@` symbol is utilized across four primary language constructs:

1. **Temporal Coordinate Markers**:
   - Absolute Markers: `@0ms:`, `@100ms:` (Establishes execution at a fixed global clock coordinate).
   - Relative Offsets: `@+10ms:` (Advances the local branch clock by a relative duration).
   - Branch Identifiers: `@worker:` (Scopes execution within a specific timeline branch).

2. **Entropic Lifetime Annotations**:
   - Explicit Decay Duration: `let @decayed(50ms) token = "xyz"` (Triggers entropic state shift to `Decayed` after 50ms).
   - Dynamic Decay Rate: `let @decay_rate(20ms) telemetry = 100` (Enforces per-tick entropic decay rate).

3. **Execution Block Directives**:
   - `@chaos` / `directive chaos`: Overrides safety constraints to simulate non-deterministic environment jitter.
   - `@deterministic` / `directive deterministic`: Enforces strict WCET bounds and deterministic state reconciliation.

---

## 2. Declarations and Expressions

#### Variable Initialization (`let`)
Initializes a new binding within the entropic arena of the current execution branch. Supports optional lifetime decay annotations and uninitialized definite assignment tracking.

**Syntax:**
```causm
let [annotation] <identifier> [= <expression>]
```

**Lifetime Annotations & Uninitialized Let:**
- `let @decayed(50ms) token = "xyz"`: Specifies explicit lifetime decay duration.
- `let @decay_rate(20ms) sensor_stream = 100`: Specifies entropic decay rate per time unit.
- `let uninit_var`: Uninitialized binding subject to compile-time definite assignment verification.

**Entropic Implications:**
- If the `<expression>` evaluates to a variable, the value is **consumed** (moved) from its source, rendering it unavailable unless an explicit `clone()` operation is performed.
- If the `<expression>` is a literal, it is allocated within the local memory arena.

### Entropic Transition Handlers (`on_decay` / `decay_handler`)
Specifies a logic block that executes automatically upon entropic state decay.

**Syntax:**
```causm
on_decay(<variable_name>) {
    <statements>
}

decay_handler for <type_name> {
    <statements>
}
```

### Type Specifications (`type`)
Defines structured data types with advanced temporal and entropic constraints.

**Syntax:**
```causm
type <name> = [<base_type> +] struct [decay_after <amount>ms] [scoped(@<branch>)] {
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

### Interface Specifications (`interface`)
Defines abstract behavior contracts and default method implementations.

**Syntax:**
```causm
interface <name> [[= <base_interface>] + interface] {
    [routine <method_name>(<params>) -> <return_type> taking <amount>ms [where <constraints>] [{ <body> }]]
    ...
}
```

---

## 3. Control Flow & Execution Primitives

### Try Unwrap Operator (`?`)
Unwraps a fallible entropic payload or error result.

**Syntax:**
```causm
let result = <expression>?
```

### Paced Time-Step Loops (`for ... step`)
Iterates over numeric ranges with fixed temporal step pacing.

**Syntax:**
```causm
for <step_var> in <start>..<end> step <amount>ms {
    <statements>
}
```

### Match Entropy & Pattern Guards
Inspects variable entropic state with optional conditional guards.

**Syntax:**
```causm
match entropy(<target>) {
    Valid(<pattern>) [if <guard_condition>]: { <statements> }
    Decayed(<pattern>) [if <guard_condition>]: { <statements> }
    Pending(<pattern>) [if <guard_condition>]: { <statements> }
    Consumed [if <guard_condition>]: { <statements> }
}
```

### Temporal Assertions (`assert_time`)
Enforces strict temporal constraints during execution.

**Syntax:**
```causm
assert_time(elapsed <relop> <amount>ms) [else <statement_block>]
```

**Semantics:**
- **Static Analysis**: The analyzer calculates the Worst-Case Execution Time (WCET). If the limit is statically exceeded, a `Temporal Assertion Violation` occurs.
- **Dynamic Verification**: The Register-based Temporal Virtual Machine (TVM) verifies the local clock during runtime. If the assertion fails, the `else` block is executed; otherwise, a runtime fault is triggered.

### Conditional Execution (`if` / `if let`)
Facilitates speculative path evaluation followed by deterministic state reconciliation, or dynamic downcasting of interfaces.

**Syntax:**
```causm
if (<expression>) <statement_block> [else <statement_block>] [reconcile (<resolution_rules> | auto)]
if let <binding> = <expression>.(<type_name>) <statement_block> [else <statement_block>]
```

**Semantics:**
- Both execution paths undergo speculative analysis for entropic consistency.
- `reconcile` rules define the resolution mechanism for variables consumed within a single path. The `auto` keyword automatically merges decayed states and enforces type consistency.
- `if let` attempts to downcast an interface variable to a concrete type `type_name`. If successful, the `then` block executes with `binding` bound as the concrete type.

### Non-Consuming Borrow (`&` / `peek`)
Provides read-only access to struct fields without triggering entropic decay:

```causm
let ref_reading = &sensor_pack
let active_level = ref_reading.level
```

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

### Temporal Leases (`lease ... reconcile`)
Provides transient, read-only access for a fixed duration with optional inline timeline state reconciliation.

**Syntax:**
```causm
lease <binding> = <source> <amount>ms {
    <statements>
} [reconcile (auto | <rules>)]
```

### Block Directives (`directive`)
Overrides execution entropy mode within scoped blocks.

**Syntax:**
```causm
directive (chaos | deterministic) {
    <statements>
}
```

**See also**: [Temporal Leases Specification](./causm_spec_leases.md)

---

## 4. Routines and Execution Contracts

### Routine Specification (`routine`)
Defines a procedure governed by a deterministic execution contract.

**Syntax:**
```causm
routine <name>(<params>) [-> <return_type>] taking (<amount>ms | _ | ?) [where <state_constraints>] { <statements> }
```

**Temporal Contract Forms:**
- `taking <amount>ms`: Explicit fixed Worst-Case Execution Time (WCET) budget.
- `taking _`: Inferred contract automatically computed by the static compiler analyzer across all code paths.
- `taking ?`: Dynamic wildcard contract empirically synthesized by `causm tune` fuzzing sweeps.

**Parameter Passing Modes:**
- `consume`: The argument is moved into the routine's scope.
- `clone`: The argument is replicated.
- `peek`: Read-only access is granted; the caller's state remains unaffected.
- `decay`: The caller's value transitions to the `Decayed` state following the call.
- `lease`: Leased for the duration of the method invocation.

---

## 5. Timeline Management

### Branching Operations (`split` / `merge`)
- **`split <parent> into [<branches>]`**: Initializes isolated child timelines.
- **`merge [<branches>] into <target> [reconcile (<rules>)]`**: Recombines branch states into a target timeline.

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

### Reference Operator Shorthand (`&`)
Syntactic sugar for non-consuming `peek` access of a variable within expressions.

**Syntax:**
```causm
let ref_val = &variable_name
```

---

## 10. Module Import Declarations

Supports multi-file code organization across `.csm` files.

**Syntax:**
```causm
// Module alias import
import "path/file.csm" as Alias

**See also**: [Module System Specification](./causm_spec_modules.md)

---

## 11. Foreign Blocks & System Calls (`foreign`, `syscall`)

Enables native C ABI foreign function declarations and low-level kernel system call invocations with capability sandbox enforcement and explicit duration bounds.

**Syntax:**
```causm
foreign "libc.so.6" abi("C") {
    pub routine getpid() -> i32 taking 1ms
}

routine sys_write(peek msg: string) -> i64 taking 2ms {
    require System.Syscall
    let result = syscall("sys_write", 1, msg, 12) taking 2ms
    yield result
}
```

---

## 12. Developer Tooling CLI (`causm tune`, `causm profile`, `causm fmt`)

The Causm devtools suite (`crates/causm-devtools`) provides built-in tools for formatting, profiling, and self-calibrating temporal contracts.

- **`causm fmt [files...]`**: Formats Causm code using AST parsing rules with a two-tier round-trip and entropic semantic validation gate.
- **`causm profile <file.csm>`**: Profiles TVM memory watermarks, logical global/root clocks, and timeline branch lifetimes.
- **`causm tune [files...]`**: Empirically benchmarks routines via chaos fuzzing and updates `taking ?` or existing contracts with statistical $P_{99.9}$ safety margins:
  - `causm tune <file> --all` (`-a`): Continuously re-tunes all contracts in the file.
  - `causm tune <file> --routine <name>` (`-r`): Pinpoints an individual routine for recalibration.
  - `causm tune <file> --dry-run`: Previews suggested temporal contract changes without modifying files.

---

## 13. Compiler Attributes & Annotations (`@`)

Causm supports fine-grained compile-time attributes attached to statements, types, and isolates.

**Syntax:**
```causm
@<attribute_name>[(<arg1>, <arg2>, ...)]
<statement>
```

**Standard & Extensible Attributes:**
- `@derive(Clone, Debug, PartialEq)`: Derives trait implementations for struct and enum declarations.
- `@must_use("Reason message")`: Emits compile-time diagnostics if a returned value is discarded.
- `@inline`: Directs IR lowering to inline the targeted routine definition.
- `@test`: Marks routine definitions as executable integration test targets.
- `@seccomp("sys_read", "sys_write", "sys_exit")`: Custom plugin attribute restricting permitted kernel syscalls within an `isolate` sandbox block.

---

## 14. Compiler Plugins & WebAssembly SDK (`causm-plugin-sdk`)

Causm provides a high-performance, hermetic plugin architecture enabling custom AST transformations, static linters, and verification passes via WebAssembly (WASMI 2.0) or Stdio IPC.

### Project Manifest Configuration (`causm.toml`)
Plugins are discovered declaratively from `causm.toml` in project workspaces:

```toml
[package]
name = "my_system"
version = "0.1.0"

[plugins.seccomp_guard]
path = "plugins/seccomp_guard.wasm"

[plugins.seccomp_guard.options]
allow = "sys_read,sys_write,sys_exit"
strict = true
```

### Developing WASM Plugins with `causm-plugin-sdk`
Plugins compile to `wasm32-unknown-unknown` utilizing the `#[causm_plugin]` macro:

```rust
use causm_plugin_sdk::prelude::*;

#[causm_plugin(name = "custom_linter", version = "0.1.0")]
pub fn process_ast(program: Program, ctx: &PluginContext) -> Result<Program, PluginError> {
    // Inspect or mutate AST program nodes
    Ok(program)
}
```
