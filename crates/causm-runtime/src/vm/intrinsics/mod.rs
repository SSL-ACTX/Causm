//! Centralized registry and dispatch for runtime intrinsics.

pub mod base64;
pub mod binary;
pub mod json;
pub mod sync;

use crate::vm::TemporalError;
use causm_core::value::Payload;

/// Checks whether a routine identifier corresponds to a registered VM intrinsic.
pub fn is_intrinsic(name: &str) -> bool {
    matches!(
        name,
        // Math
        "sqrt"
            | "sin"
            | "cos"
            | "tan"
            | "exp"
            | "ln"
            | "log10"
            | "floor"
            | "ceil"
            | "round"
            // Collections & Strings
            | "push"
            | "pop"
            | "array_push"
            | "array_slice"
            | "string_from_bytes"
            | "char_at"
            | "str_slice"
            // JSON
            | "json_parse"
            | "json_stringify"
            // Base64
            | "base64_encode"
            | "base64_decode"
            | "base64_encode_chunk"
            | "base64_decode_chunk"
            // Binary
            | "binary_write_u16_be"
            | "binary_write_u16_le"
            | "binary_write_u32_be"
            | "binary_write_u32_le"
            | "binary_write_u64_be"
            | "binary_write_u64_le"
            | "binary_read_u16_be"
            | "binary_read_u16_le"
            | "binary_read_u32_be"
            | "binary_read_u32_le"
            | "binary_read_u64_be"
            | "binary_read_u64_le"
            // Sync
            | "__sync_atomic_new_int"
            | "__sync_atomic_load_int"
            | "__sync_atomic_store_int"
            | "__sync_atomic_fetch_add"
            | "__sync_atomic_cas_int"
            | "__sync_atomic_new_bool"
            | "__sync_atomic_load_bool"
            | "__sync_atomic_store_bool"
            | "__sync_atomic_cas_bool"
            | "__sync_mutex_new"
            | "__sync_mutex_try_lock"
            | "__sync_mutex_unlock"
            | "__sync_mutex_is_locked"
            | "__sync_mutex_owner"
            | "__sync_channel_new"
            | "__sync_channel_send"
            | "__sync_channel_recv"
            | "__sync_channel_close"
            | "__sync_channel_is_closed"
            | "__sync_channel_len"
            | "__sync_channel_is_full"
            | "__sync_channel_is_empty"
    )
}

/// Dispatches an intrinsic call to its corresponding domain module.
pub fn dispatch_intrinsic(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    // 1. JSON
    if let Some(res) = dispatch_json(name, args) {
        return Some(res);
    }
    // 2. Base64
    if let Some(res) = dispatch_base64(name, args) {
        return Some(res);
    }
    // 3. Binary
    if let Some(res) = dispatch_binary(name, args) {
        return Some(res);
    }
    // 4. Sync
    if let Some(res) = sync::dispatch(name, args) {
        return Some(res);
    }
    // 5. Math
    if let Some(res) = dispatch_math(name, args) {
        return Some(res);
    }
    // 6. Collections & Strings
    if let Some(res) = dispatch_collections(name, args) {
        return Some(res);
    }

    None
}

fn dispatch_json(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    match name {
        "json_parse" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(
                    "json_parse expects (json_string)".to_string(),
                )));
            }
            match &args[0] {
                Payload::String(s) => Some(json::parse_json(s).map_err(|e| {
                    TemporalError::EvalError(format!("JSON parse error: {}", e))
                })),
                _ => Some(Err(TemporalError::TypeMismatch(
                    "json_parse expects string".to_string(),
                ))),
            }
        }
        "json_stringify" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(
                    "json_stringify expects (payload)".to_string(),
                )));
            }
            let s = json::stringify_json(&args[0]);
            Some(Ok(Payload::String(s)))
        }
        _ => None,
    }
}

fn dispatch_base64(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    match name {
        "base64_encode" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(
                    "base64_encode expects (array_of_bytes)".to_string(),
                )));
            }
            match &args[0] {
                Payload::Array(arr) => {
                    let encoded = base64::encode_payload_array(arr);
                    Some(Ok(Payload::String(encoded)))
                }
                _ => Some(Err(TemporalError::TypeMismatch(
                    "base64_encode expects array".to_string(),
                ))),
            }
        }
        "base64_decode" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(
                    "base64_decode expects (string)".to_string(),
                )));
            }
            match &args[0] {
                Payload::String(s) => {
                    let decoded = base64::decode_to_payload_array(s)
                        .map_err(TemporalError::EvalError);
                    Some(decoded.map(Payload::Array))
                }
                _ => Some(Err(TemporalError::TypeMismatch(
                    "base64_decode expects string".to_string(),
                ))),
            }
        }
        "base64_encode_chunk" => {
            if args.len() != 3 {
                return Some(Err(TemporalError::EvalError(
                    "base64_encode_chunk expects (b0, b1, b2)".to_string(),
                )));
            }
            let b0 = match args[0] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let b1 = match args[1] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let b2 = match args[2] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let chunk = base64::encode_chunk(b0, b1, b2);
            Some(Ok(Payload::Array(
                chunk.iter().map(|b| Payload::Integer(*b as i64)).collect(),
            )))
        }
        "base64_decode_chunk" => {
            if args.len() != 4 {
                return Some(Err(TemporalError::EvalError(
                    "base64_decode_chunk expects (c0, c1, c2, c3)".to_string(),
                )));
            }
            let c0 = match args[0] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let c1 = match args[1] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let c2 = match args[2] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let c3 = match args[3] {
                Payload::Integer(i) => i as u8,
                _ => 0,
            };
            let chunk = match base64::decode_chunk(c0, c1, c2, c3) {
                Ok(ch) => ch,
                Err(e) => return Some(Err(TemporalError::EvalError(e))),
            };
            Some(Ok(Payload::Array(
                chunk.iter().map(|b| Payload::Integer(*b as i64)).collect(),
            )))
        }
        _ => None,
    }
}

fn dispatch_binary(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    match name {
        "binary_write_u16_be" => {
            let val = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            Some(Ok(Payload::Array(binary::write_u16_be(val))))
        }
        "binary_write_u16_le" => {
            let val = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            Some(Ok(Payload::Array(binary::write_u16_le(val))))
        }
        "binary_write_u32_be" => {
            let val = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            Some(Ok(Payload::Array(binary::write_u32_be(val))))
        }
        "binary_write_u32_le" => {
            let val = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            Some(Ok(Payload::Array(binary::write_u32_le(val))))
        }
        "binary_write_u64_be" => {
            let val = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            Some(Ok(Payload::Array(binary::write_u64_be(val))))
        }
        "binary_write_u64_le" => {
            let val = match args.first() {
                Some(Payload::Integer(i)) => *i,
                _ => 0,
            };
            Some(Ok(Payload::Array(binary::write_u64_le(val))))
        }
        "binary_read_u16_be" => match args.first() {
            Some(Payload::Array(arr)) => {
                Some(Ok(Payload::Integer(binary::read_u16_be(arr))))
            }
            _ => Some(Ok(Payload::Integer(0))),
        },
        "binary_read_u16_le" => match args.first() {
            Some(Payload::Array(arr)) => {
                Some(Ok(Payload::Integer(binary::read_u16_le(arr))))
            }
            _ => Some(Ok(Payload::Integer(0))),
        },
        "binary_read_u32_be" => match args.first() {
            Some(Payload::Array(arr)) => {
                Some(Ok(Payload::Integer(binary::read_u32_be(arr))))
            }
            _ => Some(Ok(Payload::Integer(0))),
        },
        "binary_read_u32_le" => match args.first() {
            Some(Payload::Array(arr)) => {
                Some(Ok(Payload::Integer(binary::read_u32_le(arr))))
            }
            _ => Some(Ok(Payload::Integer(0))),
        },
        "binary_read_u64_be" => match args.first() {
            Some(Payload::Array(arr)) => {
                Some(Ok(Payload::Integer(binary::read_u64_be(arr))))
            }
            _ => Some(Ok(Payload::Integer(0))),
        },
        "binary_read_u64_le" => match args.first() {
            Some(Payload::Array(arr)) => {
                Some(Ok(Payload::Integer(binary::read_u64_le(arr))))
            }
            _ => Some(Ok(Payload::Integer(0))),
        },
        _ => None,
    }
}

fn dispatch_collections(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    match name {
        "push" | "array_push" => {
            if args.len() != 2 {
                return Some(Err(TemporalError::EvalError(format!(
                    "{} expects (array, element)",
                    name
                ))));
            }
            match &args[0] {
                Payload::Array(arr) => {
                    let mut new_arr = arr.clone();
                    new_arr.push(args[1].clone());
                    Some(Ok(Payload::Array(new_arr)))
                }
                _ => Some(Err(TemporalError::TypeMismatch(format!(
                    "{} expects first argument to be array",
                    name
                )))),
            }
        }
        "pop" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(
                    "pop expects (array)".to_string(),
                )));
            }
            match &args[0] {
                Payload::Array(arr) => {
                    Some(Ok(arr.last().cloned().unwrap_or(Payload::Null)))
                }
                _ => Some(Err(TemporalError::TypeMismatch(
                    "pop expects argument to be array".to_string(),
                ))),
            }
        }
        "array_slice" => {
            if args.len() != 3 {
                return Some(Err(TemporalError::EvalError(
                    "array_slice expects (array, start, end)".to_string(),
                )));
            }
            let start = match args[1] {
                Payload::Integer(i) => i.max(0) as usize,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "array_slice start must be integer".to_string(),
                    )))
                }
            };
            let end = match args[2] {
                Payload::Integer(i) => i.max(0) as usize,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "array_slice end must be integer".to_string(),
                    )))
                }
            };
            match &args[0] {
                Payload::Array(arr) => {
                    let clamped_start = start.min(arr.len());
                    let clamped_end = end.min(arr.len()).max(clamped_start);
                    Some(Ok(Payload::Array(
                        arr[clamped_start..clamped_end].to_vec(),
                    )))
                }
                _ => Some(Err(TemporalError::TypeMismatch(
                    "array_slice expects array".to_string(),
                ))),
            }
        }
        "string_from_bytes" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(
                    "string_from_bytes expects (array_of_bytes)".to_string(),
                )));
            }
            match &args[0] {
                Payload::Array(arr) => {
                    let bytes: Vec<u8> = arr
                        .iter()
                        .filter_map(|p| match p {
                            Payload::Integer(i) => Some(*i as u8),
                            _ => None,
                        })
                        .collect();
                    let s = String::from_utf8_lossy(&bytes).to_string();
                    Some(Ok(Payload::String(s)))
                }
                _ => Some(Err(TemporalError::TypeMismatch(
                    "string_from_bytes expects array".to_string(),
                ))),
            }
        }
        "char_at" => {
            if args.len() != 2 {
                return Some(Err(TemporalError::EvalError(
                    "char_at expects (string, index)".to_string(),
                )));
            }
            let s = match &args[0] {
                Payload::String(s) => s,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "char_at expects string".to_string(),
                    )))
                }
            };
            let idx = match args[1] {
                Payload::Integer(i) => i as usize,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "char_at expects index".to_string(),
                    )))
                }
            };
            let c = s.chars().nth(idx).map(|c| c as i64).unwrap_or(0);
            Some(Ok(Payload::Integer(c)))
        }
        "str_slice" => {
            if args.len() != 3 {
                return Some(Err(TemporalError::EvalError(
                    "str_slice expects (string, start, end)".to_string(),
                )));
            }
            let s = match &args[0] {
                Payload::String(s) => s,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "str_slice expects string".to_string(),
                    )))
                }
            };
            let start = match args[1] {
                Payload::Integer(i) => i.max(0) as usize,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "str_slice start must be integer".to_string(),
                    )))
                }
            };
            let end = match args[2] {
                Payload::Integer(i) => i.max(0) as usize,
                _ => {
                    return Some(Err(TemporalError::TypeMismatch(
                        "str_slice end must be integer".to_string(),
                    )))
                }
            };
            let chars: Vec<char> = s.chars().collect();
            let clamped_start = start.min(chars.len());
            let clamped_end = end.min(chars.len()).max(clamped_start);
            let sub: String = chars[clamped_start..clamped_end].iter().collect();
            Some(Ok(Payload::String(sub)))
        }
        _ => None,
    }
}

fn dispatch_math(
    name: &str,
    args: &[Payload],
) -> Option<Result<Payload, TemporalError>> {
    match name {
        "sqrt" | "sin" | "cos" | "tan" | "exp" | "ln" | "log10" | "floor"
        | "ceil" | "round" => {
            if args.len() != 1 {
                return Some(Err(TemporalError::EvalError(format!(
                    "{} expects 1 argument",
                    name
                ))));
            }
            let f = match args[0].as_float() {
                Some(val) => val,
                None => {
                    return Some(Err(TemporalError::TypeMismatch(format!(
                        "{} expects numeric",
                        name
                    ))))
                }
            };
            let res = match name {
                "sqrt" => f.sqrt(),
                "sin" => f.sin(),
                "cos" => f.cos(),
                "tan" => f.tan(),
                "exp" => f.exp(),
                "ln" => f.ln(),
                "log10" => f.log10(),
                "floor" => f.floor(),
                "ceil" => f.ceil(),
                "round" => f.round(),
                _ => unreachable!(),
            };
            Some(Ok(Payload::Float(res.to_bits())))
        }
        _ => None,
    }
}
