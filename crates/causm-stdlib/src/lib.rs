use causm_runtime::vm::Vm;

pub mod archive;
pub mod io;
pub mod net;

pub const STD_FS_MOD: &str = include_str!("../csm/std/fs/mod.csm");
pub const STD_FS_TYPES: &str = include_str!("../csm/std/fs/types.csm");
pub const STD_FS_FFI: &str = include_str!("../csm/std/fs/ffi.csm");
pub const STD_FS_OPS: &str = include_str!("../csm/std/fs/ops.csm");
pub const STD_ENV: &str = include_str!("../csm/std/env.csm");
pub const STD_PATH: &str = include_str!("../csm/std/path.csm");
pub const STD_TIME_MOD: &str = include_str!("../csm/std/time/mod.csm");
pub const STD_TIME_TYPES: &str = include_str!("../csm/std/time/types.csm");
pub const STD_TIME_FFI: &str = include_str!("../csm/std/time/ffi.csm");
pub const STD_TIME_OPS: &str = include_str!("../csm/std/time/ops.csm");
pub const STD_NET_MOD: &str = include_str!("../csm/std/net/mod.csm");
pub const STD_NET_TYPES: &str = include_str!("../csm/std/net/types.csm");
pub const STD_NET_FFI: &str = include_str!("../csm/std/net/ffi.csm");
pub const STD_NET_OPS: &str = include_str!("../csm/std/net/ops.csm");
pub const STD_ENCODING_MOD: &str = include_str!("../csm/std/encoding/mod.csm");
pub const STD_ENCODING_TYPES: &str = include_str!("../csm/std/encoding/types.csm");
pub const STD_ENCODING_UTF8: &str = include_str!("../csm/std/encoding/utf8.csm");
pub const STD_ENCODING_BASE64: &str = include_str!("../csm/std/encoding/base64.csm");
pub const STD_ENCODING_BINARY: &str = include_str!("../csm/std/encoding/binary.csm");
pub const STD_PROCESS_MOD: &str = include_str!("../csm/std/process/mod.csm");
pub const STD_PROCESS_TYPES: &str = include_str!("../csm/std/process/types.csm");
pub const STD_PROCESS_FFI: &str = include_str!("../csm/std/process/ffi.csm");
pub const STD_PROCESS_OPS: &str = include_str!("../csm/std/process/ops.csm");
pub const STD_COLLECTION_MOD: &str = include_str!("../csm/std/collection/mod.csm");
pub const STD_COLLECTION_TYPES: &str =
    include_str!("../csm/std/collection/types.csm");
pub const STD_COLLECTION_ARRAY: &str =
    include_str!("../csm/std/collection/array.csm");
pub const STD_COLLECTION_BUFFER: &str =
    include_str!("../csm/std/collection/buffer.csm");
pub const STD_COLLECTION_STACK: &str =
    include_str!("../csm/std/collection/stack.csm");
pub const STD_COLLECTION_QUEUE: &str =
    include_str!("../csm/std/collection/queue.csm");
pub const STD_COLLECTION_RING_BUFFER: &str =
    include_str!("../csm/std/collection/ring_buffer.csm");
pub const STD_COLLECTION_BITSET: &str =
    include_str!("../csm/std/collection/bitset.csm");

pub const STD_CORE_MOD: &str = include_str!("../csm/std/core/mod.csm");
pub const STD_CORE_TYPES: &str = include_str!("../csm/std/core/types.csm");
pub const STD_CORE_OPS: &str = include_str!("../csm/std/core/ops.csm");

pub const STD_HTTP_MOD: &str = include_str!("../csm/std/http/mod.csm");
pub const STD_HTTP_TYPES: &str = include_str!("../csm/std/http/types.csm");
pub const STD_HTTP_OPS: &str = include_str!("../csm/std/http/ops.csm");

pub const STD_JSON_MOD: &str = include_str!("../csm/std/json/mod.csm");
pub const STD_JSON_TYPES: &str = include_str!("../csm/std/json/types.csm");
pub const STD_JSON_OPS: &str = include_str!("../csm/std/json/ops.csm");
pub const STD_JSON_DECODE: &str = include_str!("../csm/std/json/decode.csm");
pub const STD_JSON_ENCODE: &str = include_str!("../csm/std/json/encode.csm");

pub const STD_SYNC_MOD: &str = include_str!("../csm/std/sync/mod.csm");
pub const STD_SYNC_TYPES: &str = include_str!("../csm/std/sync/types.csm");
pub const STD_SYNC_ATOMIC: &str = include_str!("../csm/std/sync/atomic.csm");
pub const STD_SYNC_MUTEX: &str = include_str!("../csm/std/sync/mutex.csm");
pub const STD_SYNC_CHANNEL: &str = include_str!("../csm/std/sync/channel.csm");

pub fn get_module(path: &str) -> Option<&'static str> {
    match path {
        "std::core" | "std/core" => Some(STD_CORE_MOD),
        "std::core::types" | "std/core/types" => Some(STD_CORE_TYPES),
        "std::core::ops" | "std/core/ops" => Some(STD_CORE_OPS),
        "std::http" | "std/http" => Some(STD_HTTP_MOD),
        "std::http::types" | "std/http/types" => Some(STD_HTTP_TYPES),
        "std::http::ops" | "std/http/ops" => Some(STD_HTTP_OPS),
        "std::fs" | "std/fs" => Some(STD_FS_MOD),
        "std::fs::types" | "std/fs/types" => Some(STD_FS_TYPES),
        "std::fs::ffi" | "std/fs/ffi" => Some(STD_FS_FFI),
        "std::fs::ops" | "std/fs/ops" => Some(STD_FS_OPS),
        "std::env" | "std/env" => Some(STD_ENV),
        "std::path" | "std/path" => Some(STD_PATH),
        "std::time" | "std/time" => Some(STD_TIME_MOD),
        "std::time::types" | "std/time/types" => Some(STD_TIME_TYPES),
        "std::time::ffi" | "std/time/ffi" => Some(STD_TIME_FFI),
        "std::time::ops" | "std/time/ops" => Some(STD_TIME_OPS),
        "std::net" | "std/net" => Some(STD_NET_MOD),
        "std::net::types" | "std/net/types" => Some(STD_NET_TYPES),
        "std::net::ffi" | "std/net/ffi" => Some(STD_NET_FFI),
        "std::net::ops" | "std/net/ops" => Some(STD_NET_OPS),
        "std::encoding" | "std/encoding" => Some(STD_ENCODING_MOD),
        "std::encoding::types" | "std/encoding/types" => Some(STD_ENCODING_TYPES),
        "std::encoding::utf8" | "std/encoding/utf8" => Some(STD_ENCODING_UTF8),
        "std::encoding::base64" | "std/encoding/base64" => Some(STD_ENCODING_BASE64),
        "std::encoding::binary" | "std/encoding/binary" => Some(STD_ENCODING_BINARY),
        "std::process" | "std/process" => Some(STD_PROCESS_MOD),
        "std::process::types" | "std/process/types" => Some(STD_PROCESS_TYPES),
        "std::process::ffi" | "std/process/ffi" => Some(STD_PROCESS_FFI),
        "std::process::ops" | "std/process/ops" => Some(STD_PROCESS_OPS),
        "std::collection" | "std/collection" => Some(STD_COLLECTION_MOD),
        "std::collection::types" | "std/collection/types" => {
            Some(STD_COLLECTION_TYPES)
        }
        "std::collection::array" | "std/collection/array" => {
            Some(STD_COLLECTION_ARRAY)
        }
        "std::collection::buffer" | "std/collection/buffer" => {
            Some(STD_COLLECTION_BUFFER)
        }
        "std::collection::stack" | "std/collection/stack" => {
            Some(STD_COLLECTION_STACK)
        }
        "std::collection::queue" | "std/collection/queue" => {
            Some(STD_COLLECTION_QUEUE)
        }
        "std::collection::ring_buffer" | "std/collection/ring_buffer" => {
            Some(STD_COLLECTION_RING_BUFFER)
        }
        "std::collection::bitset" | "std/collection/bitset" => {
            Some(STD_COLLECTION_BITSET)
        }
        "std::json" | "std/json" | "std::encoding::json" | "std/encoding/json" => {
            Some(STD_JSON_MOD)
        }
        "std::json::types"
        | "std/json/types"
        | "std::encoding::json::types"
        | "std/encoding/json/types" => Some(STD_JSON_TYPES),
        "std::json::ops"
        | "std/json/ops"
        | "std::encoding::json::ops"
        | "std/encoding/json/ops" => Some(STD_JSON_OPS),
        "std::json::decode"
        | "std/json/decode"
        | "std::encoding::json::decode"
        | "std/encoding/json/decode" => Some(STD_JSON_DECODE),
        "std::json::encode"
        | "std/json/encode"
        | "std::encoding::json::encode"
        | "std/encoding/json/encode" => Some(STD_JSON_ENCODE),
        "std::sync" | "std/sync" => Some(STD_SYNC_MOD),
        "std::sync::types" | "std/sync/types" => Some(STD_SYNC_TYPES),
        "std::sync::atomic" | "std/sync/atomic" => Some(STD_SYNC_ATOMIC),
        "std::sync::mutex" | "std/sync/mutex" => Some(STD_SYNC_MUTEX),
        "std::sync::channel" | "std/sync/channel" => Some(STD_SYNC_CHANNEL),
        _ => None,
    }
}

pub fn all_embedded_modules() -> Vec<(&'static str, &'static str)> {
    vec![
        ("std/core", STD_CORE_MOD),
        ("std/core/types", STD_CORE_TYPES),
        ("std/core/ops", STD_CORE_OPS),
        ("std/http", STD_HTTP_MOD),
        ("std/http/types", STD_HTTP_TYPES),
        ("std/http/ops", STD_HTTP_OPS),
        ("std/fs", STD_FS_MOD),
        ("std/fs/types", STD_FS_TYPES),
        ("std/fs/ffi", STD_FS_FFI),
        ("std/fs/ops", STD_FS_OPS),
        ("std/env", STD_ENV),
        ("std/path", STD_PATH),
        ("std/time", STD_TIME_MOD),
        ("std/time/types", STD_TIME_TYPES),
        ("std/time/ffi", STD_TIME_FFI),
        ("std/time/ops", STD_TIME_OPS),
        ("std/net", STD_NET_MOD),
        ("std/net/types", STD_NET_TYPES),
        ("std/net/ffi", STD_NET_FFI),
        ("std/net/ops", STD_NET_OPS),
        ("std/encoding", STD_ENCODING_MOD),
        ("std/encoding/types", STD_ENCODING_TYPES),
        ("std/encoding/utf8", STD_ENCODING_UTF8),
        ("std/encoding/base64", STD_ENCODING_BASE64),
        ("std/encoding/binary", STD_ENCODING_BINARY),
        ("std/process", STD_PROCESS_MOD),
        ("std/process/types", STD_PROCESS_TYPES),
        ("std/process/ffi", STD_PROCESS_FFI),
        ("std/process/ops", STD_PROCESS_OPS),
        ("std/collection", STD_COLLECTION_MOD),
        ("std/collection/types", STD_COLLECTION_TYPES),
        ("std/collection/array", STD_COLLECTION_ARRAY),
        ("std/collection/buffer", STD_COLLECTION_BUFFER),
        ("std/collection/stack", STD_COLLECTION_STACK),
        ("std/collection/queue", STD_COLLECTION_QUEUE),
        ("std/collection/ring_buffer", STD_COLLECTION_RING_BUFFER),
        ("std/collection/bitset", STD_COLLECTION_BITSET),
        ("std/json", STD_JSON_MOD),
        ("std/json/types", STD_JSON_TYPES),
        ("std/json/ops", STD_JSON_OPS),
        ("std/json/decode", STD_JSON_DECODE),
        ("std/json/encode", STD_JSON_ENCODE),
        ("std/sync", STD_SYNC_MOD),
        ("std/sync/types", STD_SYNC_TYPES),
        ("std/sync/atomic", STD_SYNC_ATOMIC),
        ("std/sync/mutex", STD_SYNC_MUTEX),
        ("std/sync/channel", STD_SYNC_CHANNEL),
    ]
}

pub fn register_all(vm: &mut Vm) {
    io::register_io_capabilities(vm);
    net::register_net_capabilities(vm);
}
