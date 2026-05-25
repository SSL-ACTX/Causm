# Specification: Isochronous Scheduling and Relativistic Execution

## 1. Introduction
Causm ensures systemic timeline determinism through **Isochronous Scheduling**. This mechanism guarantees that the `global_clock` advances in fixed, predictable increments, regardless of the logical complexity of individual execution paths.

## 2. Relativistic Contexts
An execution timeline can define a **Relativistic Context** via the `isolate` manifest.

### 2.1 `slice_ms` (Temporal Granularity)
The `slice_ms` defines the fundamental temporal unit for a timeline. Every execution cycle (or "tick") on this timeline is padded by the TVM to exactly match this duration.
- **Independence**: `slice_ms` is decoupled from `cpu_budget_ms`. A timeline may possess a 10ms slice but a 1000ms total budget.

### 2.2 `cpu_budget_ms` (Total Execution)
Defines the cumulative allowed `local_clock` time for the entire isolate lifecycle.

## 3. Tick Loops (`loop tick`)
The `loop tick` construct is the primary mechanism for periodic execution.

```causm
isolate Sensor {
    enable slice(10ms)
    
    loop tick {
        let packet = chan_recv(bus)
        process(packet)
        // Deterministic Padding happens here
    }
}
```

### 3.1 Deterministic Padding
At the end of every `loop tick` iteration (including paths where a `break` is executed), the TVM calculates the remaining time in the current slice:
`Padding = slice_ms - iteration_cost`
The TVM then stalls the `local_clock` by the `Padding` duration before initiating the next cycle.

### 3.2 Violation Detection
The Z3 Correctness Kernel proves that for any path `P` through the loop body:
`Path_Taken(P) => Body_Cost(P) <= slice_ms`
If a path exceeds the slice duration, the program is rejected to prevent "temporal drift."

## 4. Temporal Equalization
Branching constructs (`if/else`, `select`) within an isochronous context are subject to **Temporal Equalization**. The TVM identifies the path with the maximum WCET and pads all shorter paths to match, ensuring that the timeline exit time is invariant relative to the selected branch.
