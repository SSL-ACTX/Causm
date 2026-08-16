use crate::vm::error::TemporalError;
use causm_core::types::Type;
use causm_core::value::Payload;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::Mutex;

#[cfg(unix)]
type RawHandle = *mut libc::c_void;

#[cfg(not(unix))]
type RawHandle = *mut std::ffi::c_void;

pub struct ForeignLibraryManager {
    handles: Mutex<HashMap<String, usize>>,
}

impl Default for ForeignLibraryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ForeignLibraryManager {
    pub fn new() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(unix)]
    pub fn get_or_load_symbol(
        &self,
        lib_name: &str,
        symbol: &str,
    ) -> Result<*mut libc::c_void, TemporalError> {
        let mut map = self.handles.lock().unwrap();
        let handle = if let Some(&h) = map.get(lib_name) {
            h as RawHandle
        } else {
            // Attempt to load library using dlopen
            let c_lib = CString::new(lib_name).map_err(|_| {
                TemporalError::EvalError(format!(
                    "Invalid library path: {}",
                    lib_name
                ))
            })?;

            let mut h = unsafe { libc::dlopen(c_lib.as_ptr(), libc::RTLD_LAZY) };
            if h.is_null() {
                // If lib_name is "libc.so.6" and on Android/Termux, try "libc.so" or RTLD_DEFAULT
                if lib_name.contains("libc") {
                    let fallback_name = CString::new("libc.so").unwrap();
                    h = unsafe {
                        libc::dlopen(fallback_name.as_ptr(), libc::RTLD_LAZY)
                    };
                }
            }
            if h.is_null() {
                // Try RTLD_DEFAULT
                h = libc::RTLD_DEFAULT;
            }

            map.insert(lib_name.to_string(), h as usize);
            h
        };

        let c_sym = CString::new(symbol).map_err(|_| {
            TemporalError::EvalError(format!("Invalid symbol name: {}", symbol))
        })?;

        let sym_ptr = unsafe { libc::dlsym(handle, c_sym.as_ptr()) };
        if sym_ptr.is_null() {
            // Try RTLD_DEFAULT as fallback lookup
            let default_ptr =
                unsafe { libc::dlsym(libc::RTLD_DEFAULT, c_sym.as_ptr()) };
            if !default_ptr.is_null() {
                return Ok(default_ptr);
            }
            return Err(TemporalError::EvalError(format!(
                "FFI symbol '{}' not found in '{}'",
                symbol, lib_name
            )));
        }

        Ok(sym_ptr)
    }

    #[cfg(not(unix))]
    pub fn get_or_load_symbol(
        &self,
        lib_name: &str,
        symbol: &str,
    ) -> Result<RawHandle, TemporalError> {
        Err(TemporalError::EvalError(format!(
            "Dynamic FFI dlopen not supported on non-unix platform for {}::{}",
            lib_name, symbol
        )))
    }
}

/// Invokes raw C function pointer with dynamically marshalled arguments.
///
/// # Safety
///
/// `sym_ptr` must be a valid, callable C function pointer matching the native ABI.
pub unsafe fn invoke_foreign_symbol(
    sym_ptr: *mut libc::c_void,
    args: &mut [Payload],
    return_type: &Type,
) -> Result<Payload, TemporalError> {
    // Keep CString allocations alive for the duration of the native call
    let mut c_strings = Vec::new();
    let mut raw_args: Vec<usize> = Vec::new();

    // Struct buffers allocated on the heap: (arg_index, sorted_keys, buffer)
    let mut struct_buffers: Vec<(usize, Vec<String>, Vec<u8>)> = Vec::new();

    for (arg_idx, arg) in args.iter().enumerate() {
        match arg {
            Payload::Integer(i) => {
                raw_args.push(*i as usize);
            }
            Payload::Float(bits) => {
                raw_args.push(*bits as usize);
            }
            Payload::Bool(b) => {
                raw_args.push(if *b { 1 } else { 0 });
            }
            Payload::String(s) => {
                let c_str = CString::new(s.as_str()).map_err(|_| {
                    TemporalError::EvalError("String contains null byte".to_string())
                })?;
                raw_args.push(c_str.as_ptr() as usize);
                c_strings.push(c_str);
            }
            Payload::Null => {
                raw_args.push(0);
            }
            Payload::Struct(map) => {
                // Marshall 64-bit integer fields consecutively into a C-compatible memory buffer.
                // If the struct has POSIX timespec fields, place tv_sec first, then tv_nsec.
                let mut buf = Vec::with_capacity(map.len() * 8);
                let sorted_keys: Vec<String> = if map.contains_key("tv_sec")
                    && map.contains_key("tv_nsec")
                {
                    let mut keys = vec!["tv_sec".to_string(), "tv_nsec".to_string()];
                    for k in map.keys() {
                        if k != "tv_sec" && k != "tv_nsec" {
                            keys.push(k.clone());
                        }
                    }
                    keys
                } else {
                    let mut keys: Vec<String> = map.keys().cloned().collect();
                    keys.sort();
                    keys
                };
                for k in &sorted_keys {
                    let val_i64 = match map.get(k) {
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Integer(i),
                        )) => *i,
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Float(bits),
                        )) => *bits as i64,
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Bool(true),
                        )) => 1,
                        Some(causm_core::value::EntropicState::Valid(
                            Payload::Bool(false),
                        )) => 0,
                        _ => 0i64,
                    };
                    buf.extend_from_slice(&val_i64.to_ne_bytes());
                }
                let ptr = buf.as_mut_ptr() as usize;
                struct_buffers.push((arg_idx, sorted_keys, buf));
                raw_args.push(ptr);
            }
            Payload::Topology(_) | Payload::Array(_) => {
                return Err(TemporalError::EvalError(
                    "Passing complex array/topology types directly to raw C FFI is unsupported"
                        .to_string(),
                ));
            }
        }
    }

    // Call function pointer based on arity
    let result_raw: usize = match raw_args.len() {
        0 => {
            // For routines returning String with 0 args (e.g. getcwd(NULL, 0)), pass (NULL, 0) or call directly
            if let Type::String = return_type {
                let func: extern "C" fn(usize, usize) -> usize =
                    unsafe { std::mem::transmute(sym_ptr) };
                func(0, 0)
            } else {
                let func: extern "C" fn() -> usize =
                    unsafe { std::mem::transmute(sym_ptr) };
                func()
            }
        }
        1 => {
            let func: extern "C" fn(usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0])
        }
        2 => {
            let func: extern "C" fn(usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0], raw_args[1])
        }
        3 => {
            let func: extern "C" fn(usize, usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0], raw_args[1], raw_args[2])
        }
        4 => {
            let func: extern "C" fn(usize, usize, usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(raw_args[0], raw_args[1], raw_args[2], raw_args[3])
        }
        5 => {
            let func: extern "C" fn(usize, usize, usize, usize, usize) -> usize =
                unsafe { std::mem::transmute(sym_ptr) };
            func(
                raw_args[0],
                raw_args[1],
                raw_args[2],
                raw_args[3],
                raw_args[4],
            )
        }
        6 => {
            let func: extern "C" fn(
                usize,
                usize,
                usize,
                usize,
                usize,
                usize,
            ) -> usize = unsafe { std::mem::transmute(sym_ptr) };
            func(
                raw_args[0],
                raw_args[1],
                raw_args[2],
                raw_args[3],
                raw_args[4],
                raw_args[5],
            )
        }
        _ => {
            return Err(TemporalError::EvalError(format!(
                "Foreign function calls with > 6 arguments ({}) not supported",
                raw_args.len()
            )));
        }
    };

    // Write back any modified bytes from C struct buffers into the Causm arguments
    for (arg_idx, sorted_keys, buf) in struct_buffers {
        if let Payload::Struct(ref mut map) = args[arg_idx] {
            for (i, k) in sorted_keys.into_iter().enumerate() {
                let offset = i * 8;
                if offset + 8 <= buf.len() {
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&buf[offset..offset + 8]);
                    let val_i64 = i64::from_ne_bytes(bytes);
                    map.insert(
                        k,
                        causm_core::value::EntropicState::Valid(Payload::Integer(
                            val_i64,
                        )),
                    );
                }
            }
        }
    }
    match return_type {
        Type::Integer
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::Unknown => Ok(Payload::Integer(result_raw as i64)),
        Type::Bool => Ok(Payload::Bool(result_raw != 0)),
        Type::String => {
            if result_raw == 0 {
                Ok(Payload::Null)
            } else {
                let c_str =
                    unsafe { CStr::from_ptr(result_raw as *const libc::c_char) };
                let rust_str = c_str.to_string_lossy().to_string();
                Ok(Payload::String(rust_str))
            }
        }
        Type::Float | Type::F32 | Type::F64 => {
            let f = f64::from_bits(result_raw as u64);
            Ok(Payload::Float(f.to_bits()))
        }
        _ => Ok(Payload::Integer(result_raw as i64)),
    }
}
