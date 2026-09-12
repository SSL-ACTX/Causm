//! Declarative definitions of compiler builtin signatures and intrinsics.
//!
//! Provides a scalable, single-source-of-truth macro registry for all builtins
//! registered in the type analyzer.

use causm_core::types::Type;
use causm_core::ParamMode;

pub struct BuiltinDef {
    pub name: &'static str,
    pub params: Vec<Type>,
    pub return_type: Type,
    pub param_mode: ParamMode,
}

macro_rules! define_builtins {
    ($(
        $(#[$meta:meta])*
        [$mode:ident] $name:literal : ($($arg:expr),* $(,)?) -> $ret:expr
    );* $(;)?) => {
        pub fn all_builtins() -> Vec<BuiltinDef> {
            vec![
                $(
                    BuiltinDef {
                        name: $name,
                        params: vec![$($arg),*],
                        return_type: $ret,
                        param_mode: ParamMode::$mode,
                    },
                )*
            ]
        }
    };
}

define_builtins! {
    // --- Math Intrinsics ---
    [Clone] "sqrt" : (Type::Float) -> Type::Float;
    [Clone] "sin" : (Type::Float) -> Type::Float;
    [Clone] "cos" : (Type::Float) -> Type::Float;
    [Clone] "tan" : (Type::Float) -> Type::Float;
    [Clone] "exp" : (Type::Float) -> Type::Float;
    [Clone] "ln" : (Type::Float) -> Type::Float;
    [Clone] "log10" : (Type::Float) -> Type::Float;
    [Clone] "floor" : (Type::Float) -> Type::Float;
    [Clone] "ceil" : (Type::Float) -> Type::Float;
    [Clone] "round" : (Type::Float) -> Type::Float;

    // --- Collection Intrinsics ---
    [Peek] "push" : (Type::Array(Box::new(Type::Unknown)), Type::Unknown) -> Type::Array(Box::new(Type::Unknown));
    [Peek] "pop" : (Type::Array(Box::new(Type::Unknown))) -> Type::Unknown;
    [Peek] "array_push" : (Type::Array(Box::new(Type::Unknown)), Type::Unknown) -> Type::Array(Box::new(Type::Unknown));
    [Peek] "array_slice" : (Type::Array(Box::new(Type::Unknown)), Type::Integer, Type::Integer) -> Type::Array(Box::new(Type::Unknown));
    [Peek] "string_from_bytes" : (Type::Array(Box::new(Type::Integer))) -> Type::String;
    [Peek] "char_at" : (Type::String, Type::Integer) -> Type::Integer;
    [Peek] "str_slice" : (Type::String, Type::Integer, Type::Integer) -> Type::String;

    // --- JSON Encoding ---
    [Peek] "json_parse" : (Type::String) -> Type::Unknown;
    [Peek] "json_stringify" : (Type::Unknown) -> Type::String;

    // --- Base64 Encoding ---
    [Peek] "base64_encode" : (Type::Array(Box::new(Type::Integer))) -> Type::String;
    [Peek] "base64_decode" : (Type::String) -> Type::Array(Box::new(Type::Integer));
    [Peek] "base64_encode_chunk" : (Type::Integer, Type::Integer, Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "base64_decode_chunk" : (Type::Integer, Type::Integer, Type::Integer, Type::Integer) -> Type::Array(Box::new(Type::Integer));

    // --- Binary Packing/Unpacking ---
    [Peek] "binary_write_u16_be" : (Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "binary_write_u16_le" : (Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "binary_write_u32_be" : (Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "binary_write_u32_le" : (Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "binary_write_u64_be" : (Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "binary_write_u64_le" : (Type::Integer) -> Type::Array(Box::new(Type::Integer));
    [Peek] "binary_read_u16_be" : (Type::Array(Box::new(Type::Integer))) -> Type::Integer;
    [Peek] "binary_read_u16_le" : (Type::Array(Box::new(Type::Integer))) -> Type::Integer;
    [Peek] "binary_read_u32_be" : (Type::Array(Box::new(Type::Integer))) -> Type::Integer;
    [Peek] "binary_read_u32_le" : (Type::Array(Box::new(Type::Integer))) -> Type::Integer;
    [Peek] "binary_read_u64_be" : (Type::Array(Box::new(Type::Integer))) -> Type::Integer;
    [Peek] "binary_read_u64_le" : (Type::Array(Box::new(Type::Integer))) -> Type::Integer;

    // --- Sync Primitives ---
    [Peek] "__sync_atomic_new_int" : (Type::Integer) -> Type::Unknown;
    [Peek] "__sync_atomic_load_int" : (Type::Unknown) -> Type::Integer;
    [Peek] "__sync_atomic_store_int" : (Type::Unknown, Type::Integer) -> Type::Unknown;
    [Peek] "__sync_atomic_fetch_add" : (Type::Unknown, Type::Integer) -> Type::Unknown;
    [Peek] "__sync_atomic_cas_int" : (Type::Unknown, Type::Integer, Type::Integer) -> Type::Unknown;
    [Peek] "__sync_atomic_new_bool" : (Type::Bool) -> Type::Unknown;
    [Peek] "__sync_atomic_load_bool" : (Type::Unknown) -> Type::Bool;
    [Peek] "__sync_atomic_store_bool" : (Type::Unknown, Type::Bool) -> Type::Unknown;
    [Peek] "__sync_atomic_cas_bool" : (Type::Unknown, Type::Bool, Type::Bool) -> Type::Unknown;
    [Peek] "__sync_mutex_new" : () -> Type::Unknown;
    [Peek] "__sync_mutex_try_lock" : (Type::Unknown, Type::String) -> Type::Unknown;
    [Peek] "__sync_mutex_unlock" : (Type::Unknown) -> Type::Unknown;
    [Peek] "__sync_mutex_is_locked" : (Type::Unknown) -> Type::Bool;
    [Peek] "__sync_mutex_owner" : (Type::Unknown) -> Type::String;
    [Peek] "__sync_channel_new" : (Type::Integer) -> Type::Unknown;
    [Peek] "__sync_channel_send" : (Type::Unknown, Type::Integer) -> Type::Unknown;
    [Peek] "__sync_channel_recv" : (Type::Unknown) -> Type::Unknown;
    [Peek] "__sync_channel_close" : (Type::Unknown) -> Type::Unknown;
    [Peek] "__sync_channel_is_closed" : (Type::Unknown) -> Type::Bool;
    [Peek] "__sync_channel_len" : (Type::Unknown) -> Type::Integer;
    [Peek] "__sync_channel_is_full" : (Type::Unknown) -> Type::Bool;
    [Peek] "__sync_channel_is_empty" : (Type::Unknown) -> Type::Bool;
}
