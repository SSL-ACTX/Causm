# Causm Module System and Code Import Specification

This document provides the formal specification for the Causm Module System, code import protocols, and entropic namespace resolution across `.csm` files.

---

## 1. Overview & Architecture

The Module System allows Causm programs to be decomposed across multiple `.csm` source files while guaranteeing that timeline isolation, manifest capabilities, worst-case execution time (WCET) bounds, and Z3 symbolic safety proofs are fully preserved across module boundaries.

---

## 2. Formal Import Syntax

Causm supports two top-level import statement forms:

### 2.1 File-Level Imports

Imports an entire `.csm` file into a named namespace.

```causm
import "<relative_path>.csm" as <Alias>
```

**Example:**
```causm
import "sensor/driver.csm" as SensorLib
```

### 2.2 Named Symbol Imports

Imports specific routines, custom types, or state declarations directly into the calling scope.

```causm
from "<relative_path>.csm" import <Symbol> [as <Alias>]
```

**Example:**
```causm
from "module_sensor_lib.csm" import compute_telemetry_digest, SensorState as State
```

---

## 3. EBNF Grammar Specification

```ebnf
import_stmt       = "import", string_literal, [ "as", identifier ] ;
from_import_stmt  = "from", string_literal, "import", import_symbol_list ;
import_symbol_list= import_symbol, { ",", import_symbol }, [ "," ] ;
import_symbol     = identifier, [ "as", identifier ] ;
```

---

## 4. Capability & Manifest Inheritance Rules

1. **Manifest Scoping**: When code is imported inside an `isolate` block, the imported routines inherit the host isolate's capability manifest (`require System.Log`, `require System.NetworkFetch`).
2. **Missing Capability Enforcement**: Static analysis fails with `SemanticErrorKind::MissingCapability` if an imported routine requires a capability that has not been explicitly declared by the importing isolate block.

---

## 5. Parameter Intent & Entropic Invariants

All exported routines retain their explicit parameter lifecycle intent:
- `peek`: Non-consuming reference access.
- `consume`: Ownership transfer across file boundaries.
- `lease`: Time-bounded resource leases.
- `decay`: Entropic state decay triggering.

---

## 6. Dependency Graph & Cycle Detection

The Causm compiler constructs a directed acyclic graph (DAG) of file dependencies during parsing. If a circular import loop is detected (`A.csm` $\rightarrow$ `B.csm` $\rightarrow$ `A.csm`), static analysis halts immediately with `SemanticErrorKind::CircularImport`.
