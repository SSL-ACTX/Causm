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

### 2.3 Explicit Type Casting (`as`)
Values can be explicitly cast between numeric types using the `as` operator:
```causm
let val: f64 = 42 as f64
let int_val: i32 = 3.14159 as i32  // truncated to 3
let byte_val: u8  = 257 as u8      // truncated to 1 (wrapping)
```
Supported target types: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`.

The compiler validates that the source and target types are numerically compatible. Casting is a compile-time syntactic operation lowered to a `Cast` IR instruction and evaluated at runtime.

## 3. Entropic Lifecycle
Variables exist in one of the following states:
- **Valid**: Fully accessible for move, clone, or peek.
- **Leased**: Bound by a `Temporal Lease`; read-only access granted, moves prohibited.
- **Decayed**: A parent structure whose field has been consumed.
- **Consumed**: A terminal state; no further access allowed.
- **Pending**: Awaiting resolution of a causal dependency (e.g., channel receive).

## 4. Temporal Type Contracts
Contracts embed execution constraints directly into the type definition, enforced by the Z3 Correctness Kernel.

### 4.1 `PacedIterable<T, MaxTime>`
A collection type that guarantees iteration over any member will not exceed `MaxTime`.
- **Enforcement**: Routines consuming this type are rejected if they contain operations exceeding the WCET bound per element.

### 4.2 `ConstantAccess<T, Time>`
Guarantees O(1) temporal cost for field or index access.
- **Enforcement**: The TVM ensures no dynamic resizing or hashing jitter occurs during access.

## 4a. Generic Type Parameters

Structs and interfaces can declare generic type parameters with entropic bounds:

```causm
type Container<T: Consumable> = struct {
    value: T
}

routine Container<T>.take_inner(consume self) -> T taking 10ms {
    let inner = self.value
    yield inner
}
```

### Supported Type Bounds
| Bound | Meaning |
|-------|---------|
| `Consumable` | `T` can be consumed (moved, not just peeked) |
| `Leasable` | `T` supports temporal lease borrowing |

Generic methods are **monomorphized at compile time**: the compiler produces a specialized IR and VM dispatch entry for each concrete instantiation (e.g., `Container<int>`, `Container<float>`).

## 5. Structural Decay
Accessing a field of a `struct` or an index of an `array` results in the decay of the parent container.

```causm
type Point = struct { x: int, y: int }
let p = Point{x: 10, y: 20}
let val = p.x // 'p' is now Decayed. 'p.y' is still Valid.
```
- A decayed structure cannot be moved or passed to a `consume` parameter.
- The parent structure is reclaimed only when all constituent fields reach the `Consumed` state.

## 5a. Array Broadcasting

Binary arithmetic operators (`+`, `-`, `*`, `/`, `%`) automatically broadcast when one or both operands are arrays:

```causm
// Scalar-to-array: scalar is broadcast across each element
let arr: array<int> = [1, 2, 3] * 10  // yields [10, 20, 30]

// Array-to-array: elementwise operation (lengths must match)
let vec1: array<int> = [1, 2, 3]
let vec2: array<int> = [10, 20, 30]
let res: array<int>  = vec1 + vec2   // yields [11, 22, 33]
```

The analyzer infers the result type and verifies shape compatibility at compile time.

## 6. Structural Subtyping and Composition

Causm features structural subtyping. Two types are compatible if they share a compatible structural layout.

### 6.1 Struct Composition
Structs can be composed using the `+` operator, which combines their fields:
```causm
type Actor = struct {
    name: string
}
type Robot = Actor + struct {
    model: string
}
```
This inherits all fields from `Actor` and adds `model` to `Robot`.

### 6.2 Structural Compatibility Rules
- **Struct Subtyping**: A struct `S1` is compatible with/assignable to another struct `S2` if `S1` contains at least all the fields of `S2` with compatible types.
- **Interface Subtyping**: A struct `S` implicitly implements an interface `I` if `S` defines all the methods specified by `I` with matching signatures (names, parameters, parameter modes, return types, and temporal contracts).

## 7. Enumerations (`enum`)

Enums define sum types with optional payload data per variant:

```causm
enum NodeStatus {
    Active(int),
    Degraded(string),
    Offline
}

let s1 = NodeStatus::Active(101)
let s2 = NodeStatus::Degraded("high_latency")
let s3 = NodeStatus::Offline
```

Variants can carry any Causm types (scalars, structs, arrays) and are fully tracked under the entropic lifecycle and match expressions.


