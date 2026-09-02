# Proposal: Explicit Sized Primitive Numerical Types (`i8`-`i64`, `u8`-`u64`, `f32`, `f64`)

**Status:** Approved & Implemented  
**Author:** Seuriin <seuriin@gmail.com>  
**Category:** Type System Core  

---

This proposal specifies the extension of Causm's type system to support explicit sized integer (`i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`) and floating-point (`f32`, `f64`) primitive types.

---

## 1. Motivation & Benefits

- **Isochronous Arena Packing**: Enables exact bit-level byte packing in `causm-runtime`'s memory arena for timeline execution branches.
- **Hardware Interop & Bit-Masking**: Clean mapping of sensor data streams (`SecurityNode<u8>`) and entropic decay masks.
- **Z3 Static Overflow Proofs**: Enables Z3 abstract interpretation in `causm-analysis` to verify numerical range safety and loop bounds.

---

## 2. Type Mapping & Alias Conventions

| Keyword | Representation | Bit Width | Standard Alias |
|---|---|---|---|
| `i8` | Signed 8-bit Integer | 8 | - |
| `i16` | Signed 16-bit Integer | 16 | - |
| `i32` | Signed 32-bit Integer | 32 | - |
| `i64` / `int` | Signed 64-bit Integer | 64 | `int` |
| `u8` | Unsigned 8-bit Integer | 8 | - |
| `u16` | Unsigned 16-bit Integer | 16 | - |
| `u32` | Unsigned 32-bit Integer | 32 | - |
| `u64` | Unsigned 64-bit Integer | 64 | - |
| `f32` | Single-precision Float | 32 | - |
| `f64` / `float` | Double-precision Float | 64 | `float` |

---

## 3. Implementation Workflow

1. **AST & Core**: Update `BuiltinType` and `Type` in `causm-core::types` and `causm-core::lib`.
2. **Grammar & Parser**: Update `causm.pest` and type parser in `causm-frontend`.
3. **Static Analyzer**: Update type compatibility, inference, and Z3 symbolic guard range checks in `causm-analysis`.
4. **VM Arena & Instructions**: Update payload representations and binary/unary arithmetic instructions in `causm-runtime`.
5. **Testing**: Add integration tests verifying sized numeric operations.
