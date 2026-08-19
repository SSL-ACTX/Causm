use causm_runtime::vm::Vm;

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

pub fn get_module(path: &str) -> Option<&'static str> {
    match path {
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
        _ => None,
    }
}

pub fn register_all(vm: &mut Vm) {
    io::register_io_capabilities(vm);
    net::register_net_capabilities(vm);
}
