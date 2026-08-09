# Causm Numerical Type Casting & Array Broadcasting Proposal

This proposal specifies explicit numerical type casting (`as`) and elementwise array broadcasting for the Causm programming language.

---

## 1. Type Casting Expression (`expr as Type`)

Explicit casting syntax converts values between numerical types (`i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`):

```causm
let val: f64 = 42 as f64
let int_val: i32 = 3.14159 as i32
let byte_val: u8 = 257 as u8 // truncated to 1
```

### Grammar Syntax (`causm.pest`)
```pest
type_cast_tail = { "as" ~ type_name }
```

---

## 2. Elementwise Array Broadcasting

Binary operations (`+`, `-`, `*`, `/`, `%`) automatically broadcast when operating on arrays:

1. **Scalar-to-Array Broadcasting**:
   ```causm
   let arr: array<int> = [1, 2, 3] * 10 // yields [10, 20, 30]
   ```
2. **Array-to-Array Vector Operations**:
   ```causm
   let vec1: array<int> = [1, 2, 3]
   let vec2: array<int> = [10, 20, 30]
   let res: array<int> = vec1 + vec2 // yields [11, 22, 33]
   ```

---

## 3. Implementation Steps

1. **AST Extensions (`causm-core`)**: Add `Expression::TypeCast { expr: Box<Expression>, target_type: TypeName }`.
2. **Grammar & Parser (`causm.pest`, `causm-frontend`)**: Add `type_cast_tail` postfix operator to Pest parser.
3. **IR Lowering & Instructions (`causm-ir`)**: Add `Instruction::Cast { dest: Reg, src: Reg, target_type: TypeName }`.
4. **Semantic Analyzer (`causm-analysis`)**: Validate numerical cast compatibility and infer array broadcasting result types.
5. **VM Execution Engine (`causm-runtime`)**: Implement runtime casting functions and elementwise array broadcasting inside binary operation handlers.
