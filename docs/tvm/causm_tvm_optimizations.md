# Causm Virtual Machine (TVM) IR Optimization & Analysis Passes

This document details the intermediate representation (IR) optimization passes, static analysis transformations, and SSA verification rules executed by the Causm Virtual Machine pipeline.

---

## 1. Overview of the Optimization Pipeline

Before lowered IR is executed by the TVM or formally verified by the Z3 solver kernel, it undergoes a series of flow-sensitive, entropic-aware IR transformation passes:

```
Lowered Flat IR -> CFG Construction -> SSA Renaming (Cooper-Harvey-Kennedy)
                -> Lease Escape Optimization
                -> Channel Liveness Analysis & Dead Channel Elimination
                -> Entropic Match Entropy Folding
                -> CFG Simplification & WCET Static Verification
                -> Optimized SSA IR / TVM Bytecode
```

---

## 2. Pass Details

### 2.1 Lease Escape Optimization (`lease_escape`)
Identifies transient `lease` bindings that do not escape their temporal scopes. Reclaims leased registers without incurring heap/arena lease tracking overhead.

### 2.2 Channel Liveness Analysis & Dead Channel Elimination
Computes channel instruction def-use chains across control flow basic blocks. Eliminates redundant channel open/close instructions (`open_chan`) while preserving cross-block channel data flow semantics.

### 2.3 Entropic Match Entropy Folding
Folds deterministic `MatchEntropy` control flow branches at compile time when target SSA registers are statically known to be in `Valid`, `Decayed`, or `Consumed` states.
- *Safety Invariant*: Folding is bypassed if target registers are mutated or consumed across parallel timeline branches.

### 2.4 CFG Simplification & Static WCET Verification
Simplifies basic block control flow graphs by merging redundant jump targets and calculating deterministic worst-case execution time (WCET) bounds for all basic blocks.

---

## 3. Visualization Tools

The CLI provides diagnostic visualization flags:
- `--emit cfg-dot`: Emits Graphviz DOT format for Control Flow Graphs.
- `--emit ssa-dot`: Emits Graphviz DOT format for SSA versioned register basic blocks.
