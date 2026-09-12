# Causm Proposal: SpeedMicro HFT Optimizations & Elastic Determinism

**Status:** Proposed & Implementation in Progress  
**Authors:** Causm Architecture Team  
**Scope:** Cranelift JIT Backend (`causm-jit`), Runtime Engine (`causm-runtime`), Static Cost Analysis (`causm-analysis`), Language Syntax (`causm-frontend`)

---

## 1. Executive Summary

Causm was conceived as a temporally deterministic, isochronous programming language with formal verification of entropic state decay and execution budgets. While the existing virtual machine operates on millisecond-scale (`ms`) budgets with software clocks, ultra-low-latency domains—such as High-Frequency Trading (HFT), high-rate digital signal processing (DSP), robotics, and telemetry—demand sub-microsecond (`µs`) and cycle-accurate temporal determinism.

The **SpeedMicro** architecture elevates Causm from millisecond bytecode interpretation to bare-metal Cranelift machine code execution with hardware-level cycle synchronization, branchless constant-time execution paths, HFT jitter-neutralizing primitives, and **Elastic Determinism**.

---

## 2. Core Architectural Pillars

```
+-------------------------------------------------------------------------+
|                              CAUSM SOURCE                               |
|              routine audio_dsp() taking cycles(4000) { ... }            |
+-------------------------------------------------------------------------+
                                    │
                                    ▼
+-------------------------------------------------------------------------+
|                  FRONTEND & STATIC PESSIMISTIC ANALYSIS                 |
|       - Cooper-Harvey-Kennedy SSA CFG Construction                      |
|       - Pessimistic Z3 Hardware WCET Bounds (Max L3 Miss Costing)       |
+-------------------------------------------------------------------------+
                                    │
                                    ▼
+-------------------------------------------------------------------------+
|                    CRANELIFT SSA JIT COMPILATION                        |
|       - Direct SsaCFG to Cranelift Block Parameters                     |
|       - Branchless CMOV / CSEL Lowering for Conditional Paths           |
|       - Injected Hardware TSC Checkpoints (rdtsc / cntvct_el0)          |
+-------------------------------------------------------------------------+
                                    │
                                    ▼
+-------------------------------------------------------------------------+
|                  HFT RUNTIME JITTER NEUTRALIZATION                      |
|       - Zero Page Faults: Pre-faulted & Memory-Pinned Arenas (mlock)    |
|       - Thread Pinning: Hardware Core Affinity (sched_setaffinity)      |
|       - L1 Cache Warming: Prefetch loops before loop ticks              |
|       - Isochronous Spin-Padding: Burn delta cycles to exact deadline   |
|       - Elastic Determinism: Temporal Freeze on OS interrupt spikes     |
+-------------------------------------------------------------------------+
```

---

## 3. Phase-by-Phase Technical Blueprint

### Phase 1: Constant-Time & Branchless Lowering

At the microsecond level, CPU branch misprediction penalties (~10–20 cycles on modern superscalar pipelines) introduce variable execution latency that violates isochronous timing guarantees.

1. **Branchless Machine Code Generation:**
   Speculative conditional selections (`ConditionalSelect` / `if-else` without side-effects) are compiled directly into branchless CPU instructions:
   - `CSEL` / `CSINC` on AArch64.
   - `CMOV` / `SETcc` on x86_64.
   Both sides of the arithmetic computation are evaluated and selected in constant time, ensuring execution latency is completely invariant with respect to the boolean predicate.

2. **Pessimistic Hardware Z3 Cost Model:**
   Rather than treating instructions as homogeneous $1\text{ms}$ units:
   - Memory reads are costed assuming an L3 cache miss penalty.
   - Z3 calculates the mathematical maximum worst-case execution time ($WCET_{\text{max}}$).
   - This bound forms the fixed target for hardware spin-padding.

3. **Isochronous TSC Spin-Padding:**
   Because normal execution completes faster than the pessimistic $WCET_{\text{max}}$, the JIT backend injects hardware Time-Stamp Counter checks (`cntvct_el0` on ARM, `rdtsc` on x86) at block boundaries:
   - An inlined, non-yielding busy-loop spins until the exact cycle threshold is reached.

---

### Phase 2: Eliminating Hardware & OS Jitter (The HFT Suite)

Standard commodity operating systems introduce context switching, thread migration, and page-fault jitter that can exceed $10\mu\text{s}$. SpeedMicro introduces the **Spoonfeeding** HFT suite:

1. **Pre-Faulted & Pinned Memory Arenas (`mlock`):**
   - At arena creation, every page is touched to force kernel page-table allocation.
   - `libc::mlock` (or `VirtualLock` on Windows) locks the memory pages into physical RAM, guaranteeing that execution never triggers a minor or major page fault.

2. **Core Isolation & Thread Pinning:**
   - Timelines and isolated actors are pinned to dedicated physical CPU cores using `sched_setaffinity`.
   - Thread migration across cores is eliminated, preserving L1/L2 cache warmth and ensuring monotonic TSC progression.

3. **L1 Cache Warming:**
   - Prior to entering an isochronous `loop tick`, active arena variables are prefetched into L1 cache (`_mm_prefetch` / `PRFM`), eliminating memory access variance.

---

### Phase 3: Hardware Clocks & Elastic Determinism

1. **Entangled Invariant TSC:**
   - Modern CPUs maintain an invariant Time-Stamp Counter that increments at a fixed frequency independent of CPU P-state or Turbo Boost scaling.
   - SpeedMicro maps timeline clocks directly to hardware TSC ticks, removing expensive `clock_gettime` system calls.

2. **Elastic Determinism (Acausal Freeze):**
   - Commodity operating systems will occasionally preempt user threads (e.g. for hardware interrupts or hypervisor scheduling).
   - When a sudden TSC delta jump exceeds an acceptable threshold (e.g. $> 50\mu\text{s}$ for a $1\mu\text{s}$ block), SpeedMicro classifies the event as an **External Hardware Preemption**.
   - Instead of crashing due to a deadline violation, the runtime triggers a **Temporal Freeze**: the interrupted cycles are marked as "lost to the void" (acausal time). The logical clock and entanglement matrix freeze during the interrupt, preserving causal relationships and invariants across all timelines.

---

## 4. Language Ergonomics & Syntax

SpeedMicro introduces cycle-aware syntax extensions:

### 4.1 Cycle-Level Routine Contracts
```causm
routine process_audio_frame(buffer: clone [int; 128]) -> int taking cycles(4500) {
    let mut energy = 0
    for sample in buffer {
        let energy = energy + sample * sample
    }
    yield energy
}
```

### 4.2 Explicit Spin Synchronization (`sync_cycles`)
```causm
isolate HighFrequencyEngine {
    loop tick {
        let trade_signal = evaluate_market(tick_data)
        sync_cycles // Injects rdtsc/cntvct_el0 spin-pad to exact isochronous cadence
    }
}
```

---

## 5. Implementation Roadmap

| Milestone | Component | Description | Status |
|---|---|---|---|
| **M1** | `causm-jit` | Pure Cranelift JIT backend, direct SSA CFG lowering, tuples, memory, strings | **Complete** |
| **M2** | `causm-runtime` | JIT routine execution hook in `Vm`, `--jit` CLI flag | **Complete** |
| **M3** | `causm-jit::hft` | Memory pinning (`mlock`), thread affinity, cache warming, TSC spin-pad | **Underway** |
| **M4** | `causm-analysis` | Cycle-level cost estimation pass and pessimistic hardware Z3 bounds | **Planned** |
| **M5** | `causm-frontend` | `taking cycles(N)` and `sync_cycles` parser extensions | **Planned** |
