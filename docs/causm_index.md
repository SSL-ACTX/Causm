# Causm Documentation Hub

Welcome to the official documentation for the Causm. This hub provides a structured entry point to the formal specifications, virtual machine internals, and original design proposals that define the Causm ecosystem.

## 1. Formal Language Specifications (`docs/spec/`)
The following documents define the formal behavior, syntax, and semantics of Causm.

- **[Formal Syntax Reference](./spec/causm_spec_syntax.md)**: EBNF grammar and program structure.
- **[Core Semantic Model](./spec/causm_spec_semantics.md)**: Operational semantics and entropic state transitions.
- **[Formal Verification Guard](./spec/causm_spec_formal_verification.md)**: Symbolic correctness kernel powered by Z3.
- **[Entropic Channels](./spec/causm_spec_channels.md)**: Mid-timeline state transfer and causal synchronization.
- **[Control Flow](./spec/causm_spec_control_flow.md)**: Branching, speculative execution, and reconciliation.
- **[Iteration & Pacing](./spec/causm_spec_iteration.md)**: Deterministic loops and temporal pacing.
- **[Routine Contracts](./spec/causm_spec_routines.md)**: Procedure definitions and WCET enforcement.
- **[Speculative Branches](./spec/causm_spec_speculation.md)**: Micro-timelines and rollback mechanisms.
- **[Temporal Leases](./spec/causm_spec_leases.md)**: Transient, time-bound memory borrowing.
- **[Type System](./spec/causm_spec_types.md)**: Entropic types (int/float) and temporal contracts.
- **[Object-Oriented Programming](./spec/causm_spec_oop.md)**: Struct types, default values, constants, and methods.
- **[Topological Field Access](./spec/causm_spec_topologies.md)**: Memory layout and field-level entropy.
- **[Asynchronous Promises](./spec/causm_spec_promises.md)**: Temporal promises and causal synchronization.
- **[Timeline Routing](./spec/causm_spec_temporal_routing.md)**: Advanced routing across isolated timelines.
- **[Isochronous Scheduling](./spec/causm_spec_isochronous_scheduling.md)**: High-precision temporal synchronization.
- **[Module System & Imports](./spec/causm_spec_modules.md)**: Multi-file code imports, namespaces, and dependency graphs.

## 2. TVM Internals (`docs/tvm/`)
Technical documentation regarding the Register-based Temporal Virtual Machine.

- **[Acausal Debugging](./tvm/causm_tvm_debugging.md)**: Time-travel diagnostics and trace logs.
- **[Memory Reclamation](./tvm/causm_tvm_memory_reclamation.md)**: Entropic Garbage Collection (EGC) and arena management.

## 3. Design Proposals & RFCs (`docs/proposals/`, `docs/rfc/`)
Historical design documents and the standard RFC process.

- **[Standard RFC Template](./rfc/causm_RFC.md)**: Guidelines for proposing language changes.
- **[FFI & Syscall Bridge Proposal](./proposals/causm_prop_ffi_syscall_bridge.md)**: Foreign function interface, native system calls, and self-hosted standard library architecture.
- **[Module System Proposal](./proposals/causm_prop_import_system.md)**: Code imports, manifests, and entropic namespace resolution.
- **[Entropic GC Proposal](./proposals/causm_prop_egc.md)**: Original design for deterministic reclamation.
- **[Advanced Mechanics](./proposals/causm_prop_advanced_mechanics.md)**: Refinements for channels, contracts, and diagnostics.
- **[Advanced Routing](./proposals/causm_prop_advanced_routing.md)**: Early designs for complex timeline topologies.
- **[Advanced Data Structures](./proposals/causm_prop_advanced_data_structures.md)**: Proposals for complex entropic collection types.
- **[Advanced Loops](./proposals/causm_prop_advanced_loops.md)**: Extended iteration and pacing mechanisms.
- **[Developer Ergonomics](./proposals/causm_prop_dev_ergonomics.md)**: Strategies for improving language usability.
- **[If/Else Speculation](./proposals/causm_prop_if_else.md)**: Design for speculative conditional branches.
- **[Isochronous Matrix](./proposals/causm_prop_isochronous_matrix.md)**: Mathematical foundations for temporal scheduling.
- **[Iterative Paced Loops](./proposals/causm_prop_iter_paced_loop.md)**: Proposal for time-constrained iteration.
- **[Primitive Types](./proposals/causm_prop_primitive_types.md)**: Specification for primitive numeric type extensions.
- **[Promises & Causality](./proposals/causm_prop_promises.md)**: Design for acausal synchronization.
- **[Routine TP Contracts](./proposals/causm_prop_routines_tp_contract.md)**: Temporal performance contracts for routines.
- **[Speculative Branches Proposal](./proposals/causm_prop_speculative_branches.md)**: Early research on speculative execution.
- **[Temporal Leases Proposal](./proposals/causm_prop_temporal_leases.md)**: Design for time-bound resource ownership.
- **[Type Casting & Broadcasting](./proposals/causm_prop_casting_and_broadcasting.md)**: Design for the `as` casting operator and elementwise array broadcasting.
- **[Advanced Entropic OOP](./proposals/causm_prop_advanced_entropic_oop.md)**: Monomorphized generic method dispatch, associated lifecycle types, and entropic interface constraints.
- **[Profile-Guided Tuning & DevTools](./proposals/causm_prop_profile_guided_tuning_and_devtools.md)**: Inferred (`taking _`) and empirical (`taking ?`) temporal contracts, `causm-devtools` profiling/tuning suite, and continuous contract synthesis.
- **[Self-Hosted std/time Module](./proposals/causm_prop_std_time.md)**: High-resolution monotonic timestamps (`Instant`, `Duration`), wall-clock epoch time, and precision sleeping.
- **[Self-Hosted std/net Module](./proposals/causm_prop_std_net.md)**: POSIX socket networking, `SocketAddr`, `TcpStream`, `TcpListener`, and socket descriptor management.


---
*This index is maintained as the authoritative source for Causm documentation.*
