# Developer Guide: Acausal Debugging Protocols

## 1. Overview
Leveraging the Register-based TVM's `anchor` and `reset` primitives, the **Acausal Debugger** provides a unique environment for "time-travel" diagnostics. It allows developers to step backward through execution history and visualize the entropic decay graph of the memory arena.

## 2. Trace Retention
In debug mode (invoked via `--trace-causal`), the TVM maintains a **Causal Trace Log**—a compressed history of arena snapshots and temporal transitions.

### 2.1 Instruction Boundaries
Every instruction execution creates a trace entry containing:
- **Local Clock**: The relative temporal offset.
- **Arena Delta**: Changes to the entropic state of registers.
- **Path Condition**: The symbolic constraints active at the time of execution.

## 3. Backward Stepping
The debugger supports restoring the VM state to previous coordinates.

### 3.1 `step_back`
Restores the VM to the state immediately preceding the current instruction or temporal anchor.
- **Integrity**: Backward stepping is a read-only operation. The debugger preserves the original side effects (like `chan_send`) to prevent causal paradoxes during inspection.

## 4. Visualizing Entropic Decay
The CLI and LSP provide real-time visualization of how variables transition from `Valid` to `Consumed`.

### 4.1 Decay Graph
The decay graph displays the causal dependencies between variables.
- **Structural Propagation**: Shows how consuming a field (e.g., `p.x`) caused the parent structure (`p`) to transition to the `Decayed` state.
- **Entanglement Links**: Highlights variables that decayed simultaneously due to an `entangle` relationship.

## 5. CLI Usage

The `causm` CLI uses subcommands. Diagnostic and tracing flags are available under `causm run`:

```bash
# Execute with full causal history tracing
causm run --trace-causal examples/sample.csm

# Execute with verbose output (arena tables, clock metrics, WCET bounds)
causm run -v examples/sample.csm

# Combine: verbose + causal tracing
causm run -v --trace-causal examples/sample.csm

# Formal verification only (no execution)
causm check examples/sample.csm
```

> [!NOTE]
> The legacy flag `--dump-causal-history` is still accepted as a backwards-compatible alias for `--trace-causal`.

## 6. Paradox Detection in Debugging
While the debugger allows "rewinding" the local state, it strcausmy enforces the **Causal Horizon**. A developer cannot modify state and resume execution if a causal commitment (commitment to external systems) has already occurred past the target anchor point.
