# Proposal: Causm Metaprogramming, Advanced Semantics, Expressive Operators, and Unified Type System

**Status:** Proposed  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Language Core, Metaprogramming, Type System & Syntax Modernization  
**Target Crates:** `causm-frontend`, `causm-core`, `causm-analysis`, `causm-devtools`, `causm-stdlib`

---

## 1. Executive Summary & Unified Vision

With Causm’s execution layer established—spanning WASM compilation, POSIX C-FFI, `oxiz` SMT formal verification, and affine memory lifecycles—the language is ready for its complete surface-level and semantic evolution.

This proposal establishes a comprehensive language specification upgrading four interconnected dimensions of Causm:
1. **Metaprogramming Suite:** Declarative hygienic pattern macros (`macro!`), compiler attributes (`@derive`, `@must_use`, `@inline`, `@test`), and sandboxed WASM procedural plugins.
2. **Expressive Operators & Expressions:** Compound assignment (`+=`, `*=`), full bitwise manipulation (`<<`, `>>`, `&`, `|`, `^`, `~`), monadic error propagation (`?`), null-coalescing (`??`), and open/closed range expressions (`..`, `..=`).
3. **Advanced Control Flow & Temporal Expressions:** Exhaustive pattern matching across tuples, ranges, and ADT payloads with guards; first-class inline `timeout` expressions; and compile-time `const` evaluations.
4. **Unified Type System:** Standard `Result<T, E>` and `Option<T>` monads, anonymous/labeled tuples, nominal "newtypes" for physical units, and Const Generics (`[T; N]`).

---

## 2. Metaprogramming & Compiler Attributes

### 2.1 Declarative Hygienic Pattern Macros (`macro!`)

Declarative macros operate on token trees to eliminate boilerplate without compromising compile-time safety or SMT verification.

```causm
macro impl_bitwise_flags!(
    ($enum_name:ident, $underlying:type, $( $variant:ident = $val:expr );+) => {
        enum $enum_name {
            $( $variant ),+
        }

        routine $enum_name.to_bits(&self) -> $underlying taking _ =>
            match *self {
                $( $enum_name::$variant => $val ),+
            }
    }
)

// Generates zero-cost bitwise flag encoders:
impl_bitwise_flags!(
    SocketFlags, u16,
    NonBlocking = 0x0001;
    ReuseAddr   = 0x0002;
    KeepAlive   = 0x0004;
    NoDelay     = 0x0008
)
```

---

### 2.2 Standard Compiler Attributes

Attributes decorate types, routines, and isolates to instruct the frontend, optimizer, and test runners:

| Attribute | Target | Semantic Meaning |
| :--- | :--- | :--- |
| **`@derive(...)`** | `struct`, `enum` | Automatically synthesizes `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq`, `AutoDrop`. |
| **`@must_use`** | `routine`, `type` | Emits a compiler warning if the returned value or `Result` is ignored. |
| **`@inline(always)`** | `routine` | Forces the SSA IR optimizer to inline the routine body into the caller. |
| **`@unroll(N)`** | `for`, `while` | Instructs the SSA loop optimizer to fully unroll the loop by factor $N$. |
| **`@test`** | `routine` | Registers the routine as an executable unit test for `causm test`. |
| **`@bench`** | `routine` | Registers the routine for cycle-accurate statistical benchmarking via `causm bench`. |
| **`@deprecated(msg)`**| Any declaration | Warns developers with migration notes when using legacy symbols. |

---

### 2.3 Built-in Derivation Showcase

```causm
@derive(Clone, Debug, PartialEq, Serialize, Deserialize)
type SensorReading = struct {
    sensor_id: string,
    voltage_mv: i32,
    calibrated: bool
}

// Automatically gives:
let s1 = SensorReading { sensor_id: "IMU_0", voltage_mv: 3300, calibrated: true }
let s2 = s1.clone()
let is_equal = (s1 == s2)             // Synthesized PartialEq
let json_str = s1.to_json_string()     // Synthesized Serialize
print(s1.to_debug_string())           // Synthesized Debug
```

---

## 3. Expressive Operators & Syntax Primitives

### 3.1 Compound Assignment Operators
Replaces verbose re-assignments (`x = x + 1`) with idiomatic compound mutators:
* Arithmetic: `+=`, `-=`, `*=`, `/=`, `%=`
* Bitwise: `&=`, `|=`, `^=`, `<<=`, `>>=`

```causm
let mut counter = 0
counter += 10
counter <<= 2   // counter is now 40
```

---

### 3.2 Full Bitwise Manipulation Suite
Essential for binary wire framing (`std/encoding/binary`), network protocol flags (`std/net`), and hardware registers:

```causm
let mask = (0x0F << 4) | 0x02   // Bitwise shift and OR
let inverted = ~mask             // Bitwise NOT
let flag_active = (raw & 0x04) != 0
```

---

### 3.3 The Monadic Error Propagation Operator (`?`)
Eliminates nested `match` boilerplate when working with `Option<T>` or `Result<T, E>`. If the expression resolves to `Err` or `None`, the routine immediately returns early.

```causm
routine fetch_user_avatar(id: string) -> Result<Image, string> taking _ {
    let raw_json = Http.get(f"/api/users/{id}")?     // Early returns on HTTP Error
    let parsed = Json.parse(raw_json)?               // Early returns on JSON Parse Error
    let avatar_url = parsed.get("avatar_url")?.as_string()
    
    let img_bytes = Http.get(avatar_url)?
    Result::Ok(Image.decode(img_bytes)?)
}
```

---

### 3.4 Null-Coalescing Operator (`??`)
Provides clean, default-fallback expressions for `Option<T>`:

```causm
let port = config.get_int("port") ?? 8080
let hostname = user_input ?? "127.0.0.1"
```

---

### 3.5 Full Range Expressions & Slicing (`..`, `..=`)

```causm
// Iteration:
for i in 0..10 step _ { ... }        // Exclusive: 0 to 9
for byte in 0x00..=0xFF step _ { ... } // Inclusive: 0 to 255

// Slicing:
let head = buffer[..4]              // From start to 4
let tail = buffer[12..]             // From 12 to end
let sub  = buffer[4..12]            // Window 4 to 12
```

---

### 3.6 Tuple Primitives & Multi-Value Returns
Tuples provide lightweight, unnamed structured grouping without the overhead of declaring custom `type ... = struct` names for simple pairs.

```causm
// Tuple creation and type annotation:
let point: (i32, i32) = (100, 250)

// Multi-value routine returns:
routine divide_with_remainder(dividend: int, divisor: int) -> (int, int) taking _ {
    (dividend / divisor, dividend % divisor)
}

// Tuple destructuring:
let (quotient, rem) = divide_with_remainder(100, 7)
print(f"100 / 7 = {quotient} remainder {rem}")
```

---

## 4. Advanced Control Flow & Temporal Semantics

### 4.1 Exhaustive Pattern Matching (`match`)

Causm’s `match` expression is expanded to support deep structural destructuring, ranges, tuples, and boolean match guards:

```causm
let status_text = match packet {
    // 1. Enum payload matching:
    JsonValue::String(s) => f"String payload: {s}",
    JsonValue::Number(n) if n < 0 => "Negative quantity rejected",
    JsonValue::Number(n) => f"Valid number: {n}",
    
    // 2. Tuple matching:
    (0, 0) => "Coordinate origin",
    (x, y) if x == y => "Diagonal coordinate",
    
    // 3. Inclusive range matching:
    0x00..=0x1F => "ASCII Control Character",
    0x30..=0x39 => "ASCII Digit",
    
    // 4. Wildcard fallback:
    _ => "Unknown binary frame"
}
```

---

### 4.2 First-Class Inline `timeout` Expressions

Replaces ad-hoc statement-level select blocks with expression-level temporal bounds:

```causm
let response = timeout 50ms {
    stream.recv(1024)
} else {
    // Executes automatically if 50ms temporal contract is breached:
    b"TIMEOUT_FALLBACK_PAYLOAD"
}
```

---

### 4.3 Compile-Time Constants & `const fn`

Variables and expressions annotated with `const` are evaluated by the frontend AST engine during compilation with **zero runtime instructions**:

```causm
const MAX_PACKET_SIZE: usize = 1024 * 64 // 64 KB
const DEFAULT_TIMEOUT: Duration = 500ms

const routine compute_arena_padding(align: usize, size: usize) -> usize =>
    (size + (align - 1)) & ~(align - 1)

const BUFFER_STRIDE: usize = compute_arena_padding(8, 25) // Evaluated to 32 at compile-time!
```

---

## 5. Unified Type System Enhancements

### 5.1 First-Class Monadic Enums in `std/core`

```causm
enum Option<T> {
    Some(T),
    None
}

enum Result<T, E> {
    Ok(T),
    Err(E)
}
```

#### Standard Monadic Helper Routines on `Option` and `Result`:
* `.is_some()`, `.is_none()`, `.is_ok()`, `.is_err()`
* `.unwrap()`, `.unwrap_or(default)`, `.expect(msg)`
* `.map(fn)` and `.and_then(fn)` for functional transformation chains.

---

### 5.2 Const Generics (`[T; N]`)
Allows data structures to be parameterized by static integer constants (such as buffer capacities):

```causm
type StaticRingBuffer<T, const CAPACITY: usize> = struct {
    data: [T; CAPACITY],
    head: usize,
    tail: usize,
    size: usize
}

// Instantiate fixed-size arrays and structures with zero heap allocation:
let mut imu_samples: StaticRingBuffer<f32, 64> = StaticRingBuffer::new()
```

---

### 5.3 Nominal "Newtypes" (Unit Safety for Physical Systems)

In robotics and aerospace, mixing up millimeters and meters or radians and degrees causes catastrophic hardware failures. Nominal newtypes enforce zero-cost unit safety:

```causm
type Millimeters = distinct int
type Radians     = distinct float

routine set_joint_angle(angle: Radians) taking _ { ... }

let d: Millimeters = 500
// COMPILE ERROR: Cannot pass Millimeters to routine expecting Radians!
// set_joint_angle(d) 
```

---

## 6. Comprehensive Idiomatic Showcase

### Real-Time Microservice Endpoint (`examples/modern_system_showcase.csm`)

```causm
import "std/net" as Net
import "std/json" as Json
import "std/http" as Http

@derive(Clone, Debug, Serialize, Deserialize)
type TelemetryPacket = struct {
    node_id: string,
    seq: i32,
    altitude_m: f32,
    battery_pct: f32
}

macro route_handler!(
    ($path:expr, $handler:ident, max_time: $t:duration) => {
        if (req.path == $path) {
            let res = timeout $t {
                $handler(req)
            } else {
                Http.Response.status(504).body("Gateway Timeout")
            }
            return res
        }
    }
)

routine handle_telemetry(req: Http.Request) -> Result<Http.Response, string> taking _ {
    // 1. Monadic extraction with `?`:
    let body_str = req.body_as_string()?
    let packet = Json.from_string::<TelemetryPacket>(body_str)?

    // 2. Compound assignment & bitwise operations:
    let mut flags: u16 = 0
    if (packet.battery_pct < 20.0) { flags |= 0x01 }
    if (packet.altitude_m > 1000.0) { flags |= 0x02 }

    // 3. Pattern matching with match guards:
    let alert_level = match flags {
        0 => "NOMINAL",
        f if (f & 0x01) != 0 => "LOW_BATTERY_WARNING",
        _ => "CRITICAL_AIRSPACE_ALERT"
    }

    print(f"[Telemetry] Node {packet.node_id} (Seq #{packet.seq}) -> Status: {alert_level}")

    // 4. Return formatted JSON response:
    Result::Ok(Http.Response.ok().json(struct {
        ack = packet.seq,
        status = alert_level,
        flags = flags
    }))
}

@0ms: {
    isolate edge_gateway {
        enable memory(128KB)
        require System.WASI
        require System.Log

        using server = Net.TcpListener.bind(8080) {
            print("Edge Telemetry Gateway active on port 8080")
            
            loop on server {
                using conn = server.accept() {
                    let req = Http.Request.read_from(&conn)?
                    let response = handle_telemetry(req).unwrap_or_else(err => Http.Response.bad_request(err))
                    conn.send(response.to_bytes())
                }
            }
        }
    }
}
```

---

## 7. Implementation Roadmap

| Phase | Milestone | Scope | Deliverables |
| :--- | :--- | :--- | :--- |
| **Phase 1** | **Core Operators & Expressions** | Frontend (`causm-frontend`) | Compound assignments (`+=`), bitwise operators (`&`, `|`, `~`, `<<`, `>>`), ranges (`..`, `..=`), and monadic `?`. |
| **Phase 2** | **Tuples & Multi-Value Returns** | Frontend + Core | Tuple literals `(a, b)`, tuple type definitions, and destructuring bindings. |
| **Phase 3** | **Declarative Macro Engine (`macro!`)** | Frontend AST | Implement token tree pattern matcher, hygiene tables, and macro expansion pass. |
| **Phase 4** | **Compiler Attributes & `@derive`** | Frontend + Devtools | Synthesize `Clone`, `Debug`, `Serialize`, `Deserialize`, `PartialEq`, and `@must_use`. |
| **Phase 5** | **Advanced Pattern Matching** | Frontend + Analysis | Deep match destructuring across enums, tuples, ranges, and boolean guards. |
| **Phase 6** | **Const Generics & Newtypes** | Core + Analysis | Static integer generic bounds (`[T; N]`) and nominal `distinct` type safety. |

---

## 8. Conclusion

By unifying declarative macros, expressive operators (`+=`, `?`, `|ban`), rich pattern matching, and unit-safe nominal types, Causm achieves the expressiveness and productivity of Rust, Swift, and Python while maintaining its core purpose: **uncompromising hard real-time determinism, sub-microsecond latency, and SMT-verified safety.**
