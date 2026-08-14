# Proposal: Advanced Entropic Object-Oriented Programming (Protocols, Traits & Behavioral Inheritance)

**Status:** Approved & Implemented  
**Author:** Seuriin <seuriin@gmail.com>  
**Category:** Language Core & OOP Semantics  

---

This document outlines the architecture for extending Causm's **Entropic Object Orientation** with Monomorphized Generic Method Dispatch, Associated Lifecycle Types, and Entropic Interface Constraints.

---

## Architecture Overview

### 1. Polymorphic Generic Method Monomorphization
Enables generic routines and methods (`routine Vault<T>.process(...)`) to be monomorphized at compile-time into specialized intermediate representations:
- **Generic Instantiation**: Substitutes abstract type parameter `T` with concrete scalar or struct types (`int`, `float`, `SecurityNode`).
- **Arena Layout Specialization**: Allocates exact memory arena register byte sizes per specialized variant.

```causm
type Container<T: Consumable> = struct {
    value: T
}

routine Container<T>.take_inner(consume self) -> T taking 10ms {
    let inner = self.value
    yield inner
}
```

### 2. Associated Entropic Lifecycle Types & Interfaces
Allows interfaces to declare associated decay limits, entropic preconditions, and execution time bounds:

```causm
interface Streamable {
    type PayloadType: Consumable
    decay_after 500ms

    routine next(peek self) -> PayloadType taking 10ms
}
```

---

## Implementation Plan

### Step 1: Grammar & AST Extensions (`causm.pest`, `causm-frontend`)
- Extend `method_name` in `causm.pest` to accept generic type arguments: `identifier ~ ("<" ~ type_param_list ~ ">")? ~ ("." ~ identifier)?`.
- Extend `interface_decl` to parse associated `type` declarations and `decay_after` parameters.

### Step 2: Analyzer Monomorphization & Type Bounds Checking (`causm-analysis`)
- Implement generic type instantiation table in `EntropicAnalyzer`.
- Verify `Consumable` and `Leasable` bounds on generic arguments during interface assignment and routine calls.

### Step 3: VM Monomorphized Dispatch (`causm-runtime`)
- Specialize `routine` resolution table in VM to look up `Struct<T>.method` specialization symbols.
