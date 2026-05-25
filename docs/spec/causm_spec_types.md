# Specification: Entropic Type System

## 1. Core Principles
The Causm type system ensures that data lifecycle is coupled with temporal execution. Every type must define its behavior under **Entropic Decay**.

## 2. Scalar Types

### 2.1 Integers (`int`)
- Represented as 64-bit signed integers.
- Baseline operation cost: 1ms.

### 2.2 Floats (`float`)
- Represented as IEEE-754 64-bit double-precision floats.
- **Numeric Promotion**: Integers are automatically promoted to floats when used in mixed-type arithmetic or comparisons.
- Baseline operation cost: 1ms.

## 3. Entropic Lifecycle
Variables exist in one of the following states:
- **Valid**: Fully accessible for move, clone, or peek.
- **Leased**: Bound by a `Temporal Lease`; read-only access granted, moves prohibited.
- **Decayed**: A parent structure whose field has been consumed.
- **Consumed**: A terminal state; no further access allowed.

## 4. Temporal Type Contracts
Contracts embed execution constraints directly into the type definition, enforced by the Z3 Correctness Kernel.

### 4.1 `PacedIterable<T, MaxTime>`
A collection type that guarantees iteration over any member will not exceed `MaxTime`.
- **Enforcement**: Routines consuming this type are rejected if they contain operations exceeding the WCET bound per element.

### 4.2 `ConstantAccess<T, Time>`
Guarantees O(1) temporal cost for field or index access.
- **Enforcement**: The TVM ensures no dynamic resizing or hashing jitter occurs during access.

## 5. Structural Decay
Accessing a field of a `struct` or an index of an `array` results in the decay of the parent container.

```causm
type Point = struct { x: int, y: int }
let p = Point{x: 10, y: 20}
let val = p.x // 'p' is now Decayed. 'p.y' is still Valid.
```
- A decayed structure cannot be moved or passed to a `consume` parameter.
- The parent structure is reclaimed only when all constituent fields reach the `Consumed` state.
