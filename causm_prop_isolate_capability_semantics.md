# Proposal: Conditional Capability Semantics, Fine-Grained Routine Contracts, and Resource-Bounded Isolates

**Status:** Draft Proposal  
**Author:** Causm Architecture Team  
**Category:** Language Semantics, Capability Verification & Isolate Sandboxing  

---

## 1. Executive Summary & Motivation

In Causm, `isolate` blocks provide bounded execution envelopes with deterministic compute limits (`enable cpu(...)`, `enable memory(...)`, `enable slice(...)`) and explicit security boundaries (`require <Capability>`).

Currently, capability checks in the compiler/analyzer evaluate foreign blocks and FFI invocations broadly. When an isolate imports a standard library module (such as `std/fs` or `std/net`), attempting to use any pure/in-memory utility (e.g., `path_basename` or `url_parse`) can fail if the module contains ungranted host FFI blocks, forcing either:
1. Permitting dangerous/broad capabilities (`require System.FFI`) inside untrusted isolates.
2. Fragmenting the standard library into duplicate "isolated" and "host" variants (e.g., `std/fs` vs `std/fs_pure`), introducing severe maintenance debt and API fragmentation.

This proposal introduces **Conditional Capability Semantics**:
1. **Routine-Level Capability Contracts (`requires <Capability>`)**: Declarative capability requirements on function signatures.
2. **Call-Graph & Reachability Tree-Shaking**: The analyzer checks capabilities only for reachable call graphs within an isolate, ignoring unused FFI functions in imported modules.
3. **Capability Introspection / Conditional Branching**: Compile-time or runtime capability branching (`if capability(System.FFI)` or pure fallback paths).
4. **Virtualized In-Memory Capability Providers**: Allowing isolate manifests to satisfy I/O capabilities through pure memory-backed providers without host syscalls.

---

## 2. Syntax Specification

### 2.1 Routine Capability Annotations (`requires <Capability>`)

Routines declare required capabilities directly in their signatures:

```causm
// Pure utility — requires 0 capabilities, callable anywhere
fn path_join(dir: string, file: string) -> string {
    return dir + "/" + file
}

// Privileged I/O routine — requires explicit capability
fn create_file(path: string) -> File requires System.FFI {
    let fd = ffi_call "libc.so.6":open(path, 577, 438)
    return File { fd = fd, path = path }
}

// Scoped capability requirements
fn write_temp(data: string) -> bool requires System.IO(path="/tmp") {
    ...
}
```

### 2.2 Isolate Block Manifest & Capability Scoping

Inside an `isolate` block, only declared capabilities are available in the local capability stack:

```causm
@0ms: {
    isolate untrusted_worker {
        // Resource constraints
        enable cpu(20ms)
        enable memory(32KB)
        enable slice(5ms)

        // Only logging permitted (System.FFI is NOT granted)
        require System.Log

        // Safe import: pure routines work seamlessly
        from "std/fs" import path_join, path_basename

        let clean_path = path_join("/tmp", "output.dat")
        print("Resolved path: " + clean_path)

        // COMPILE ERROR: create_file requires System.FFI, which is missing in untrusted_worker
        // let f = create_file(clean_path)
    }
}
```

### 2.3 Conditional Capability Handling & Fallbacks

Libraries can provide unified APIs with conditional degradation when running under limited or zero capabilities:

```causm
routine persist_state(data: string) -> bool {
    if capability(System.FFI) {
        // Direct host persistence
        let file = create_file("/var/data/state.bin")
        write_all(file, data, len(data))
        return true
    } else if capability(System.VirtualStorage) {
        // Fallback to in-memory / virtual isolate storage
        VirtualStorage.write("state.bin", data)
        return true
    } else {
        // Degrade gracefully without crashing
        return false
    }
}
```

---

## 3. Formal Analysis & Verifier Pipeline

```
+-------------------------------------------------------------+
|                      causm-frontend                         |
|  - Parse routine `requires <Cap>` clauses                   |
|  - Parse isolate manifests and `capability(...)` conditions |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                      causm-analysis                         |
|  1. Call Graph Construction: Build reachable routine set    |
|  2. Capability Stack Resolution: Push/pop isolate manifests |
|  3. Reachability Pruning: Prune uncalled module routines    |
|  4. Static Capability Verification (Z3 & Solver):           |
|     - Check every called routine against active cap stack   |
|     - Verify memory & WCET constraints                      |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                      causm-runtime (TVM)                    |
|  - Execute in bounded Memory Arena (OOM protection)         |
|  - Track WCET isochronous tick budget                       |
|  - Enforce Virtualized Capability Backends (if specified)   |
+-------------------------------------------------------------+
```

### 3.1 Static Reachability & Tree-Shaking
When module `M` is imported inside an `isolate`:
1. The `EntropicAnalyzer` scans all top-level statements of the `isolate` body.
2. It builds a directed call-graph `G = (V, E)` of reachable routines.
3. For every routine $r \in V$:
   $$\forall c \in \text{required\_capabilities}(r), \quad c \subseteq \text{active\_isolate\_capabilities}$$
4. Unreachable routines in `M` with ungranted capabilities are completely ignored and pruned from verification errors.

---

## 4. Virtualized Backends for Limited / Sandboxed Environments

For embedded, WASM, or restricted worker environments, isolate manifests can bind abstract capabilities to pure in-memory virtual drivers:

```causm
@0ms: {
    isolate ram_fs_worker {
        enable memory(64KB)
        enable cpu(100ms)

        // Route System.IO to in-memory virtual arena
        require System.IO(backend="virtual", max_size=32KB)

        import "std/fs" as fs
        let file = fs.create_file("/virtual/test.log")
        fs.write_all(file, "Virtualized safe storage", 24)
    }
}
```

---

## 5. Architectural Advantages

| Feature | Fragmented Stdlib Approach | Conditional Capability Semantics |
| :--- | :--- | :--- |
| **Code Reuse** | Low (separate `std/fs` vs `std/fs_pure`) | **100% unified standard library** |
| **Safety** | Manual developer diligence | **Formal compile-time static proof** |
| **Resource Portability** | Custom builds per platform | **Dynamic configuration via isolate manifests** |
| **Ecosystem Ergonomics**| High fragmentation & import confusion | **Single import path with automatic capability gating** |

---

## 6. Implementation Milestones

1. **Phase 1: AST & Grammar Updates (`causm-core`, `causm-frontend`)**
   * Add `requires <Capability>` clause to `RoutineDeclaration` in AST.
   * Add `capability(<Cap>)` expression built-in.
2. **Phase 2: Semantic Analysis & Reachability (`causm-analysis`)**
   * Implement call-graph reachability pruning in `EntropicAnalyzer`.
   * Enforce routine capability contracts against `capability_stack`.
3. **Phase 3: Standard Library Integration (`causm-stdlib`)**
   * Annotate I/O functions across `std/fs`, `std/net`, `std/process`, `std/env` with specific `requires` contracts.
   * Verify all pure routines remain accessible in zero-capability isolates.
4. **Phase 4: Comprehensive Test Suite (`causm-cli/tests/integration`)**
   * Add tests for pure imports in zero-cap isolates, missing capability errors on call, and virtual backend routing.
