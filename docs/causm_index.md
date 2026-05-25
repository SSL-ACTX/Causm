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
- **[Topological Field Access](./spec/causm_spec_topologies.md)**: Memory layout and field-level entropy.
- **[Asynchronous Promises](./spec/causm_spec_promises.md)**: Temporal promises and causal synchronization.
- **[Timeline Routing](./spec/causm_spec_temporal_routing.md)**: Advanced routing across isolated timelines.
- **[Isochronous Scheduling](./spec/causm_spec_isochronous_scheduling.md)**: High-precision temporal synchronization.

## 2. TVM Internals (`docs/tvm/`)
Technical documentation regarding the Register-based Temporal Virtual Machine.

- **[Acausal Debugging](./tvm/causm_tvm_debugging.md)**: Time-travel diagnostics and trace logs.
- **[Memory Reclamation](./tvm/causm_tvm_memory_reclamation.md)**: Entropic Garbage Collection (EGC) and arena management.

## 3. Design Proposals & RFCs (`docs/proposals/`, `docs/rfc/`)
Historical design documents and the standard RFC process.

- **[Standard RFC Template](./rfc/causm_RFC.md)**: Guidelines for proposing language changes.
- **[Entropic GC Proposal](./proposals/causm_prop_egc.md)**: Original design for deterministic reclamation.
- **[Advanced Mechanics](./proposals/causm_prop_advanced_mechanics.md)**: Refinements for channels, contracts, and diagnostics.
- **[Advanced Routing](./proposals/causm_prop_advanced_routing.md)**: Early designs for complex timeline topologies.
- **[Developer Ergonomics](./proposals/causm_prop_dev_ergonomics.md)**: Strategies for improving language usability.
- **[If/Else Speculation](./proposals/causm_prop_if_else.md)**: Design for speculative conditional branches.
- **[Isochronous Matrix](./proposals/causm_prop_isochronous_matrix.md)**: Mathematical foundations for temporal scheduling.
- **[Iterative Paced Loops](./proposals/causm_prop_iter_paced_loop.md)**: Proposal for time-constrained iteration.
- **[Promises & Causality](./proposals/causm_prop_promises.md)**: Design for acausal synchronization.
- **[Routine TP Contracts](./proposals/causm_prop_routines_tp_contract.md)**: Temporal performance contracts for routines.
- **[Speculative Branches Proposal](./proposals/causm_prop_speculative_branches.md)**: Early research on speculative execution.
- **[Temporal Leases Proposal](./proposals/causm_prop_temporal_leases.md)**: Design for time-bound resource ownership.

---
*This index is maintained as the authoritative source for Causm documentation.*
