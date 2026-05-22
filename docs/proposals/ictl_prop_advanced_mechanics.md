# Proposal: Advanced Causal Mechanics and Temporal Refinements

This document specifies a suite of high-level enhancements for the Isolate Concurrent Temporal Language (ICTL), focusing on cross-timeline synchronization, type-level temporal enforcement, and acausal diagnostic protocols.

---

## 1. Entropic Channels: Deterministic Mid-Timeline Communication

Currently, ICTL timelines primarily exchange state during formal `merge` operations. **Entropic Channels** facilitate mid-execution communication between split timelines while maintaining strict single-ownership and temporal determinism.

### 1.1 Lock-Free Entropic Transfer
Communication via an Entropic Channel is defined as an **Acyclic State Transfer**. 
- **Transmission**: Invoking `chan_send` on a variable transitions it to the `Consumed` state in the sender's arena.
- **Reception**: The variable is initialized as `Valid` in the receiver's arena at a deterministic temporal offset.
- **Synchronization**: To prevent race conditions, the receiver must utilize an `await_chan` primitive, which suspends the local clock until the message is available, aligning with the sender's global temporal coordinate.

### 1.2 Channel Manifests
Channels must be declared in the `isolate` manifest, specifying the expected entropic type and the maximum temporal latency allowed for transmission.

```ictl
isolate Worker {
    require Chan.Inbound(id="data_pipe", type=struct, latency=5ms)
}
```

---

## 2. Temporal Type Contracts: Type-Level WCET Enforcement

To move beyond manual temporal accounting, **Temporal Type Contracts** embed execution constraints directly into the type system.

### 2.1 Bound Traits
Data structures can implement "Bound Traits" that define the Worst-Case Execution Time (WCET) for standard operations (access, iteration, transformation).
- **`PacedIterable<T, MaxTime>`**: A collection type that guarantees iteration over any member will not exceed `MaxTime`.
- **`ConstantAccess<T, Time>`**: Guarantees O(1) temporal cost for field access.

### 2.2 Compile-Time Verification
The static analyzer verifies that any routine consuming a contract-bound type does not violate the specified temporal invariants. This allows the compiler to reject code that might cause non-deterministic jitter before it ever reaches the STVM.

```ictl
// The compiler enforces that this routine never exceeds 10ms 
// because it only operates on PacedIterable types.
routine process_batch(data: PacedIterable<LogEntry, 2ms>) -> Result {
    foreach(entry in data) {
        process(entry)
    }
}
```

---

## 3. Acausal Debugging Protocols: Time-Travel Diagnostics

Leveraging the STVM's `anchor` and `reset` primitives, the **Acausal Debugger** provides a mechanism for stepping backward through the execution history and visualizing the entropic decay graph.

### 3.1 Trace Retention and Replay
In Debug Mode, the VM maintains a **Causal Trace Log**—a compressed history of arena snapshots and temporal transitions. 
- **`step_back`**: Restores the VM to the previous temporal anchor or instruction boundary.
- **`visualize_decay`**: Generates a graph showing which operations triggered structural decay across the timeline's history.

### 3.2 Paradox Prevention
To ensure debugging does not introduce acausal paradoxes, the debugger operates in a **Read-Only Observation State**. While the debugger can "rewind" the VM state to inspect variables, it cannot modify the state and then "resume" execution if a `commit` boundary has been crossed, preserving the integrity of external side-effects.

```bash
# Example Debugger CLI Command
ictl-debug --trace examples/sample.ictl --at 50ms --show-entropy
```

---

### Architectural Significance
These refinements collectively strengthen the ICTL ecosystem. Entropic Channels enable complex concurrent coordination; Temporal Type Contracts shift temporal safety from a runtime concern to a compile-time guarantee; and Acausal Debugging transforms the language's unique reset mechanics into a powerful developmental tool.