# Causm Documentation Hub

Welcome to the official documentation for the Causm. This hub provides a structured entry point to the formal specifications, virtual machine internals, and original design proposals that define the Causm ecosystem.

## 1. Formal Language Specifications (`docs/spec/`)
The following documents define the formal behavior, syntax, and semantics of Causm.

- **[Formal Syntax Reference](./spec/causm_spec_syntax.md)**: EBNF grammar, compiler attributes, and program structure.
- **[Core Semantic Model](./spec/causm_spec_semantics.md)**: Operational semantics and entropic state transitions.
- **[Formal Verification Guard](./spec/causm_spec_formal_verification.md)**: Symbolic correctness kernel powered by Z3.
- **[Control Flow & Pattern Matching](./spec/causm_spec_control_flow.md)**: Branching, algebraic matching, speculative execution, and reconciliation.
- **[Iteration & Pacing](./spec/causm_spec_iteration.md)**: Deterministic loops, stepped ranges, and temporal pacing.
- **[Routine Contracts](./spec/causm_spec_routines.md)**: Procedure definitions, direct invocations, and WCET enforcement.
- **[Speculative Branches](./spec/causm_spec_speculation.md)**: Micro-timelines and rollback mechanisms.
- **[Temporal Leases](./spec/causm_spec_leases.md)**: Transient, time-bound memory borrowing.
- **[Type System](./spec/causm_spec_types.md)**: Entropic types, tuples, fixed arrays, and temporal contracts.
- **[Object-Oriented Programming](./spec/causm_spec_oop.md)**: Struct types, ADT Enums, distinct newtypes, const generics, and methods.
- **[Topological Field Access](./spec/causm_spec_topologies.md)**: Memory layout and field-level entropy.
- **[Asynchronous Promises](./spec/causm_spec_promises.md)**: Temporal promises and causal synchronization.
- **[Timeline Routing](./spec/causm_spec_temporal_routing.md)**: Advanced routing across isolated timelines.
- **[Isochronous Scheduling](./spec/causm_spec_isochronous_scheduling.md)**: High-precision temporal synchronization.
- **[Module System & Imports](./spec/causm_spec_modules.md)**: Multi-file code imports, namespaces, and dependency graphs.

## 2. Standard Library (`crates/causm-stdlib/`)
Documentation for the pure Causm embedded standard library modules.

- **[Standard Library Overview](../crates/causm-stdlib/README.md)**: Architecture, `.csa` bytecode archive caching, and module guide.
  - **`std/fs`**: POSIX file operations, directory traversal, and auto-closing handles.
  - **`std/net`**: POSIX TCP stream connections and socket listeners.
  - **`std/time`**: High-resolution monotonic timestamps (`Instant`), durations, and precision sleeping.
  - **`std/json`**: Pure Causm native JSON parsing, serialization, and ADT enum representations.
  - **`std/http`**: HTTP/1.1 request formatting, client networking, and response decoding.
  - **`std/encoding`**: Base64 encoding/decoding, binary serialization, and bitwise manipulation.
  - **`std/collection`**: Array utilities, `RingBuffer`, `Stack`, `Queue`, and `BitSet`.
  - **`std/process`**: Process spawning, pipe redirection, and exit status inspections.

---

## 3. TVM Internals (`docs/tvm/`)
Technical architecture of the Register-based Temporal Virtual Machine.

- **[Acausal Debugging](./tvm/causm_tvm_debugging.md)**: Time-travel diagnostics, causal trace logs, and state rollback.
- **[Memory Reclamation & EGC](./tvm/causm_tvm_memory_reclamation.md)**: Entropic Garbage Collection, dual-partitioned linear epoch arenas, and saturation policies.
- **[TVM Optimization Passes](./tvm/causm_tvm_optimizations.md)**: SSA phi nodes, dominance trees, CFG simplification, and copy propagation.

---

## 4. Proposals, RFCs & Developer Guides (`docs/proposals/`, `docs/rfc/`)
Design proposals, Request for Comments (RFCs), and compiler extension guides.

- **[Standard RFC Template](./rfc/causm_RFC.md)**: Process and template for proposing language modifications.
- **[Compiler Plugins & WebAssembly SDK](./proposals/causm_prop_compiler_plugins_and_wasm_sdk.md)**: Plugin architecture, WASM host sandbox (`wasmi`), IPC driver, and `causm-plugin-sdk`.
- **[Implemented Proposals Archive](./proposals/implemented/)**: Complete catalog of historical design proposals detailing core grammar, memory models, and standard libraries.

---
*This index is maintained as the authoritative source for Causm documentation.*
