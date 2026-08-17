# Proposal: Causm Standard Library — `std/encoding` (UTF-8, Base64, Binary)

**Status:** Proposed  
**Authors:** Seuriin <seuriin@gmail.com>, Iris Seravelle <iris.seravelle@gmail.com>  
**Category:** Standard Library  
**Target Crates:** `causm-stdlib`, `causm-frontend`, `causm-runtime`, `causm-core`

---

## 1. Executive Summary

Causm's networking, cryptography, and IPC primitives all ultimately operate over **raw byte arrays**. Today, bridging the gap between Causm's `string` type and a manipulable `array` of integer byte values requires reaching outside the language—either through C-FFI calls or hand-rolled ASCII math—because no VM-level intrinsic exists to perform the conversion. Downstream concerns such as Base64 encoding for HTTP `Authorization` headers, big-endian framing for TCP wire formats, and UTF-8 validation for text protocols are entirely absent from the standard library.

This proposal introduces `std/encoding`, a three-module encoding library built **entirely in pure Causm** (no FFI), backed by two new lightweight VM intrinsics that bridge the `string ↔ array` boundary at the execution layer. The result is a self-contained, formally verifiable, WCET-deterministic encoding stack that integrates cleanly with the pipeline operator (`|>`) introduced in the *Syntax Modernization* proposal.

---

## 2. VM Intrinsics: Bridging `string ↔ array`

Before any pure-Causm encoding logic can be written, the VM must expose two new built-in operations. These are **not** standard library routines; they are primitive intrinsics wired directly into the `causm-runtime` execution engine, recognized during IR lowering in `causm-frontend`, and given dedicated AST expression nodes in `causm-core`.

### 2.1 `str_bytes(text: string) -> array`

Extracts the raw UTF-8 byte representation of a Causm `string` value and returns it as an `array` of `i32` integer values (one element per byte, value range `0–255`).

```causm
let msg    = "Hello"
let bytes  = str_bytes(msg)
// bytes == [72, 101, 108, 108, 111]
```

### 2.2 `to_str(arr: array) -> string`

Constructs a Causm `string` from an `array` of `i32` integer byte values. Each element is interpreted as an unsigned byte (`0–255`). The resulting string is valid UTF-8; passing an invalid byte sequence is a runtime entropic fault.

```causm
let bytes  = [72, 101, 108, 108, 111]
let result = to_str(bytes)
// result == "Hello"
```

### 2.3 Full Pipeline Implementation

Both intrinsics must be threaded through **every layer** of the compiler and runtime:

| Layer | Work Required |
| :--- | :--- |
| **Grammar** (`causm.pest`) | Add `str_bytes` and `to_str` as recognized `intrinsic_call` terminals |
| **AST** (`causm-core/src/lib.rs`) | Add `Expr::StrBytes { text: Box<Expr> }` and `Expr::ToStr { arr: Box<Expr> }` variants |
| **Parser** (`causm-frontend/src/parser/`) | Emit the new AST variants when the intrinsic identifiers are encountered in call position |
| **IR Lowering** (`causm-frontend/src/lower/`) | Lower to new flat IR ops `StrBytes { dst, src }` and `ToStr { dst, src }` |
| **Type Inference** (`causm-analysis/src/`) | `str_bytes`: `string -> array`; `to_str`: `array -> string`; emit type error on mismatch |
| **WCET Cost** (`causm-analysis/src/`) | Both ops carry `O(n)` cost proportional to string/array length; annotate as `taking _` compatible |
| **VM Execution** (`causm-runtime/src/vm/`) | Implement `OpStrBytes` by iterating the arena string bytes into a new heap array; implement `OpToStr` by collecting array elements into a UTF-8 arena string |

---

## 3. Module: `std/encoding/utf8`

The `utf8` module provides the primary human-readable text encoding routines. It is a thin, semantically clear wrapper over the two VM intrinsics, giving encoding operations a stable, namespaced API.

### 3.1 Routines

```causm
// Encode a Causm string to its raw UTF-8 byte array.
pub routine utf8.encode(text: string) -> array taking _ {
    let bytes = str_bytes(text)
    yield bytes
}

// Decode a UTF-8 byte array back to a Causm string.
// `len` controls how many bytes from the start of `bytes` are consumed.
pub routine utf8.decode(bytes: array, len: i32) -> string taking _ {
    let slice = array.slice(bytes, 0, len)
    let text  = to_str(slice)
    yield text
}

// Return the byte length of the UTF-8 encoding of `text`,
// without producing the intermediate byte array.
pub routine utf8.encode_len(text: string) -> i32 taking _ {
    let bytes = str_bytes(text)
    let n     = array.len(bytes)
    yield n
}
```

### 3.2 Usage Example

```causm
@5ms: {
    let message    = "Causm encodes!"
    let encoded    = utf8.encode(message)
    let byte_count = utf8.encode_len(message)
    print(f"Byte length: {byte_count}")

    let decoded = utf8.decode(encoded, byte_count)
    print(f"Round-tripped: {decoded}")
}
```

---

## 4. Module: `std/encoding/base64`

The `base64` module implements the standard RFC 4648 Base64 alphabet in **pure Causm**—no FFI, no host calls, no external dependencies. Every operation is a composition of integer arithmetic on `i32` values, making it fully amenable to Z3 SMT verification and static WCET analysis.

### 4.1 Alphabet Mapping

```causm
// Maps an index in [0, 63] to its Base64 ASCII byte value.
// A-Z => 0-25, a-z => 26-51, 0-9 => 52-61, '+' => 62, '/' => 63
pub routine base64.alphabet_char(idx: i32) -> i32 taking _ {
    if idx < 26 {
        yield idx + 65          // 'A' = 65
    } else if idx < 52 {
        yield idx - 26 + 97     // 'a' = 97
    } else if idx < 62 {
        yield idx - 52 + 48     // '0' = 48
    } else if idx == 62 {
        yield 43                // '+'
    } else {
        yield 47                // '/'
    }
}

// Maps a Base64 ASCII byte value back to its [0, 63] index.
// Returns -1 for '=' (padding) and -2 for invalid characters.
pub routine base64.decode_char(c: i32) -> i32 taking _ {
    if c >= 65 && c <= 90 {
        yield c - 65            // A-Z => 0-25
    } else if c >= 97 && c <= 122 {
        yield c - 97 + 26       // a-z => 26-51
    } else if c >= 48 && c <= 57 {
        yield c - 48 + 52       // 0-9 => 52-61
    } else if c == 43 {
        yield 62                // '+'
    } else if c == 47 {
        yield 63                // '/'
    } else if c == 61 {
        yield -1                // '=' padding
    } else {
        yield -2                // invalid
    }
}
```

### 4.2 Chunk Encode / Decode

```causm
// Encodes 3 raw bytes (b0, b1, b2) into 4 Base64 ASCII byte values.
pub routine base64.encode_chunk(b0: i32, b1: i32, b2: i32) -> array taking _ {
    let i0 = (b0 >> 2) & 63
    let i1 = ((b0 & 3) << 4) | ((b1 >> 4) & 15)
    let i2 = ((b1 & 15) << 2) | ((b2 >> 6) & 3)
    let i3 = b2 & 63
    let c0 = base64.alphabet_char(i0)
    let c1 = base64.alphabet_char(i1)
    let c2 = base64.alphabet_char(i2)
    let c3 = base64.alphabet_char(i3)
    yield [c0, c1, c2, c3]
}

// Decodes 4 Base64 ASCII byte values (c0, c1, c2, c3) into 3 raw bytes.
pub routine base64.decode_chunk(c0: i32, c1: i32, c2: i32, c3: i32) -> array taking _ {
    let v0 = base64.decode_char(c0)
    let v1 = base64.decode_char(c1)
    let v2 = base64.decode_char(c2)
    let v3 = base64.decode_char(c3)
    let b0 = (v0 << 2) | (v1 >> 4)
    let b1 = ((v1 & 15) << 4) | (v2 >> 2)
    let b2 = ((v2 & 3) << 6) | v3
    yield [b0, b1, b2]
}
```

### 4.3 Full Array Encode

```causm
// Encodes an array of raw bytes of length `len` into a Base64 ASCII byte array.
// Output length is always ceil(len / 3) * 4 bytes (standard padding with '=').
pub routine base64.encode(bytes: array, len: i32) -> array taking _ {
    let out    = array.new()
    let i      = 0
    let pad_eq = 61    // '='

    loop {
        if i >= len { break }
        let b0    = array.get(bytes, i)
        let b1    = if (i + 1) < len { array.get(bytes, i + 1) } else { 0 }
        let b2    = if (i + 2) < len { array.get(bytes, i + 2) } else { 0 }
        let chunk = base64.encode_chunk(b0, b1, b2)
        let c0    = array.get(chunk, 0)
        let c1    = array.get(chunk, 1)
        let c2    = if (i + 1) < len { array.get(chunk, 2) } else { pad_eq }
        let c3    = if (i + 2) < len { array.get(chunk, 3) } else { pad_eq }
        array.push(out, c0)
        array.push(out, c1)
        array.push(out, c2)
        array.push(out, c3)
        i = i + 3
    }

    yield out
}
```

### 4.4 Usage Example

```causm
@5ms: {
    let raw       = utf8.encode("Causm")
    let raw_len   = utf8.encode_len("Causm")
    let b64_bytes = base64.encode(raw, raw_len)
    let b64_str   = utf8.decode(b64_bytes, array.len(b64_bytes))
    print(f"Base64: {b64_str}")    // "Q2F1c20="
}
```

---

## 5. Module: `std/encoding/binary`

The `binary` module provides deterministic integer serialization and deserialization for both big-endian (network byte order) and little-endian wire formats. All routines are **pure Causm integer arithmetic** with no FFI and `O(1)` WCET cost.

### 5.1 Write Routines

```causm
// Serialize `val` as a single unsigned byte (bits 0-7).
pub routine binary.write_u8(val: i32) -> array taking _ {
    yield [val & 255]
}

// Serialize `val` as 2 bytes, big-endian (network byte order).
pub routine binary.write_u16_be(val: i32) -> array taking _ {
    yield [(val >> 8) & 255, val & 255]
}

// Serialize `val` as 2 bytes, little-endian.
pub routine binary.write_u16_le(val: i32) -> array taking _ {
    yield [val & 255, (val >> 8) & 255]
}

// Serialize `val` as 4 bytes, big-endian.
pub routine binary.write_u32_be(val: i32) -> array taking _ {
    yield [
        (val >> 24) & 255,
        (val >> 16) & 255,
        (val >>  8) & 255,
         val        & 255
    ]
}

// Serialize `val` as 4 bytes, little-endian.
pub routine binary.write_u32_le(val: i32) -> array taking _ {
    yield [
         val        & 255,
        (val >>  8) & 255,
        (val >> 16) & 255,
        (val >> 24) & 255
    ]
}

// Serialize as 8 bytes, big-endian.
// `hi` = upper 32 bits, `lo` = lower 32 bits of the 64-bit value.
pub routine binary.write_u64_be(hi: i32, lo: i32) -> array taking _ {
    yield [
        (hi >> 24) & 255,
        (hi >> 16) & 255,
        (hi >>  8) & 255,
         hi        & 255,
        (lo >> 24) & 255,
        (lo >> 16) & 255,
        (lo >>  8) & 255,
         lo        & 255
    ]
}

// Serialize as 8 bytes, little-endian.
pub routine binary.write_u64_le(hi: i32, lo: i32) -> array taking _ {
    yield [
         lo        & 255,
        (lo >>  8) & 255,
        (lo >> 16) & 255,
        (lo >> 24) & 255,
         hi        & 255,
        (hi >>  8) & 255,
        (hi >> 16) & 255,
        (hi >> 24) & 255
    ]
}
```

### 5.2 Read Routines

```causm
// Read 2 bytes from `arr` as a big-endian unsigned 16-bit integer.
pub routine binary.read_u16_be(arr: array) -> i32 taking _ {
    let b0 = array.get(arr, 0)
    let b1 = array.get(arr, 1)
    yield (b0 << 8) | b1
}

// Read 2 bytes from `arr` as a little-endian unsigned 16-bit integer.
pub routine binary.read_u16_le(arr: array) -> i32 taking _ {
    let b0 = array.get(arr, 0)
    let b1 = array.get(arr, 1)
    yield b0 | (b1 << 8)
}

// Read 4 bytes from `arr` as a big-endian unsigned 32-bit integer.
pub routine binary.read_u32_be(arr: array) -> i32 taking _ {
    let b0 = array.get(arr, 0)
    let b1 = array.get(arr, 1)
    let b2 = array.get(arr, 2)
    let b3 = array.get(arr, 3)
    yield (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
}

// Read 4 bytes from `arr` as a little-endian unsigned 32-bit integer.
pub routine binary.read_u32_le(arr: array) -> i32 taking _ {
    let b0 = array.get(arr, 0)
    let b1 = array.get(arr, 1)
    let b2 = array.get(arr, 2)
    let b3 = array.get(arr, 3)
    yield b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
}
```

### 5.3 Usage Example

```causm
@1ms: {
    // Build a minimal 4-byte message header: [magic u16 BE] [length u16 BE]
    let magic_bytes  = binary.write_u16_be(0xCAFE)
    let length_bytes = binary.write_u16_be(42)
    let header       = array.concat(magic_bytes, length_bytes)

    // Read back the magic and length fields
    let magic  = binary.read_u16_be(header)
    let length = binary.read_u16_be(array.slice(header, 2, 4))
    print(f"Magic: {magic}, Payload length: {length}")
}
```

---

## 6. Pipeline Operator Synergy

The `std/encoding` modules are designed as first-class citizens of Causm's `|>` pipeline operator. The following patterns become natural with the encoding library in place.

### 6.1 TCP Client with Framed Binary Protocol

```causm
@10ms: {
    using stream = Net.TcpStream.connect("192.168.1.1", 8080) {
        // Build a length-prefixed frame: [u32 BE length][UTF-8 payload]
        let payload = "PING"
        let body    = utf8.encode(payload)
        let frame   = utf8.encode_len(payload)
            |> binary.write_u32_be()
            |> array.concat(body)

        stream.send(frame)
    }
}
```

### 6.2 HTTP Basic Auth Header

```causm
@5ms: {
    let credentials = "user:secret"
    let auth_token  = credentials
        |> utf8.encode()
        |> base64.encode(utf8.encode_len(credentials))
        |> utf8.decode(array.len())

    let header = f"Authorization: Basic {auth_token}"
    print(header)
}
```

### 6.3 Binary Telemetry Packet

```causm
@2ms: {
    // Construct a compact telemetry packet: [timestamp u32 BE][sensor_id u16 BE][value u32 BE]
    let timestamp = 1_724_000_000
    let sensor_id = 7
    let value     = 993

    let packet = array.concat(
        binary.write_u32_be(timestamp),
        array.concat(
            binary.write_u16_be(sensor_id),
            binary.write_u32_be(value)
        )
    )

    let encoded = base64.encode(packet, array.len(packet))
    print(f"Telemetry (base64): {utf8.decode(encoded, array.len(encoded))}")
}
```

---

## 7. WCET Analysis & Temporal Gate Compatibility

All routines in `std/encoding` are statically verifiable by the Causm WCET analyzer. The following cost classifications apply:

| Routine | Complexity | `taking` Annotation | Notes |
| :--- | :--- | :--- | :--- |
| `str_bytes(text)` | `O(n)` in string byte length | `taking _` (inferred) | VM iterates string arena buffer once |
| `to_str(arr)` | `O(n)` in array length | `taking _` (inferred) | VM iterates array elements once |
| `utf8.encode(text)` | `O(n)` | `taking _` | Delegates to `str_bytes` |
| `utf8.decode(bytes, len)` | `O(n)` | `taking _` | Delegates to `to_str` after slice |
| `utf8.encode_len(text)` | `O(n)` | `taking _` | Single `str_bytes` + `array.len` |
| `base64.alphabet_char(idx)` | `O(1)` | `taking _` | Pure integer branch chain |
| `base64.decode_char(c)` | `O(1)` | `taking _` | Pure integer branch chain |
| `base64.encode_chunk(b0,b1,b2)` | `O(1)` | `taking _` | 4 fixed shift/mask ops |
| `base64.decode_chunk(c0,c1,c2,c3)` | `O(1)` | `taking _` | 3 fixed shift/mask ops |
| `base64.encode(bytes, len)` | `O(n)` | `taking _` | Loop over `ceil(n/3)` chunks |
| `binary.write_u8` | `O(1)` | `taking _` | 1 array element |
| `binary.write_u16_be/le` | `O(1)` | `taking _` | 2 fixed shift/mask ops |
| `binary.write_u32_be/le` | `O(1)` | `taking _` | 4 fixed shift/mask ops |
| `binary.write_u64_be/le` | `O(1)` | `taking _` | 8 fixed shift/mask ops |
| `binary.read_u16_be/le` | `O(1)` | `taking _` | 2 array reads + OR |
| `binary.read_u32_be/le` | `O(1)` | `taking _` | 4 array reads + OR |

All `O(n)` routines are **safe inside temporal gates** (`@Nms`) provided the compiler can statically bound `n` at the call site. Fixed-size strings and arrays with constant-bounded lengths satisfy this constraint without user annotation. For dynamically sized inputs, the user must annotate or the WCET analyzer will emit a budget warning.

---

## 8. Implementation Roadmap

| Phase | Milestone | Deliverable |
| :--- | :--- | :--- |
| **Phase 1** | **VM Intrinsics** | Add `str_bytes` / `to_str` to `causm.pest`, AST, IR lowering, type inference, and VM execution in `causm-runtime` |
| **Phase 2** | **`std/encoding/utf8`** | Implement `encode`, `decode`, `encode_len` in `causm-stdlib/src/encoding/utf8.csm` |
| **Phase 3** | **`std/encoding/base64`** | Implement alphabet maps, chunk encode/decode, and full array encode in `causm-stdlib/src/encoding/base64.csm` |
| **Phase 4** | **`std/encoding/binary`** | Implement all `write_*` and `read_*` routines in `causm-stdlib/src/encoding/binary.csm` |
| **Phase 5** | **Integration Tests** | Write isolated, named integration tests in `crates/causm-cli/tests/integration/` covering all routines and edge cases |
| **Phase 6** | **WCET Annotation Validation** | Confirm static analyzer correctly infers `O(n)` and `O(1)` cost bounds and emits budget warnings for unbounded dynamic inputs |

---

## 9. Integration Tests

All tests reside in `crates/causm-cli/tests/integration/` and follow the `test_<category>_<feature>_<scenario>` naming mandate.

### 9.1 VM Intrinsics

```
test_intrinsic_str_bytes_ascii_string
test_intrinsic_str_bytes_empty_string
test_intrinsic_str_bytes_multibyte_utf8
test_intrinsic_to_str_valid_ascii_bytes
test_intrinsic_to_str_roundtrip_with_str_bytes
test_intrinsic_to_str_empty_array
test_intrinsic_str_bytes_type_error_non_string
test_intrinsic_to_str_type_error_non_array
```

### 9.2 `std/encoding/utf8`

```
test_encoding_utf8_encode_ascii_text
test_encoding_utf8_encode_empty_string
test_encoding_utf8_decode_valid_bytes
test_encoding_utf8_decode_partial_length
test_encoding_utf8_encode_len_ascii
test_encoding_utf8_encode_len_empty
test_encoding_utf8_roundtrip_encode_decode
```

### 9.3 `std/encoding/base64`

```
test_encoding_base64_alphabet_char_az_range
test_encoding_base64_alphabet_char_az_lowercase_range
test_encoding_base64_alphabet_char_digit_range
test_encoding_base64_alphabet_char_plus_slash
test_encoding_base64_decode_char_az_range
test_encoding_base64_decode_char_az_lowercase_range
test_encoding_base64_decode_char_digit_range
test_encoding_base64_decode_char_padding_equals
test_encoding_base64_decode_char_invalid_byte
test_encoding_base64_encode_chunk_three_bytes
test_encoding_base64_encode_chunk_zero_padding
test_encoding_base64_decode_chunk_four_chars
test_encoding_base64_encode_full_array_no_padding
test_encoding_base64_encode_full_array_one_pad
test_encoding_base64_encode_full_array_two_pad
test_encoding_base64_encode_empty_array
test_encoding_base64_roundtrip_encode_decode_ascii
```

### 9.4 `std/encoding/binary`

```
test_encoding_binary_write_u8_zero
test_encoding_binary_write_u8_max_byte
test_encoding_binary_write_u16_be_known_value
test_encoding_binary_write_u16_le_known_value
test_encoding_binary_write_u32_be_known_value
test_encoding_binary_write_u32_le_known_value
test_encoding_binary_write_u64_be_known_value
test_encoding_binary_write_u64_le_known_value
test_encoding_binary_read_u16_be_known_value
test_encoding_binary_read_u16_le_known_value
test_encoding_binary_read_u32_be_known_value
test_encoding_binary_read_u32_le_known_value
test_encoding_binary_read_u16_be_roundtrip_write
test_encoding_binary_read_u32_be_roundtrip_write
test_encoding_binary_read_u32_le_roundtrip_write
```

### 9.5 Pipeline & Synergy

```
test_encoding_pipeline_utf8_base64_encode_string
test_encoding_pipeline_binary_framed_header_construct
test_encoding_pipeline_base64_http_auth_header
test_encoding_pipeline_telemetry_packet_base64_roundtrip
```

---

## 10. Conclusion

`std/encoding` closes the final gap between Causm's formal deterministic execution model and practical systems programming tasks. By grounding the library in two minimal, precisely-typed VM intrinsics (`str_bytes` and `to_str`) and building everything above them in pure Causm, the encoding stack inherits all of Causm's guarantees: **Z3-verified entropic state transitions, statically bounded WCET cost, arena memory safety, and zero FFI surface**. The result is a standard library module expressive enough to power real-world TCP framing, HTTP authentication, and binary telemetry—while remaining completely transparent to the formal verification pipeline.
