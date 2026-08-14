# Proposal: Causm Advanced Loop Constructs (Entropic, Event-Driven, and Timed Iterations)

**Status:** Approved & Implemented  
**Author:** Seuriin <seuriin@gmail.com>  
**Category:** Control Flow & Iteration  

---

This document specifies the design requirements and execution semantics for advanced loop constructs in the Causm programming language. These structures enhance deterministic temporal execution, entropic safety, and developer ergonomics.

---

## 1. Entropic Condition Loop (`while valid`)

In Causm, variables transition between states (`Valid`, `Leased`, `Decayed`, `Consumed`). Currently, checking if a variable is still usable during iteration requires verbose manual matching. The `while valid` loop automates this.

### Grammar & Syntax
```causm
// Executes only as long as telemetry remains in the Valid state
while valid (telemetry) (max 100ms) {
    let reading = telemetry.value
    print(reading)
}
```

### Entropic & Safety Semantics
*   **Automatic Decay Check**: The loop condition implicitly evaluates `entropy(telemetry) == Valid` at the start of each iteration.
*   **Safe Exit**: If the variable decays (e.g. its temporal lease expires) or is consumed by an operation inside the body, the loop exits gracefully at the iteration boundary without panicking.
*   **Temporal Budget**: A `max` temporal budget constraint is mandatory to enforce WCET boundaries.

---

## 2. Event-Driven Ticking Loops (`loop tick on`)

While `loop tick` provides isochronous execution tied to a fixed timeframe slice, many real-time systems react to external events. The `loop tick on` construct binds the start of an isochronous tick to data arrival events.

### Grammar & Syntax
```causm
// Ticks isochronously upon packet arrival on sensor_bus
loop tick on sensor_bus {
    let packet = chan_recv(sensor_bus)
    process(packet)
}
```

### Entropic & Safety Semantics
*   **Event-Triggered Ticking**: The timeline blocks/awaits the channel until a message arrives. Once a message is received, a new tick boundary starts with the channel's message lease timeline.
*   **Commit Phase**: State modifications made inside the loop body are buffered and committed at the conclusion of the event tick.
*   **Watchdog Constraints**: If processing the received event exceeds the active slice budget starting from the event timestamp, a watchdog bite is triggered.

---

## 3. Bounded Conditional Loops (`while`)

To improve readability over manual `loop` blocks with nested `if/break` logic, Causm introduces a standard conditional `while` loop, protected by mandatory execution bounds.

### Grammar & Syntax
```causm
let count = 0
while (count < 10) (max 50ms) {
    count = count + 1
}
```

### Entropic & Safety Semantics
*   **Mandatory WCET Boundary**: Every `while` loop must specify a `max` temporal budget.
*   **Static Z3 Unrolling**: The verifier unrolls the loop body twice to prove type and entropic safety invariants. The total budget `max_ms` is added to the output clock coordinate to ensure downstream timing determinism.

---

## 4. Sliding-Window Timed Iteration (`for ... step`)

For workloads where data elements must be processed at precise temporal intervals rather than as fast as possible.

### Grammar & Syntax
```causm
let dataset = [1, 2, 3]

// Emits 1 at 0ms, 2 at 10ms, 3 at 20ms...
for item in dataset step 10ms {
    transmit(item)
}
```

### Entropic & Safety Semantics
*   **Implicit Pacing**: Equivalent to `pacing 10ms` per iteration, ensuring the `local_clock` is padded precisely to `10ms` multiples.
*   **Safety Assertions**: The verifier statically ensures that the worst-case execution time of `transmit(item)` is less than the `step` duration.

---

## Loop Constructs Comparison Matrix

| Construct | Trigger Condition | Termination Boundary | Timing Behavior |
| :--- | :--- | :--- | :--- |
| **`while valid`** | Variable state is `Valid` | Loop budget (`max`) or state change | Clock padded to `max` on completion |
| **`loop tick on`** | Message arrival on channel | Explicit `break` | Isochronous window per event |
| **`while`** | Boolean expression | Explicit condition or loop budget (`max`) | Clock padded to `max` on completion |
| **`for ... step`** | Collection iteration | End of collection | Clock padded to `step` interval per item |
