# Proposal: Causm Foreign Function Interface (FFI) and Low-Level Syscall Bridge

**Status:** Approved & Active Implementation  
**Author:** Causm Architecture Team  
**Category:** Language Core & Systems Integration  

---

## 1. Executive Summary

This proposal presents the architecture, formal syntax, safety model, and execution contracts for the **Causm Foreign Function Interface (FFI) and Native Syscall Bridge**. This system enables native OS system calls and external C dynamic library routines (`.so`, `.dylib`, `.dll`) to be directly bound, dynamically loaded, and safely executed within Causm `.csm` source files.

Rather than disabling Causm's mathematical verification laws when crossing foreign boundaries, this proposal establishes the **Entropic Wrapper & Capability Isolation Standard (Option C)**:
1. Native calls are sandbox-guarded by explicit capability manifests (`require System.FFI(...)` / `require System.Syscall(...)`).
2. Foreign return values and raw pointers are wrapped within Causm's entropic state machine (`Valid`, `Leased`, `Decayed`, `Consumed`), ensuring Z3 static solver verification remains 100% active for host code.
3. The Temporal Virtual Machine (TVM) executes foreign calls through a real dynamic symbol resolution pipeline (`dlopen`/`dlsym`/C trampoline) and direct architecture-aware OS system calls rather than hardcoded compiler simulations.

---

## 2. Safety Philosophy: Option C (Entropic Wrapper & Capability Isolation)

A central design decision of this proposal is refusing to allow `foreign` blocks to silently disable Causm's core laws. Unverified external C execution is reconciled with Causm's formal temporal and entropic invariants through three strict constraints:

1. **No Silent Laws Disabling**: External native routines cannot bypass Z3 solver checks or entropic decay rules implicitly. Bypassing Z3 is only permitted if the block explicitly specifies `directive chaos` or the compiler is run with `--no-z3`.
2. **Capability Sandbox Manifests**: Invoking a foreign library function requires an explicit `require System.FFI(lib="...", symbol="...")` capability declaration in the host `isolate` block. Unprivileged isolates attempting FFI or syscall execution fail static analysis with `SemanticErrorKind::MissingCapability`.
3. **Entropic Pointer Wrappers (`EntropicPtr<T>`)**: Raw native pointers (`*const T`, `*mut T`) and file descriptors are encapsulated in managed entropic wrappers. Z3 symbolically proves that Causm host code never dereferences, leaks, or uses a native handle after its entropic state transitions to `Decayed` or `Consumed`.

---

## 3. Formal Syntax and Grammar Specification

### 3.1 External Library Binding (`foreign`)

Declares external dynamic library routines with ABI specifications, capability requirements, and entropic parameter modes.

```causm
foreign "<library_name>" abi("<abi_name>") {
    [pub] routine <routine_name>(<params>) -> <return_type> taking <amount>ms
}
```

**Example:**
```causm
foreign "libc.so.6" abi("C") {
    pub routine write(peek fd: i32, peek buf: string, peek count: u64) -> i64 taking 2ms
    pub routine getpid() -> i32 taking 1ms
    pub routine getppid() -> i32 taking 1ms
    pub routine getuid() -> i32 taking 1ms
    pub routine getcwd() -> string taking 1ms
    pub routine close(consume fd: i32) -> i32 taking 1ms
}
```

### 3.2 Platform-Independent System Call Invocation (`syscall`)

Executes system calls using either target-specific numeric numbers or platform-independent symbolic syscall names (`"sys_write"`, `"sys_read"`, `"sys_getpid"`, `"sys_openat"`).

```causm
syscall((<sys_number> | <sys_symbol_string>), [<arg1>, <arg2>, ...]) taking <amount>ms
```

**Example:**
```causm
routine sys_write(peek fd: i32, peek msg: string) -> i64 taking 2ms {
    require System.Syscall(sys_id="sys_write")
    let len = msg.len()
    let result = syscall("sys_write", fd, msg, len) taking 2ms
    yield result
}
```

### 3.3 Pest EBNF Grammar

```ebnf
foreign_block     = "foreign", string_literal, "abi", "(", string_literal, ")", "{", foreign_routine*, "}" ;
foreign_routine   = pub_opt, "routine", identifier, "(", param_decl_list?, ")", "->", type_name, duration_limit ;
syscall_expr      = "syscall", "(", ( integer_literal | string_literal ), [ ",", expression* ], ")", duration_limit? ;
```

---

## 4. Automatic Entropic Cleanup & Resource Reclamation

External C allocations, sockets, and raw file handles bound through FFI are governed by Causm's Entropic Pointer Wrappers with declarative `auto_drop` attributes or manual `on_decay` handlers:

### 4.1 Declarative `auto_drop` Attributes

Custom types holding raw FFI handles can declare automatic reclamation rules:

```causm
type NativeHandle = struct decay_after 1000ms auto_drop("libc.so.6", "close", fd) {
    fd: i32
}
```

Upon entropic decay, the TVM automatically invokes `close(fd)` without requiring boilerplate code.

### 4.2 Manual Entropic Decay Handlers (`on_decay`)

```causm
type EntropicBuffer = struct decay_after 500ms {
    ptr: i64,
    size: u64
}

on_decay(buffer) {
    foreign "libc.so.6" abi("C") {
        routine free(consume ptr: i64) -> null taking 1ms
    }
    call free(buffer.ptr)
}
```

### Entropic State Transitions for Native Memory:
- **Valid**: Buffer allocated; read/write access permitted.
- **Leased**: Borrowed by an FFI call for duration `Nms`; caller cannot free or move buffer during lease.
- **Decayed**: Age exceeds `decay_after`; automatic `auto_drop` or `on_decay` handler triggers cleanup.
- **Consumed**: Pointer passed to a `consume` parameter in a native `free()` wrapper; future access is rejected by Z3 static verification.

---

## 5. Security Sandboxing & Path Restrictions

To protect isolate boundaries from malicious dynamic library loading, FFI declarations are validated against strict path sandbox policies:

```causm
isolate sandboxed_driver {
    enable cpu(5000ms)
    require System.FFI(
        lib="libc.so.6",
        allowed_paths=["/lib", "/usr/lib", "/system/lib64", "/data/data/com.termux/files/usr/lib"]
    )

    foreign "libc.so.6" abi("C") {
        routine getpid() -> i32 taking 1ms
    }
}
```

If an isolate attempts to load an dynamic library outside the `allowed_paths` whitelist, static analysis fails immediately with `SemanticErrorKind::ForbiddenLibraryPath`.

---

## 6. TVM Syscall & FFI Execution Engine Architecture

When lowered to the Register-based Temporal Virtual Machine (TVM), FFI and Syscall operations map to dedicated TVM instructions with dynamic symbol resolution:

```
Lowered AST -> IR Routine with ForeignBinding { lib_name, abi, symbol }
            -> IR Instruction: Syscall { sys_target, args, dest }
```

### TVM Dynamic FFI Execution Flow:
1. **Capability & Path Inspection**: The TVM verifies that the active `isolate` block holds `System.Syscall` or `System.FFI` permissions matching target library path and symbol name.
2. **Dynamic Symbol Resolution (`dlopen`/`dlsym`)**: The TVM maintains a thread-safe `ForeignLibraryManager`. On the first invocation of a foreign routine, it resolves the shared object library and obtains the raw function pointer.
3. **C Type Marshalling & Trampoline**: Causm `Payload` values are marshalled into C ABI types:
   - `Payload::Integer(i)` -> C `int` / `int64_t` / `uint64_t`
   - `Payload::Float(f)` -> C `double` / `float`
   - `Payload::String(s)` -> C `*const c_char` (temporary null-terminated buffer)
   - `Payload::Bool(b)` -> C `uint8_t`
4. **Direct Architecture-Aware Syscall Dispatch**:
   - Platform-independent symbols (`"sys_write"`, `"sys_read"`, `"sys_getpid"`) are translated to native OS kernel numbers (e.g. `1` on Linux x86_64, `64` for write on Linux AArch64).
   - Numeric and symbolic calls invoke direct OS syscall primitives via `libc::syscall(...)` with argument registers.
5. **Isochronous Time Padding**: The TVM advances the local branch clock by the declared `taking Nms` duration. If execution completes earlier, deterministic sleep padding is inserted to prevent side-channel timing leaks.
6. **Z3 Memory Verification**: The Z3 correctness kernel verifies at compile time that pointer arguments passed to FFI calls are in the `Valid` state and have sufficient arena lifetime.

---

## 7. Self-Hosted Standard Library Example (`std/fs.csm`)

```causm
@0ms: {
    isolate std_fs {
        enable cpu(10000ms)
        require System.Syscall(sys_id="sys_openat")
        require System.FFI(lib="libc.so.6")

        foreign "libc.so.6" abi("C") {
            pub routine open(peek path: string, peek flags: i32) -> i32 taking 3ms
            pub routine close(consume fd: i32) -> i32 taking 1ms
        }

        pub routine read_file_string(peek path: string) -> string taking 10ms {
            let fd = call open(path, 0)
            let buffer = "                                "
            let bytes_read = syscall("sys_read", fd, buffer, 32) taking 5ms
            let status = call close(fd)
            yield buffer
        }
    }
}
```

---

## 8. Resolution Summary

This proposal establishes a fully-specified, secure, platform-independent FFI and Syscall bridge for Causm. By combining Option C capability isolation with dynamic `dlopen`/`dlsym` resolution, raw OS syscall dispatch, `auto_drop` entropic cleanup attributes, and dynamic library path sandboxing, Causm standard libraries (`causm-stdlib`) can be authored entirely in self-hosted `.csm` code.
