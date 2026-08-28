# Proposal: WebAssembly-Based Compiler Plugin System, Extensible AST Transformations, and Developer SDK (`causm-plugins`)

**Status:** Proposed  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Compiler Architecture, Metaprogramming & Tooling Extensibility  
**Target Crates:** `causm-plugins` (New), `causm-plugin-sdk` (New), `causm-core`, `causm-cli`, `causm-frontend`

---

## 1. Executive Summary & Motivation

As modern programming languages mature, compilers inevitably face the **Compiler Bloat Dilemma**:
* **The Monolithic Pitfall:** Baking domain-specific security rules (e.g. Linux seccomp/BPF), custom decorators, code generators, and multi-target transpilers directly into the compiler binary causes unbounded codebase bloat, balloons binary sizes, and slows down compile-time feedback loops.
* **The Fragile Alternative:** Lacking an extensible compiler engine forces teams to rely on fragile regex scripts, external preprocessors, or unverified macro copy-pasting.

This proposal establishes the architecture, runtime ABI, safety model, and developer SDK for the **Causm WebAssembly & IPC Compiler Plugin System (`causm-plugins`)**.

Rather than requiring dangerous native dynamic libraries (`.so`, `.dylib`) or heavy C++ compiler hooks, Causm plugins execute inside a **sandboxed, deterministic WebAssembly interpreter (`wasmi`)** or over a **standardized JSON-over-Stdio IPC protocol**. Plugins can inspect, lint, and rewrite Abstract Syntax Trees (AST) at compile time without compromising compiler speed, memory safety, or multi-platform portability.

---

## 2. Core Architecture & Compilation Pipeline

The plugin engine executes as an explicit, sandboxed pass positioned immediately after parsing and before formal SMT analysis.

```
                     ┌───────────────────────────┐
                     │     Source Code (.csm)    │
                     └─────────────┬─────────────┘
                                   │
                                   ▼  1. causm-frontend (Pest Parser)
                     ┌───────────────────────────┐
                     │     Raw AST (Program)     │
                     └─────────────┬─────────────┘
                                   │
                                   ▼  2. causm-plugins Engine (wasmi / Stdio)
       ┌──────────────────────────────────────────────────────────────────┐
       │ For each plugin registered in `causm.toml`:                      │
       │  1. Serialize AST -> Bincode (WASM) or JSON (Stdio Pipe)         │
       │  2. Dispatch to Plugin Execution Sandbox:                        │
       │     - In-Memory WASM: execute via `wasmi` (Max 16MB / 100ms)     │
       │     - IPC Subprocess: pipe to plugin binary `stdin` / `stdout`   │
       │  3. Validate returned AST against Structural Integrity Rules     │
       │  4. Ingest compiler diagnostics, warnings, and error spans       │
       └──────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼  3. causm-analysis (oxiz / Z3 Solver)
                     ┌───────────────────────────┐
                     │  Formally Verified AST    │
                     │  (Proves plugin-generated │
                     │   code satisfies all laws)│
                     └─────────────┬─────────────┘
                                   │
                                   ▼  4. causm-ir -> causm-runtime (TVM)
                     ┌───────────────────────────┐
                     │   SSA IR & Execution VM   │
                     └───────────────────────────┘
```

### 2.1 The Critical Safety Invariant (Zero Implicit Trust)
Because the plugin execution layer sits strictly **before** `causm-analysis`:
$$\forall \text{AST}_{\text{plugin}} \in \text{Transform}(\text{AST}), \quad \text{Verify}(\text{AST}_{\text{plugin}}) \implies \text{Valid}$$

**Code generated or modified by third-party plugins is NEVER trusted implicitly.** 
Every synthesized statement, newtype, temporal contract (`taking _`), and struct must pass 100% of Causm's formal SMT proofs and entropic memory safety checks. A malicious, buggy, or hallucinating plugin **cannot bypass language laws or produce undefined behavior.**

---

## 3. Dual Execution Engine: In-Memory WASM & Stdio IPC

To ensure both maximum performance and polyglot developer freedom, `causm-plugins` supports a **Dual-Engine Model**:

```
                              ┌───────────────────────────┐
                              │  causm-plugins Dispatcher │
                              └─────────────┬─────────────┘
                                            │
                     ┌──────────────────────┴──────────────────────┐
                     ▼                                             ▼
        [ In-Process WASM Engine ]                    [ Stdio Subprocess IPC ]
        - Engine: `wasmi` interpreter                 - Protocol: JSON over stdin/stdout
        - Artifact: `plugin.wasm`                     - Artifact: Any executable binary / script
        - Speed: Microsecond execution                - Freedom: Write plugins in Python, Go,
        - Sandbox: Strict 16MB RAM / no disk                     TypeScript, Rust, or Bash!
```

### 3.1 In-Process WebAssembly Engine (`wasmi`)
* **Pure Rust:** `wasmi` has zero C/C++ dependencies, preserving Causm's ability to cross-compile to Android (Termux) and WASM without toolchain headaches.
* **Lightweight:** Adds less than 450 KB to the `causm-cli` binary.
* **Deterministic Sandbox:** Enforces strict fuel/gas bounds (max 100ms CPU runtime) and a 16 MB linear memory ceiling per plugin.

### 3.2 In-Memory WASM ABI Specification
Communication between the host compiler and `.wasm` plugins operates via a shared-memory buffer protocol:

```rust
// Exported by plugin.wasm:
extern "C" {
    /// Allocates `len` bytes inside plugin linear memory for payload staging
    pub fn causm_plugin_alloc(len: u32) -> *mut u8;
    
    /// Deallocates memory slice after payload transfer
    pub fn causm_plugin_dealloc(ptr: *mut u8, len: u32);
    
    /// Core hook: processes AST payload and returns packed `(out_ptr << 32) | out_len`
    pub fn causm_plugin_transform(ptr: *mut u8, len: u32) -> u64;
}
```

### 3.3 Stdio IPC Protocol (The `protoc` Model)
For developers writing plugins in Python, Go, or Node.js, `causm-cli` spawns the plugin as a subprocess and exchanges data via standard I/O streams:
* **`stdin`**: Receives `PluginRequest` JSON payload.
* **`stdout`**: Emits `PluginResponse` JSON payload.
* **`stderr`**: Forwarded directly to compiler logs for debugging.

---

## 4. The Unified Plugin Protocol (`PluginRequest` / `PluginResponse`)

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginRequest {
    pub protocol_version: String,
    pub compiler_version: String,
    pub file_path: String,
    pub ast: Program,
    pub options: HashMap<String, TomlValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginResponse {
    pub status: PluginStatus,
    pub modified_ast: Option<Program>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginDiagnostic {
    pub level: DiagnosticLevel, // Error, Warning, Note
    pub message: String,
    pub span: Option<Span>,
}
```

---

## 5. The Developer SDK (`causm-plugin-sdk`)

To make writing plugins in Rust completely painless, we introduce the `causm-plugin-sdk` crate.

### 5.1 Authoring a Plugin in Rust

```rust
// plugins/seccomp_guard/src/lib.rs
use causm_plugin_sdk::prelude::*;

#[causm_plugin(name = "seccomp_guard", version = "0.1.0")]
pub fn process_ast(mut program: Program, ctx: &PluginContext) -> Result<Program, PluginError> {
    for stmt in &mut program.statements {
        if let Statement::Isolate(isolate) = stmt {
            // Check if isolate has the custom @seccomp attribute
            if let Some(attr) = isolate.attributes.iter().find(|a| a.name == "seccomp") {
                let allowed_syscalls = attr.get_string_list("allow")?;

                // Audit all syscall statements inside this isolate
                for inner in &isolate.body {
                    if let Statement::Syscall(sys) = inner {
                        if !allowed_syscalls.contains(&sys.target_name) {
                            return Err(PluginError::diagnostic(
                                DiagnosticLevel::Error,
                                format!("Syscall '{}' violates @seccomp policy!", sys.target_name),
                                sys.span,
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(program)
}
```

### 5.2 Compiling the Plugin
```bash
cargo build --target wasm32-wasip1 --release
# Produces: target/wasm32-wasip1/release/seccomp_guard.wasm (Ready to distribute!)
```

---

## 6. Project Configuration: `causm.toml`

Projects declare compiler plugins declaratively in their root configuration file:

```toml
# causm.toml
[package]
name = "autonomous_flight_system"
version = "0.1.0"
authors = ["Seuriin <seuriin@gmail.com>", "Iris Seravelle <iris.seravelle@gmail.com>"]

[dependencies]
std = "0.1.0"

[plugins]
# 1. In-process WASM Plugin (Fast & Sandboxed)
seccomp_guard = { path = "./plugins/seccomp_guard.wasm" }

# 2. Remote Registry WASM Plugin (Automatic download & checksum verification)
trace_viz = { url = "https://pkg.causm.network/plugins/trace_viz.wasm", sha256 = "b7a2..." }

# 3. Polyglot Stdio Subprocess Plugin (e.g. Python linter)
schema_validator = { command = "python3 ./scripts/validate_schema.py" }

[plugins.seccomp_guard.options]
strict_mode = true
default_deny = true
```

---

## 7. Flagship Showcase: Linux `@seccomp` Syscall Guard Plugin

### 7.1 Source Code (`examples/plugin_seccomp_showcase.csm`)

```causm
@0ms: {
    // Custom @seccomp attribute evaluated and verified by `seccomp_guard.wasm`
    @seccomp(allow=["sys_read", "sys_write", "sys_exit"])
    isolate telemetry_worker {
        enable memory(64KB)
        enable cpu(500ms)
        require System.Syscall
        require System.Log

        let log_msg = "Telemetry packet verified.\n"
        
        // ✅ Permitted by plugin:
        let written = syscall("sys_write", 1, log_msg, len(log_msg)) taking 2ms
        print(f"Wrote {written} bytes securely.")

        // ❌ COMPILE-TIME PLUGIN ERROR:
        // let pid = syscall("sys_getpid") taking 1ms
        // --> Error: Syscall 'sys_getpid' violates @seccomp policy!
    }
}
```

---

## 8. Plugin Hook Points & Extensibility Matrix

| Hook Name | Invocation Point | Input / Output | Primary Use Case |
| :--- | :--- | :--- | :--- |
| **`on_ast_transform`** | Post-Pest Parsing | `Program` $\to$ `Program` | Procedural macros, code synthesis, `@derive`, DSLs |
| **`on_lint_validate`** | Pre-Analysis Pass | `Program` $\to$ `Vec<Diagnostic>` | Security guards (`seccomp`, MISRA-C compliance, linters) |
| **`on_ir_emit`** | Post-SSA Optimization | `SsaCFG` $\to$ `Vec<u8>` | Custom transpilers (`causm-to-c`, `causm-to-wasm`, Verilog) |

---

## 9. Implementation Roadmap

| Phase | Milestone | Deliverable |
| :--- | :--- | :--- |
| **Phase 1** | **AST Serialization** | Derive `Serialize`/`Deserialize` across all AST types in `causm-core` with Bincode and JSON. |
| **Phase 2** | **Stdio IPC Driver** | Add `--plugin=<cmd>` support in `causm-cli` using stdin/stdout JSON streaming. |
| **Phase 3** | **`wasmi` Host Engine** | Implement `causm-plugins` crate with embedded WebAssembly execution and memory ABI. |
| **Phase 4** | **Developer SDK** | Publish `causm-plugin-sdk` with `#[causm_plugin]` procedural macro. |
| **Phase 5** | **`causm.toml` Discovery** | Integrate declarative `[plugins]` section parsing into project builds. |
| **Phase 6** | **Flagship Showcase** | Build and test `seccomp_guard.wasm` with full unit and integration tests. |

---

## 10. Conclusion

The `causm-plugins` architecture gives Causm the unlimited metaprogramming power of Babel and Rust procedural macros, without binary bloat, platform build matrix hell, or security risks. By isolating compiler plugins inside a fast WebAssembly sandbox and subjecting all generated code to mathematical SMT proofs, Causm achieves infinite extensibility while preserving 100% formal correctness.
