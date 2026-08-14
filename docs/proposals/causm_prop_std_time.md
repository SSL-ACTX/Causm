# Causm Proposal: Self-Hosted `std/time` Standard Library Module

* **Proposal Name:** `causm_prop_std_time`
* **Author:** Iris Seravelle & Seuriin
* **Status:** Proposed / Draft
* **Category:** Standard Library / Runtime Architecture / Real-Time Systems
* **Target Version:** `causm-stdlib v0.2.0`

---

## 1. Executive Summary

Causm is fundamentally a real-time, time-aware programming language with compile-time temporal budgets (`taking <ms>`, `taking _`), clock coordinates (`@0ms:`, `@+10ms:`), and hardware-isochronous execution loops (`loop tick`). However, Causm currently lacks a self-hosted, native standard library module for querying wall-clock time, high-resolution monotonic timestamps, duration arithmetic, and time measurement.

This proposal specifies **`std/time`**: a self-hosted standard library module implemented entirely in `.csm` using low-level C ABI bindings to POSIX `libc.so.6` (`clock_gettime`, `gettimeofday`, `nanosleep`) combined with high-level Causm struct types (`Instant`, `Duration`, `Timespec`) and inferred temporal contracts (`taking _`).

---

## 2. Motivation & Philosophy

### 2.1 The Two Clocks of Causm
Causm programs operate under two distinct temporal dimensions:
1. **Logical / Deterministic TVM Clock:** The discrete, virtual clock tracked by the Temporal Virtual Machine (TVM) and verified statically by Z3 (e.g. `@0ms:`, `@100ms:`, `taking 20ms`).
2. **Physical / Wall-Clock Monotonic Time:** Physical hardware clock progression measured in nanoseconds since boot (`CLOCK_MONOTONIC`) or UNIX epoch (`CLOCK_REALTIME`).

A self-hosted `std/time` module bridges these two domains, allowing developers to:
- Benchmark code execution against real hardware elapsed time.
- Synchronize external physical sensor streams with TVM logical timelines.
- Represent timestamps, durations, and timeouts using strongly typed entropic structs.

---

## 3. Architecture & Module Structure

The `std/time` module will be located in `crates/causm-stdlib/csm/std/time/` and exposed via the root import `import "std/time"` or `from "std/time" import *`.

```
crates/causm-stdlib/csm/std/time/
├── mod.csm         # Public API entrypoint and re-exports
├── types.csm       # Struct definitions (Instant, Duration, Timespec)
├── ffi.csm         # C ABI declarations (clock_gettime, gettimeofday, nanosleep)
└── ops.csm         # Pure Causm arithmetic, elapsed calculation, and formatting
```

---

## 4. Detailed Specification

### 4.1 Native FFI Layer (`std/time/ffi.csm`)

```causm
// crates/causm-stdlib/csm/std/time/ffi.csm
// POSIX Clock IDs: CLOCK_REALTIME = 0, CLOCK_MONOTONIC = 1, CLOCK_PROCESS_CPUTIME_ID = 2

foreign "libc.so.6" abi("C") {
    pub routine clock_gettime(peek clk_id: i32, peek tp: Timespec) -> i32 taking 1ms
    pub routine nanosleep(peek req: Timespec, peek rem: Timespec) -> i32 taking 1ms
    pub routine time(peek tloc: i64) -> i64 taking 1ms
}
```

### 4.2 Data Types & Lifecycle (`std/time/types.csm`)

```causm
// crates/causm-stdlib/csm/std/time/types.csm

// 1. Raw POSIX struct timespec
pub type Timespec = struct {
    tv_sec: i64 = 0,
    tv_nsec: i64 = 0
}

// 2. High-resolution Monotonic Timestamp
pub type Instant = struct {
    secs: i64,
    nanos: i64
}

// 3. Time Duration Representation
pub type Duration = struct {
    nanos_total: i64
}
```

### 4.3 High-Level Operations & Inferred Contracts (`std/time/ops.csm`)

All pure Causm helper routines utilize `taking _` so that the static analyzer computes exact instruction costs across platforms:

```causm
// crates/causm-stdlib/csm/std/time/ops.csm

/// Captures the current monotonic instant
pub routine now() -> Instant taking _ {
    let mut ts = struct Timespec { tv_sec = 0, tv_nsec = 0 }
    // CLOCK_MONOTONIC = 1
    let _code = call clock_gettime(1, &ts)
    struct Instant {
        secs = ts.tv_sec,
        nanos = ts.tv_nsec
    }
}

/// Captures current UNIX timestamp in seconds
pub routine unix_timestamp() -> i64 taking _ {
    let raw = call time(0)
    raw
}

/// Returns elapsed duration since a previous Instant in milliseconds
pub routine elapsed_ms(peek start: Instant) -> i64 taking _ {
    let current = call now()
    let sec_diff = current.secs - start.secs
    let nano_diff = current.nanos - start.nanos
    let total_ms = (sec_diff * 1000) + (nano_diff / 1000000)
    total_ms
}

/// Returns elapsed duration since a previous Instant in microseconds
pub routine elapsed_us(peek start: Instant) -> i64 taking _ {
    let current = call now()
    let sec_diff = current.secs - start.secs
    let nano_diff = current.nanos - start.nanos
    let total_us = (sec_diff * 1000000) + (nano_diff / 1000)
    total_us
}

/// Creates a Duration from milliseconds
pub routine from_millis(ms: i64) -> Duration taking _ {
    struct Duration {
        nanos_total = ms * 1000000
    }
}

/// Creates a Duration from seconds
pub routine from_secs(secs: i64) -> Duration taking _ {
    struct Duration {
        nanos_total = secs * 1000000000
    }
}

/// Blocks physical execution for the specified milliseconds
pub routine sleep_ms(ms: i64) -> i32 taking _ {
    let sec = ms / 1000
    let nsec = (ms % 1000) * 1000000
    let req = struct Timespec { tv_sec = sec, tv_nsec = nsec }
    let rem = struct Timespec { tv_sec = 0, tv_nsec = 0 }
    call nanosleep(&req, &rem)
}
```

---

## 5. Idiomatic Usage Example

```causm
// examples/time_showcase.csm
import "std/time" as Time

@0ms: {
    print("--- Causm std/time Benchmark & Telemetry Showcase ---")

    let start = call Time.now()
    let ts = call Time.unix_timestamp()
    print("Current UNIX Epoch (seconds): " + ts)

    // Execute real-time workload
    let counter = 0
    while (counter < 10000) taking 20ms {
        counter = counter + 1
    }

    let elapsed = call Time.elapsed_us(&start)
    print("Workload Physical Execution Time: " + elapsed + " µs")

    // Precision sleep
    print("Sleeping for 10ms...")
    call Time.sleep_ms(10)
    print("Resumed.")
}
```

---

## 6. Formal Verification & Entropic Invariants

1. **Borrow Safety on Measurement:** Routines measuring elapsed time (`elapsed_ms`, `elapsed_us`) accept `peek start: Instant`, ensuring the reference instant is never consumed or decayed during measurement loops.
2. **Deterministic Bound Inference:** All math operations inside `std/time` are straight-line arithmetic without unbounded recursion, ensuring the Z3 solver computes tight, finite WCET bounds for every `taking _` wrapper.
3. **Safe Interoperability with TVM:** Physical time measurements do not violate TVM logical clock invariance; TVM local clocks remain deterministic, while `Instant` measures physical hardware deltas.

---

## 7. Implementation Roadmap

| Phase | Deliverable | Description |
| :--- | :--- | :--- |
| **Phase 1** | **Csm Source Files** | Create `crates/causm-stdlib/csm/std/time/{mod,types,ffi,ops}.csm` |
| **Phase 2** | **Crate Registration** | Expose `STD_TIME_*` in `crates/causm-stdlib/src/lib.rs` and `get_module` |
| **Phase 3** | **Showcase Example** | Add `examples/time_showcase.csm` demonstrating timestamps & benchmarks |
| **Phase 4** | **Test Suite** | Add unit and integration tests in `crates/causm-cli/tests/integration/` |
