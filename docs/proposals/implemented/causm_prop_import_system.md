# Proposal: Module System, Code Import Protocols, and Entropic Namespace Resolution

**Status:** Approved & Implemented  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Modular Systems & Compilation Pipeline  

---

This document specifies the formal requirements for modularizing Causm programs across multiple `.csm` source files. It introduces the **Module System**, the **Import Protocol**, and **Entropic Capability Propagation** to enable modular software composition without compromising timeline isolation, formal Z3 verification, or entropic state determinism.

---

## 1. The Modular Composition Challenge in Isochronous Runtimes

In Causm, every routine, custom type, and state variable operates within formal WCET budgets, timeline branches, and capability manifests. Traditional module import mechanisms (like C `#include` or JS `import`) statically flatten or dynamically evaluate scripts, which breaks Causm's core guarantees:
- **Capability Violations**: Importing code must not bypass isolate manifests or silently grant unauthorized system capabilities (`System.NetworkFetch`, `System.Log`).
- **Entropic Invariant Safety**: Imported routines and structs must declare their lifetime annotations and entropic decay behaviors.
- **Isochronous Timeline Isolation**: Module initialization must be deterministically accounted for in worst-case time bounds.

---

## 2. Syntax & Grammar Specifications

The Pest grammar is extended to support top-level `import` declarations:

### 2.1 File-Level & Named Imports

```causm
// Import entire module into a named namespace
import "sensor/driver.csm" as SensorDriver

// Named symbol imports with alias
from "crypto/vault.csm" import Vault, DecryptKey as Key
```

### 2.2 Grammar Definition (`causm.pest`)

```pest
import_stmt = { "import" ~ string_literal ~ ("as" ~ identifier)? }
from_import_stmt = { "from" ~ string_literal ~ "import" ~ import_symbol_list }
import_symbol_list = { import_symbol ~ ("," ~ import_symbol)* ~ ","? }
import_symbol = { identifier ~ ("as" ~ identifier)? }
```

---

## 3. Entropic & Capability Import Rules

### 3.1 Strict Isolate Manifest Inheritance
When a module is imported inside an `isolate` block:
1. The imported file's capability declarations are matched against the importing isolate's `manifest`.
2. If an imported module uses a capability (e.g. `System.NetworkFetch`) that the host isolate has not declared via `require`, static analysis fails with `SemanticErrorKind::MissingCapability`.

### 3.2 Module-Level Routine Contracts
Routines exported from imported modules preserve their formal parameter intent:
- `consume` parameters consume the caller's variable across file boundaries.
- `lease` parameters enforce temporal lease bounds.
- `@decay_rate` annotations propagate to imported variable definitions.

---

## 4. AST & Lowering Pipeline Integration

1. **Dependency Graph Construction**: The compiler parses imported `.csm` files recursively and detects circular import loops (`SemanticErrorKind::CircularImport`).
2. **Namespace Symbol Resolution**: Symbols are bound in symbol tables with fully qualified module names (e.g., `SensorDriver.read_telemetry`).
3. **WCET Accounting**: Module initialization statements add deterministic costs during static time analysis.

---

## 5. Formal Z3 Solver Verification Specification

The Z3 solver verifier in `causm-analysis` receives module-level assertions:
- **Temporal Invariants**: All imported routine WCET bounds (`taking Xms`) are statically added to calling block temporal budgets.
- **Entanglement Safety**: Entangled variables crossing module boundaries are verified for timeline parity.

---

### Architectural Significance
The Module System enables multi-file software engineering in Causm while strictly preserving its core invariants. By enforcing manifest inheritance, entropic parameter intent, and static dependency graph validation, Causm guarantees that multi-file programs remain fully verifiable, isochronous, and memory-safe.
