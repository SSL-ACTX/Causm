# Proposal: Continuous Systems, Isochronous Periodic Epochs, and Partitioned Arena Memory

**Status:** Proposed  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Core Language Architecture, Temporal Scheduling & Memory Model  
**Target Crates:** `causm-runtime`, `causm-analysis`, `causm-frontend`, `causm-ir`, `causm-stdlib`

---

## 1. Executive Summary

Causm was originally conceived with finite, discrete timeline anchors (`@0ms:`, `@50ms:`) and decaying entropic data structures (`decay_after 1000ms`). While this paradigm is ideal for one-shot batch computations, signal bursts, and synchronous pipelines, **it creates a fundamental architectural crisis for long-running systems**:
1. **The Temporal Horizon Problem:** Infinite programs (such as HTTP web servers, drone autopilots, telemetry daemons, and database engines) cannot be expressed cleanly with fixed static timestamps without reaching an arbitrary end-of-time.
2. **The State Evaporation Problem:** In an infinite execution lifecycle, persistent handles (listening sockets, routing tables, connection pools, and statistics) must never decay, whereas per-request buffers must decay aggressively to prevent unbounded memory growth.
3. **The Arena Saturation Problem:** Standard linear bump allocators inevitably exhaust their finite arena ceiling ($64\text{ KB} \to 1\text{ MB}$) if allocations are performed continuously over hours or days.

This proposal establishes the formal architecture for **Continuous Systems in Causm**. We introduce:
* **Isochronous Periodic Anchors (`@every <interval>:`)** for deterministic, recurring timeline ticks.
* **Persistent Recurrent State (`state`)** vs. **Transient Ephemeral Memory (`let`)**.
* **Dual-Partitioned Epoch Arenas** providing $O(1)$ instantaneous memory reclamation between ticks with zero fragmentation.
* **Declarative Saturation Policies (`policy on_full`)** for deterministic backpressure and buffer overflow control.
* **Inductive Z3 Temporal Invariance Proofs** proving infinite execution safety across $[0, \infty)$.

---

## 2. Temporal Execution Paradigms for Continuous Programs

```
Timeline Progression:
───[ Epoch 0 ]───►───[ Epoch 1 ]───►───[ Epoch 2 ]───►───[ Epoch 3 ]───► ... [ Epoch ∞ ]
   (t = 0..16ms)      (t = 16..32ms)     (t = 32..48ms)     (t = 48..64ms)
   ├─ Process Sensor  ├─ Process Sensor  ├─ Process Sensor  ├─ Process Sensor
   ├─ Compute PID     ├─ Compute PID     ├─ Compute PID     ├─ Compute PID
   └─ Arena Reset ⟲   └─ Arena Reset ⟲   └─ Arena Reset ⟲   └─ Arena Reset ⟲
```

### 2.1 Periodic Isochronous Anchors (`@every`)

Routines or blocks declared with `@every <interval>` represent clock-synchronized periodic cycles. The TVM scheduler guarantees that each cycle is initiated precisely on the specified modulo interval.

```causm
// 1. High-rate flight control loop (60 Hz = 16.66ms period):
@every 16ms: {
    let sensor_data = Gyro.read()
    let actuator_signal = PID.compute(sensor_data)
    Motor.apply(actuator_signal)
}

// 2. Low-rate system telemetry & health check daemon (1 Hz = 1000ms period):
@every 1000ms: {
    let mem_used = arena.used_bytes()
    let health_packet = Telemetry.build_report(mem_used)
    Network.send_telemetry(health_packet)
}
```

### 2.2 Event-Paced Continuous Loops (`loop on <event>`)

For non-periodic, event-driven server loops, execution suspends until an incoming event (such as a channel packet or network connection) wakes the epoch.

```causm
service HttpDaemon {
    state listener = Net.TcpListener.bind(8080)

    loop on listener {
        using conn = listener.accept() {
            let req = conn.recv(1024)
            let resp = Router.dispatch(req)
            conn.send(resp)
        } // `conn` dropped and transient buffers reclaimed at end of cycle
    }
}
```

---

## 3. The Dual-Partitioned Epoch Arena Memory Model

To support infinite execution lifecycles within fixed memory footprints, each continuous `isolate` or `service` allocates a **Dual-Partitioned Linear Arena**.

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        Causm Isolate Fixed Arena (e.g. 64 KB)                          │
├───────────────────────────────────┬────────────────────────────────────────────────────┤
│    Persistent Partition (Static)  │           Transient Epoch Partition (Dynamic)      │
│    (Sockets, Counters, Tables)    │           (Per-Request Buffers, Scratch Arrays)    │
├───────────────────────────────────┼────────────────────────────────────────────────────┤
│ ▲                                 │ ▲                                                ▲ │
│ 0 KB (Base)                       │ Base Watermark (e.g. 4 KB)         Alloc Watermark │
│                                   │ (RESET TARGET PER TICK)                  (64 KB Max)│
└───────────────────────────────────┴────────────────────────────────────────────────────┘
```

### 3.1 Memory Partition Roles

| Partition | Keyword | Lifecycle | Reclamation Mechanism |
| :--- | :--- | :--- | :--- |
| **Persistent Partition** | `state` | Immortal (Process lifetime) | Explicit destructive `consume` or process exit |
| **Transient Partition** | `let` | Ephemeral (Single Epoch tick) | **Instant $O(1)$ watermark reset at end of tick** |

### 3.2 Epoched Compaction Algorithm (Zero Overhead)

1. **Boot Initialization Phase:**
   * The program executes global/startup initializers.
   * All `state` declarations are bump-allocated sequentially starting at byte `0`.
   * Upon reaching the first `@every` or `loop` boundary, the TVM records the current allocation offset as `BaseWatermark` (e.g., `4,096 bytes`).
2. **Active Epoch Execution Phase:**
   * Inside the tick, all `let` variables and transient arrays are allocated in the upper partition (`BaseWatermark` $\to$ `MaxCapacity`).
3. **Tick Completion & Instant Reset:**
   * When the execution reaches the end of the tick block, the TVM runs `AutoDrop` hooks for active handles in the transient partition.
   * The arena's allocation pointer is instantly snapped back:
     $$\text{ArenaPointer} \leftarrow \text{BaseWatermark}$$
   * **Cost:** Exactly 1 CPU instruction (`str wBase, [xArena, #offset]`). No free-list walking, no pointer tracing, zero fragmentation.

---

## 4. Declarative Saturation & Overflow Policies (`policy`)

In high-throughput continuous pipelines (e.g. video processing, UDP radar streams, telemetry ingestion), incoming data bursts can temporarily exceed the transient arena limit. Causm replaces non-deterministic crashes with **explicit, verified saturation policies**.

```causm
isolate TelemetryIngest {
    enable memory(32KB)
    slice 5ms
    
    // Saturation policy:
    policy on_full = RingBuffer
    policy on_deadline_breach = SkipTick
}
```

### 4.1 Supported Saturation Policies

| Policy | Behavior on Arena Limit | Primary Use Case |
| :--- | :--- | :--- |
| **`EvictDecayed`** *(Default)* | Scans active registers and compacts memory by releasing all `<decayed>` values. | Standard TCP services, REST endpoints, file streaming |
| **`RingBuffer`** | Wraps the allocation pointer back to `BaseWatermark`, overwriting oldest transient data. | High-speed UDP packet capture, continuous flight data recorders |
| **`Throttle`** | Suspends input channels and applies upstream backpressure until next clock tick. | Message queues, asynchronous broker pipelines |
| **`FailFast`** | Immediately aborts the tick, emits `TemporalMemoryBreachError`, and engages safety fallback. | Hard real-time flight avionics, medical life-support systems |

---

## 5. Inductive Z3 Invariance Proofs across $[0, \infty)$

Static analysis must formally prove that an infinite program will never run out of memory or violate temporal constraints at $t = 10\text{ years}$.

```
      [ Base State Invariant P(0) ]  (Proved at boot)
                   │
                   ▼
      [ Inductive Step: ∀k ≥ 0, P(k) ⟹ P(k+1) ]
         ├─ Execution Cost(k) ≤ Interval Period
         ├─ TransientAlloc(k) ≤ (Capacity - BaseWatermark)
         └─ State Invariants Preserved
                   │
                   ▼
      [ Conclusion: Globally Safe ∀t ∈ [0, ∞) ]
```

### 5.1 Formal Inductive Rules

1. **Memory Invariance:**
   $$\forall k \ge 0, \quad \text{Alloc}(k) = \text{BaseWatermark} + \Delta\text{Alloc}(k) \le \text{MaxCapacity}$$
   Because $\Delta\text{Alloc}(k)$ is reset to $0$ at every tick boundary:
   $$\lim_{k \to \infty} \text{Memory}(k) \le \text{MaxCapacity}$$
   *The system is mathematically proven to have zero memory leaks over infinite time.*

2. **Temporal Schedulability Bound:**
   $$\forall k \ge 0, \quad \text{WCET}(\text{Tick}_k) \le \text{IntervalPeriod}$$
   If a single tick's worst-case execution path exceeds the `@every` duration, the compiler rejects the program at compile-time with `SemanticErrorKind::PeriodicDeadlineUnachievable`.

---

## 6. Comprehensive Real-World Specification Example

### 6.1 Real-Time Network Echo Daemon (`examples/continuous_daemon.csm`)

```causm
@0ms: {
    import "std/net" as Net
    import "std/time" as Time
    import "std/encoding/utf8" as Utf8

    isolate continuous_echo_server {
        enable memory(64KB)
        enable cpu(10000ms)
        require System.FFI
        require System.Log

        // Persistent Server State:
        state total_requests: int = 0
        state uptime_start: Time.Instant = Time.now()
        state listener: Net.TcpListener = Net.TcpListener.bind(8080)
        
        policy on_full = EvictDecayed

        print(f"Server initialized on port 8080 at {uptime_start.secs}s")

        // Continuous Isochronous Loop (Ticks every 10ms):
        @every 10ms: {
            // Introspect active arena headroom:
            if arena.remaining() < 8KB {
                print("Warning: Arena under memory pressure, dropping non-critical telemetry")
            }

            // Accept connection if pending (non-blocking):
            if let client = listener.try_accept() {
                total_requests += 1

                // Transient allocation for client request:
                using stream = client {
                    let rx_buf = stream.recv(512)
                    let req_text = Utf8.decode(rx_buf)
                    
                    let response = f"HTTP/1.1 200 OK\r\nContent-Length: 22\r\n\r\nEcho [Req #{total_requests}]: OK"
                    stream.send(Utf8.encode(response))
                }
            }
        } // <-- Transient arena memory automatically wiped back to BaseWatermark here!

        // Background Diagnostic Monitor (Ticks every 5000ms):
        @every 5000ms: {
            let elapsed_s = Time.elapsed_secs(uptime_start)
            let current_mem = arena.used_bytes()
            print(f"[Health Check] Uptime: {elapsed_s}s | Total Requests: {total_requests} | Arena Used: {current_mem}/65536 bytes")
        }
    }
}
```

---

## 7. Pest EBNF Grammar Additions

```pest
// Periodic Anchor & Loop Grammar
every_anchor     = { "@every" ~ duration_expr ~ ":" ~ block }
service_decl     = { "service" ~ identifier ~ "{" ~ service_body* ~ "}" }
loop_on_stmt     = { "loop" ~ "on" ~ expression ~ block }

// State Declaration (Persistent Partition)
state_stmt       = { "state" ~ identifier ~ (":" ~ type_name)? ~ "=" ~ expression }

// Saturation Policy Grammar
policy_kind      = { "EvictDecayed" | "RingBuffer" | "Throttle" | "FailFast" }
policy_target    = { "on_full" | "on_deadline_breach" | "on_overflow" }
policy_stmt      = { "policy" ~ policy_target ~ "=" ~ policy_kind }

// Introspection Primitives
arena_expr       = { "arena" ~ "." ~ ( "remaining" | "used_bytes" | "capacity" ) ~ "(" ~ ")" }
```

---

## 8. Implementation Roadmap

| Phase | Deliverable | Description |
| :--- | :--- | :--- |
| **Phase 1** | **Grammar & AST Extensions** | Add `@every`, `state`, `policy`, and `arena.*` primitives to `causm.pest` and AST. |
| **Phase 2** | **Dual-Partition Arena Engine** | Update `causm-runtime` to support `BaseWatermark` freezing and $O(1)$ tick resets. |
| **Phase 3** | **Isochronous Epoch Scheduler** | Extend TVM to dispatch `@every` periodic triggers using hardware timer interrupts / TSC ticks. |
| **Phase 4** | **Z3 Inductive Invariance Solver** | Implement compile-time mathematical proof generator for periodic execution bounds across $[0, \infty)$. |
| **Phase 5** | **Saturation Policy Handlers** | Add `RingBuffer`, `EvictDecayed`, and `Throttle` overflow handlers to `causm-runtime`. |
| **Phase 6** | **Stdlib Integration** | Update `std/net` with `try_accept()` and create `examples/continuous_daemon.csm`. |

---

## 9. Conclusion

This proposal resolves the final missing pillar of Causm. By combining **Isochronous Periodic Epochs (`@every`)** with **Dual-Partitioned Arenas** and **Declarative Saturation Policies**, Causm becomes the first language capable of running **infinite, high-throughput network daemons and mission-critical robotics loops with zero garbage collection overhead, constant $O(1)$ memory, and mathematically proven compile-time safety.**
