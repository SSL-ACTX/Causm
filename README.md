# Causm

**A Domain-Specific Language for Temporal and Entropic Memory Models**

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL_3.0-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/Status-Research-red.svg)]()

> [!IMPORTANT]
> Causm is an experimental toolchain for exploring temporal and entropic memory models. It is not intended for production environments. Specifications and implementation are subject to radical changes.

---

Causm is a domain-specific research language designed to address the inherent non-determinism in concurrent systems. By treating time as a first-class execution primitive and implementing an entropic memory model, Causm provides a framework where race conditions are eliminated through the mathematical enforcement of temporal invariants.

This repository contains the reference implementation of the Causm toolchain, including the compiler, analyzer, and the Z3-governed Register-based Temporal Virtual Machine (TVM).

## Table of Contents
- [Research Objectives](#research-objectives)
- [Architecture](#architecture)
- [Documentation](#documentation)
- [Getting Started](#getting-started)
- [Research Patterns](#research-patterns)
- [Execution Interface](#execution-interface)
- [License](#license)

---

## Research Objectives

Causm investigates the feasibility of the following hypotheses:

1.  **Temporal Execution Primitives**: Can race conditions be mitigated by making time a verifiable execution primitive? Causm explores "Isochronous Scheduling" to prevent unexpected interleaving by enforcing rigid temporal alignment.
2.  **Entropic Memory Model**: Is it feasible to model memory safety through state decay rather than borrow checking? Causm tests if data access as a destructive operation can provide a symbolically verifiable alternative to traditional models.
3.  **Causal Synchronization**: Can cross-timeline state be synchronized without traditional locking? The project researches how independent execution branches communicate state transitions while maintaining consistent causal order.
4.  **SMT-Based Temporal Correctness**: How effectively can kernels verify correctness in non-linear code? The project uses an experimental Z3-governed kernel to unroll loops and branches into symbolic constraints.

---

## Architecture

Causm employs a multi-pass pipeline to ensure programs are only executed if their temporal and entropic safety is mathematically proven.

```mermaid
graph TD
    Source[".csm Source Code"] --> Parser["Causm Parser (Pest)"]
    Parser --> AST["Abstract Syntax Tree"]
    
    subgraph "Correctness Kernel"
        AST --> Analyzer["Entropic Analyzer"]
        Analyzer --> Z3Guard["Formal Verification Guard (Z3)"]
        Z3Guard --> Proofs{{"Symbolic Proofs"}}
    end
    
    Proofs -- "UNSAT (Violation)" --> Error["Semantic Error"]
    Proofs -- "SAT (Safe)" --> Lowering["IR Lowering"]
    
    Lowering --> TVM["Register-based TVM"]
    
    subgraph "TVM Execution"
        TVM --> Sched["Isochronous Scheduler"]
        TVM --> Arena["Entropic Arena"]
        Sched --> Padding["Deterministic Padding"]
        Arena --> EGC["Entropic GC"]
    end
```

---

## Documentation

Technical specifications and research documentation are in the `docs/` directory. See the [Full Documentation Hub](docs/causm_index.md) for a structured overview.

### Language Specifications (`docs/spec/`)
- **Core**: [Syntax](docs/spec/causm_spec_syntax.md), [Semantic Model](docs/spec/causm_spec_semantics.md), [Type System](docs/spec/causm_spec_types.md)
- **Correctness**: [Formal Verification Guard](docs/spec/causm_spec_formal_verification.md), [Routine Contracts](docs/spec/causm_spec_routines.md)
- **Temporal Mechanics**: [Isochronous Scheduling](docs/spec/causm_spec_isochronous_scheduling.md), [Iteration & Pacing](docs/spec/causm_spec_iteration.md)
- **Concurrency & State**: [Entropic Channels](docs/spec/causm_spec_channels.md), [Timeline Routing](docs/spec/causm_spec_temporal_routing.md), [Asynchronous Promises](docs/spec/causm_spec_promises.md)

### TVM Internals (`docs/tvm/`)
- [Acausal Debugging](docs/tvm/causm_tvm_debugging.md): Diagnostics and trace logs.
- [Memory Reclamation](docs/tvm/causm_tvm_memory_reclamation.md): Entropic GC (EGC) and arena management.

---

## Getting Started

### Prerequisites

*   **Rust:** Version 1.75.0 or later
*   **Z3 Solver:** Required for formal verification

### Execution Interface

```bash
# Analyze and run a source file
cargo run -- examples/time_travel_showcase.csm

# Perform formal verification without execution
cargo run -- --check examples/sample.csm

# Execute with full entropic tracing
cargo run -- --run --trace-entropy examples/sample.csm
```

---

## Research Patterns

### Entropic Transfer
```ictl
isolate Producer {
    require Chan.Outbound(id="sensor_bus", type=int)
    
    let reading = 42
    chan_send sensor_bus(reading)
    // 'reading' is now Consumed.
}

isolate Consumer {
    require Chan.Inbound(id="sensor_bus", type=int, latency=5ms)
    
    await_chan sensor_bus
    let data = chan_recv(sensor_bus)
}
```

### Temporal Pacing
```ictl
routine process_packet(p: PacedIterable<int, 2ms>) taking 10ms {
    for item in p {
        compute(item)
    }
}
```

---

## License
Licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). See [LICENSE](LICENSE).

---

<div align="center">

Built with 🦀 & ⚡ by [Seuriin](https://github.com/SSL-ACTX)

</div>
