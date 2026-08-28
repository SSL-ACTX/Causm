# Proposal: Universal Standard Library Modernization — Isolate-Native Sandboxing, WASM/WASI Virtualization, and Tiered FFI Architecture

**Status:** Proposed  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Standard Library Architecture, Capability-Based Security & Platform Virtualization  
**Target Crates:** `causm-stdlib`, `causm-runtime`, `causm-frontend`, `causm-analysis`

---

## 1. Executive Summary

Causm has evolved from an experimental temporal interpreter into a self-hosted, multi-platform language with native C-FFI, WebAssembly execution (`causm-cli.wasm`), `oxiz` SMT verification, and strict capability-bounded `isolate` envelopes.

However, the current Standard Library (`causm-stdlib`) exhibits architectural coupling:
1. **Host-FFI Rigidity:** Core modules (`std/fs`, `std/net`, `std/process`, `std/time`) assume direct access to POSIX dynamic libraries (`libc.so.6`), causing compilation failures when imported into restricted, untrusted `isolate` sandboxes or pure WebAssembly environments without host FFI permissions.
2. **Binary Capability Inflexibility:** Isolates are forced into an "all-or-nothing" security model: either grant full `System.FFI` access (destroying security guarantees) or lose access to essential utility functions.
3. **Platform Fragmentation:** Running Causm across native Linux, Android Termux, WebAssembly runtimes (Wasmtime, browser), and bare-metal microcontrollers currently requires separate build workarounds.

This proposal formalizes the **Universal Standard Library Architecture (Stdlib 2.0)**:
* **The Tri-Tiered I/O Dispatch Model:** Transparent fallbacks across **Native C-FFI**, **WASI System Calls**, and **In-Memory Virtual Providers**.
* **Isolate-Native Capability Contracts:** Explicit `requires <Capability>` annotations on all privileged stdlib routines with compile-time reachability tree-shaking.
* **Unified Generic Collections & Core Types:** Standardizing `<T>` and `::<T>` across all containers (`Buffer<T>`, `Queue<T>`, `RingBuffer<T>`, `Option<T>`, `Result<T, E>`).
* **Zero-Trust Universal Safety:** Pure utilities run anywhere with zero capabilities; privileged routines gracefully degrade or redirect to virtual RAM backends.

---

## 2. The Tri-Tiered Standard Library Provider Architecture

The standard library decouples high-level user APIs from physical backend drivers through a **Three-Tier Capability Fallback Matrix**:

```
+──────────────────────────────────────────────────────────────────────────+
|                    Causm User Code & Stdlib High-Level API               |
|      (std/fs, std/net, std/time, std/process, std/json, std/collection)  |
+──────────────────────────────────────────────────────────────────────────+
                                     │
                 ┌───────────────────┼───────────────────┐
                 ▼                   ▼                   ▼
       [ Tier 1: Native FFI ]  [ Tier 2: WASI ]   [ Tier 3: Virtual ]
       (require System.FFI)   (require System.WASI)(Zero-Cap / Virtual)
                 │                   │                   │
                 ▼                   ▼                   ▼
       ┌──────────────────┐ ┌──────────────────┐ ┌──────────────────┐
       │ POSIX libc.so.6  │ │ WASI Snapshot    │ │ Pure In-Memory   │
       │ (Linux, Android, │ │ Preview 1        │ │ Arena Provider   │
       │  macOS, Windows) │ │ (Wasmtime, Web)  │ │ (Sandbox, MCU)   │
       └──────────────────┘ └──────────────────┘ └──────────────────┘
```

### 2.1 The Three Execution Tiers

| Tier | Driver Engine | Capability Requirement | Target Environment |
| :--- | :--- | :--- | :--- |
| **Tier 1: Native POSIX FFI** | Dynamic `dlopen`/`dlsym` direct register calls | `require System.FFI` | Host Linux, Android Termux, High-Performance Servers |
| **Tier 2: WASI System Calls** | Standard WASI Preview-1 imports (`fd_write`, `sock_accept`) | `require System.WASI` | Wasmtime, Wasmer, Node.js WASI, Cloudflare Workers |
| **Tier 3: Virtual In-Memory** | Pure Causm arena scratchpad, RAM-disks, virtual timers | *None* (or `require System.Virtual`) | Untrusted `isolate` sandboxes, Browser Web, Bare-Metal MCUs |

---

## 3. Module-by-Module Capability & WASM Compatibility Matrix

Every standard library module is categorized into **Pure Utilities** (zero capabilities required) and **Privileged Services** (tiered capability fallbacks).

```
causm-stdlib/csm/std/
├── core/            [PURE]     -> Option<T>, Result<T, E>, traits, monads
├── encoding/        [PURE]     -> utf8, base64, binary (BE/LE packing), hex
├── collection/      [PURE]     -> Buffer<T>, Queue<T>, RingBuffer<T>, Stack<T>, BitSet
├── json/            [PURE]     -> JsonValue ADT, recursive parser, serializer
├── path/            [PURE]     -> path_join, basename, dirname, extension normalization
├── sync/            [PURE/ISO] -> Atomic<T>, Mutex, SyncChannel<T>, CAS operations
├── time/            [TIERED]   -> Native clock_gettime <-> WASI clock_time_get <-> Virtual Tick
├── fs/              [TIERED]   -> POSIX open/read/write <-> WASI fd_* <-> Virtual RAMDisk
├── net/             [TIERED]   -> POSIX Berkeley sockets <-> WASI Sockets <-> In-Memory Channels
└── process/         [TIERED]   -> POSIX fork/pipe/kill <-> WASI proc_exit <-> Virtual Mock Proc
```

### 3.1 Tiered Execution Contracts in Action

#### 1. `std/time` (Universal Clock & Duration)
```causm
routine Time.now() -> Instant taking _ {
    if capability(System.FFI) {
        // Tier 1: Host POSIX Monotonic Clock (nanosecond precision)
        let ts = ffi_call "libc.so.6":clock_gettime(1)
        return Instant { secs = ts.tv_sec, nanos = ts.tv_nsec }
    } else if capability(System.WASI) {
        // Tier 2: WASI Monotonic Clock
        let time_ns = wasi_call "wasi_snapshot_preview1":clock_time_get(1, 1000)
        return Instant { secs = time_ns / 1000000000, nanos = time_ns % 1000000000 }
    } else {
        // Tier 3: TVM Logical Clock (Virtual Isolate Time)
        let logical_ms = tvm_clock_now()
        return Instant { secs = logical_ms / 1000, nanos = (logical_ms % 1000) * 1000000 }
    }
}
```

#### 2. `std/fs` (Universal File System & Storage)
```causm
routine File.open_readonly(path: string) -> FileResult<File> taking _ {
    if capability(System.FFI) {
        let fd = ffi_call "libc.so.6":open(path, 0, 0)
        if (fd < 0) { return FileResult::Err("FileNotFound") }
        return FileResult::Ok(File { fd = fd, path = path, is_virtual = false })
    } else if capability(System.VirtualFS) {
        // Sandboxed In-Memory Virtual RAMDisk
        let v_handle = VirtualFS.open_file(path)
        return FileResult::Ok(File { fd = v_handle.id, path = path, is_virtual = true })
    } else {
        return FileResult::Err("PermissionDenied: Missing System.FFI or System.VirtualFS")
    }
}
```

#### 3. `std/net` (Universal Socket & Networking)
```causm
routine TcpStream.connect(ip: string, port: i32) -> NetResult<TcpStream> taking _ {
    if capability(System.FFI) {
        let fd = create_posix_socket(ip, port)
        return NetResult::Ok(TcpStream { fd = fd, peer_ip = ip, peer_port = port, is_virtual = false })
    } else if capability(System.VirtualNet) {
        // In-memory IPC loopback channel
        let v_sock = VirtualNet.connect(ip, port)
        return NetResult::Ok(TcpStream { fd = v_sock.id, peer_ip = ip, peer_port = port, is_virtual = true })
    } else {
        return NetResult::Err("NetworkRestricted: Isolate lacks System.FFI capability")
    }
}
```

---

## 4. FFI Architecture Revamp & Auto-Drop Lifecycles

To ensure `auto_drop` works seamlessly across host C, WASM/WASI, and sandboxed environments without memory or handle leaks, we introduce **Polymorphic Handle Finalization**:

### 4.1 Polymorphic `auto_drop` Specification
```causm
type File = struct decay_after 5000ms auto_drop {
    fd: i32,
    path: string,
    is_virtual: bool
}
```

When the TVM triggers finalization for an `auto_drop` struct:
1. **If `is_virtual == true`:** Returns handle back to the isolate's virtual resource pool without making system calls.
2. **If `capability(System.FFI)`:** Invokes host `libc.close(fd)`.
3. **If `capability(System.WASI)`:** Invokes `wasi_snapshot_preview1::fd_close(fd)`.

---

## 5. Generic & Monomorphic Type Signatures across Stdlib

All standard library data structures adopt generic type parameters (`<T>`) and monadic error handling.

### 5.1 Standard Monadic Types (`std/core`)

```causm
enum Option<T> {
    Some(T),
    None
}

enum Result<T, E> {
    Ok(T),
    Err(E)
}

routine Option<T>.is_some(peek self) -> bool taking _ =>
    match self {
        Option::Some(_) => true,
        Option::None => false
    }

routine Option<T>.unwrap_or(peek self, default_val: T) -> T taking _ =>
    match self {
        Option::Some(v) => clone(v),
        Option::None => default_val
    }

routine Result<T, E>.is_ok(peek self) -> bool taking _ =>
    match self {
        Result::Ok(_) => true,
        Result::Err(_) => false
    }

routine Result<T, E>.unwrap(peek self) -> T taking _ =>
    match self {
        Result::Ok(v) => clone(v),
        Result::Err(_) => panic("Attempted to unwrap an Err Result")
    }
```

### 5.2 Generic Collections (`std/collection`)

```causm
type Buffer<T> = struct {
    data: [T],
    len: i32,
    capacity: i32
}

type RingBuffer<T> = struct {
    data: [T],
    head: i32,
    tail: i32,
    size: i32,
    capacity: i32
}

type Queue<T> = struct {
    data: [T],
    head: i32,
    tail: i32,
    len: i32,
    capacity: i32
}

type Stack<T> = struct {
    data: [T],
    len: i32,
    capacity: i32
}
```

---

## 6. Formal Call-Graph Reachability & Static Solver Rules

To enforce zero-trust security while maintaining a unified standard library, `causm-analysis` implements **Static Reachability Analysis & Capability Gating**:

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
|  4. Static Capability Verification (oxiz & Z3):             |
|     - Verify every reachable routine satisfies cap stack   |
|     - Statically prove fallback branches are sound          |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                      causm-runtime (TVM)                    |
|  - Enforce isolate resource limits (CPU, Memory Arena)      |
|  - Dispatch Tier 1 / Tier 2 / Tier 3 I/O providers          |
|  - Perform O(1) Epoch Arena compaction                      |
+-------------------------------------------------------------+
```

### 6.1 Formal Solver Rules

1. **Reachable Capability Subset Invariant:**
   For any routine $R$ called within isolate $I$:
   $$\forall R \in \text{Reachable}(I), \quad \text{RequiredCaps}(R) \subseteq \text{GrantedCaps}(I)$$
   If $R$ requires a capability not granted to $I$, compilation fails with `SemanticErrorKind::MissingCapability`.

2. **Guarded Branch Exception Rule:**
   If a call to privileged routine $R_{\text{priv}}$ is guarded by `if capability(C)`:
   $$\text{PathCondition} \implies \text{ActiveCap}(C)$$
   `oxiz` proves that the branch calling $R_{\text{priv}}$ is dead in unprivileged isolates, allowing static analysis to succeed.

---

## 7. End-to-End Real-World Showcases

### 7.1 Showcase 1: Untrusted Worker with Pure Stdlib Imports

An untrusted isolate imports `std/fs` and `std/collection` with **zero FFI capabilities**. Pure utilities run cleanly; privileged operations fall back to virtual RAM drivers.

```causm
import "std/fs" as fs
import "std/collection" as Collection
import "std/encoding/utf8" as Utf8

@0ms: {
    isolate untrusted_sandbox {
        enable memory(64KB)
        enable cpu(500ms)
        require System.Log

        // 1. Pure Path Utilities (Requires ZERO capabilities)
        let normalized = fs.path_join("/var/log", "telemetry.dat")
        let ext = fs.extension(normalized)
        print(f"Path: {normalized} Ext: {ext}")

        // 2. Generic In-Memory Collection (Requires ZERO capabilities)
        let mut telemetry_buf = Collection.Buffer<u8>::new(1024)
        telemetry_buf = telemetry_buf.append_slice(b"SANDBOX_PAYLOAD_001", 19)

        // 3. Graceful Capability Branching (Auto-degrades to RAM storage)
        if capability(System.FFI) {
            let f = fs.create_file(normalized)
            f.write_all(telemetry_buf.as_slice(), 19)
        } else {
            print(f"[Sandbox Fallback] Retained {telemetry_buf.len()} bytes in arena RAM.")
        }
    }
}
```

### 7.2 Showcase 2: WebAssembly & WASI Dual-Target Server

The exact same Causm source code runs natively on Linux (Tier 1 POSIX FFI) and in WebAssembly via Wasmtime (Tier 2 WASI):

```causm
import "std/net" as Net
import "std/time" as Time
import "std/encoding/utf8" as Utf8

@0ms: {
    isolate edge_microserver {
        enable memory(128KB)
        enable cpu(5000ms)
        require System.Log
        require System.WASI
        require System.FFI

        let start_time = Time.now()
        using listener = Net.TcpListener.bind(8080) {
            print(f"Microserver listening on port 8080 (Booted at {start_time.secs}s)")

            @every 16ms: {
                if let client = listener.try_accept() {
                    using conn = client {
                        let rx = conn.recv(512)
                        let resp = f"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello Causm!"
                        conn.send(Utf8.encode(resp))
                    }
                }
            }
        }
    }
}
```
---

## 8. Implementation Roadmap

| Milestone | Target Deliverable | Description |
| :--- | :--- | :--- |
| **Phase 1** | **Grammar & AST Upgrades** | Add `requires <Cap>`, `capability(<Cap>)`, and `<T>` generics to `causm.pest` and AST. |
| **Phase 2** | **Reachability Call-Graph Analyzer** | Update `causm-analysis` to prune uncalled FFI routines in isolates and verify capability subtyping. |
| **Phase 3** | **Stdlib Tri-Tier Provider Migration** | Refactor `std/fs`, `std/net`, `std/time`, `std/process` with Tier 1 (FFI), Tier 2 (WASI), and Tier 3 (Virtual) dispatch paths. |
| **Phase 4** | **Polymorphic `auto_drop` Engine** | Update TVM to route finalizers to `libc.close`, `wasi.fd_close`, or virtual drop based on handle metadata. |
| **Phase 5** | **WASM / WASI CI & Test Suite** | Add automated CI pipelines validating the entire stdlib test suite under native Linux, Termux Android, and Wasmtime WASI. |

---

## 10. Conclusion

This proposal establishes a **fully unified, platform-independent, capability-secure Standard Library for Causm.**

By combining **Tiered I/O Providers** with **Compile-Time Reachability Tree-Shaking**, Causm eliminates stdlib fragmentation forever. A single `.csm` codebase runs seamlessly across bare-metal microcontrollers, untrusted isolate sandboxes, high-performance Linux servers, and client-side WebAssembly browsers with **zero code changes and mathematically verified security.**
