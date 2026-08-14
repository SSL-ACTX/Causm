# Proposal: Profile-Guided Temporal Contracts (`causm tune`), Inferred Budgets (`taking _`), and Developer Tooling Suite (`causm-devtools`)

**Status:** Proposed  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Compiler Tooling, Temporal Verification & Real-Time Portability  
**Target Crates:** `causm-devtools` (formerly `causm-tracer`), `causm-analysis`, `causm-frontend`, `causm-cli`

---

## 1. Executive Summary

A fundamental challenge in real-time, mission-critical, and embedded systems engineering is **The Hardware Portability Problem**:
* Real-time temporal contracts (e.g., `routine write_all(...) taking 15ms`) hardcoded for a modern 4GHz x86_64 host are invalid when deployed to an edge 100MHz ARM Cortex-M or RISC-V microcontroller (where it may require 120ms).
* Conversely, hardcoding conservative microcontroller budgets on server-class hardware artificially restricts parallel throughput and wastes temporal allocation budgets.

This proposal introduces a three-pillar solution:
1. **Inferred Hardware-Agnostic Temporal Contracts (`taking _` and `taking ?`)**: Shifting the calculation of routine worst-case execution times (WCET) from manual human estimation to formal compile-time target-aware analysis.
2. **Profile-Guided Time Contract Tuning (`causm tune`)**: An automated profiling, fuzzing, and AST-patching engine that empirically derives 99.9th percentile WCET bounds across target platforms under chaotic conditions and directly writes concrete temporal contracts into source files.
3. **Unified Developer Suite (`causm-devtools`)**: Expanding `causm-tracer` into a comprehensive developer suite featuring an AST-preserving code formatter (`causm fmt`), a real-time memory and clock profiler (`causm profile`), and structured JSON/perf telemetry visualizers.

---

## 2. Inferred Temporal Contracts (`taking _` vs `taking ?`)

### 2.1 Formal Syntax & Grammar

In Causm, routines define temporal contracts that bind their maximum execution duration. We introduce two wildcard modes in `causm.pest`:

```pest
duration_wildcard = { "_" | "?" }
duration_limit = { "taking" ~ (amount ~ time_unit | duration_wildcard) }
```

```causm
// 1. Fully inferred compile-time static WCET:
pub routine process_packet(peek data: string) -> bool taking _ {
    // Compiler calculates cost based on instruction weights & target profile
}

// 2. Profile-guided empirical contract tuning marker:
pub routine calculate_fft(samples: array) -> array taking ? {
    // Marked for empirical tuning via `causm tune`
}
```

### 2.2 Semantic Distinctions

| Syntax | Intent | Resolution Phase | Use Case |
| :--- | :--- | :--- | :--- |
| `taking 15ms` | Strict Constant Contract | Compile-Time Z3 Verification | Rigid real-time deadlines, safety-critical aerospace/automotive loops |
| `taking _` | Static Analytical Inference | Compile-Time Flow Analysis | Portable standard library routines (`std/fs`, `std/path`, math routines) |
| `taking ?` | Profile-Guided Empirical | Tooling Phase (`causm tune`) | Non-deterministic algorithms, heavy I/O, complex nested loops |

---

## 3. Profile-Guided Real-Time Optimization: `causm tune`

`causm tune` introduces Profile-Guided Optimization (PGO) to deterministic temporal contracts. Instead of requiring engineers to spend days measuring execution times with oscilloscopes and logic analyzers, `causm tune` automates the entire measurement, statistical filtering, and code-generation cycle.

```
+------------------+       +-------------------+       +-----------------------+
|  Causm Source    | ----> | TVM Chaos Fuzzer  | ----> |  Telemetry Collector  |
|  (taking ?)      |       | (1,000+ Iterations|       |  (WCET P99.9 + Jitter)|
+------------------+       +-------------------+       +-----------------------+
                                                                   |
                                                                   v
+------------------+       +-------------------+       +-----------------------+
| In-Place AST Fix | <---- |   Safety Margin   | <---- | Statistical Evaluator |
|  (taking 46ms)   |       |   Buffer (+10%)   |       | (Eliminate Outliers)  |
+------------------+       +-------------------+       +-----------------------+
```

### 3.1 CLI Invocation

```bash
# Tune all `taking ?` routines targeting a specific embedded board profile:
causm tune --target=armv7-unknown-linux-gnueabihf --iterations=1000 --safety-margin=15

# Tune a specific file in place:
causm tune src/drivers/sensor.csm --fuzz --chaos-jitter=5ms

# Dry-run showing suggested AST diffs without writing to disk:
causm tune --dry-run
```

### 3.2 Automated Workflow Pipeline

1. **AST Target Discovery:** The analyzer scans AST nodes in `.csm` files identifying all routines annotated with `taking ?` or unverified `taking _`.
2. **Chaos Fuzzing & Jitter Simulation:**
   * Runs the target routine in `@chaos` mode across $N$ iterations (default: 1,000).
   * Injects simulated OS scheduling delays, cache misses, memory contention, and branch mispredictions.
3. **Worst-Case Execution Time (WCET) Calculation:**
   * Collects TVM logical clock progression and high-resolution wall-clock ticks.
   * Computes the 99.9th percentile ($P_{99.9}$) execution duration.
4. **Safety Buffer Synthesis:**
   $$\text{Final Budget} = \lceil P_{99.9} \times (1 + \text{margin}) \rceil$$
5. **In-Place AST Rewrite:**
   * Utilizes an AST rewriting pass to replace `taking ?` with concrete values (e.g., `taking 46ms`) in the original `.csm` file, preserving comments and formatting.

---

## 4. Architectural Transformation: `causm-devtools`

We propose renaming and expanding `crates/causm-tracer` into `crates/causm-devtools`.

```
crates/causm-devtools/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── fmt/              # AST-based code formatter (causm fmt)
    │   ├── printer.rs
    │   └── rules.rs
    ├── profiler/         # Clock and Memory Arena Profiling
    │   ├── timeline.rs
    │   ├── memory.rs
    │   └── clock.rs
    ├── tuner/            # `causm tune` Profile-Guided Tuning Engine
    │   ├── fuzzer.rs
    │   ├── statistics.rs
    │   └── rewriter.rs
    └── telemetry/        # Structured Trace Exporters
        ├── json.rs
        ├── speedscope.rs
        └── chrome_trace.rs
```

### 4.1 Subcommands & Capabilities

* **`causm fmt [files...]`**: Formats Causm source code to standard style rules (4-space indent, canonical `@time:` alignments, sorted imports).
* **`causm profile <file.csm>`**: Visualizes memory high-water marks, entropic state transitions, and timeline branch lifecycle durations.
* **`causm tune [files...]`**: Executes empirical WCET benchmark sweeps and patches source code temporal contracts.
* **`causm trace --format=speedscope <file.csm>`**: Exports timeline execution traces to [Speedscope](https://www.speedscope.app/) and Chrome DevTools flamechart formats.

---

## 5. Formal Z3 Solver Integration & Verification

Inferred budgets interact with Z3 static verification under strict formal rules:

1. **Contract Boundedness Proof:**
   $$\forall b \in \text{Paths}(R), \quad \text{Cost}(b) \le \text{Budget}(R)$$
   If `taking _` is used, the Z3 solver resolves:
   $$\text{Budget}(R) = \max_{b \in \text{Paths}(R)} \text{Cost}(b)$$
2. **Interface Contract Subtyping:**
   If a concrete struct implements an `interface` with a defined budget $B_{\text{interface}}$, any method using `taking _` must satisfy:
   $$\text{InferredCost}(R) \le B_{\text{interface}}$$
   Failure to satisfy this inequality results in `SemanticErrorKind::TemporalContractViolated` during compile-time static analysis.

---

## 6. Implementation Roadmap

| Phase | Deliverable | Description |
| :--- | :--- | :--- |
| **Phase 1** | **Grammar & Inferred AST** | Update `causm.pest` and AST to support `taking _` and `taking ?` |
| **Phase 2** | **Static Inference Pass** | Implement automatic path cost calculation in `causm-analysis` for `taking _` |
| **Phase 3** | **Crate Renaming** | Rename `causm-tracer` $\to$ `causm-devtools` and wire module sub-structures |
| **Phase 4** | **`causm tune` Engine** | Implement statistical chaos fuzzer, telemetry recorder, and AST rewriter |
| **Phase 5** | **Stdlib Migration** | Update `crates/causm-stdlib` (`std/fs`, `std/path`, `std/env`) to use `taking _` |
| **Phase 6** | **Formatter & Visualizers** | Add `causm fmt` and speedscope/chrome trace JSON exporter |

---

## 7. Resolution & Impact

By introducing `taking _`, `causm-devtools`, and `causm tune`, Causm becomes the first language in existence to offer **self-calibrating real-time temporal contracts**. It eliminates the manual guesswork of real-time systems programming, makes standard libraries 100% portable across embedded microcontrollers and hyperscale servers, and provides developers with modern, state-of-the-art tooling.
