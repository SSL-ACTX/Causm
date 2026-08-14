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

pub fn get_module(path: &str) -> Option<&'static str> {
    match path {
        "std::fs" | "std/fs" => Some(STD_FS_MOD),
        "std::fs::types" | "std/fs/types" => Some(STD_FS_TYPES),
        "std::fs::ffi" | "std/fs/ffi" => Some(STD_FS_FFI),
        "std::fs::ops" | "std/fs/ops" => Some(STD_FS_OPS),
        "std::env" | "std/env" => Some(STD_ENV),
        "std::path" | "std/path" => Some(STD_PATH),
        "std::time" | "std/time" => Some(STD_TIME_MOD),
        "std::time::types" | "std/time/types" | "std/time::types" => {
            Some(STD_TIME_TYPES)
        }
        "std::time::ffi" | "std/time/ffi" => Some(STD_TIME_FFI),
        "std::time::ops" | "std/time/ops" => Some(STD_TIME_OPS),
        "std::net" | "std/net" => Some(STD_NET_MOD),
        "std::net::types" | "std/net/types" | "std/net::types" => {
            Some(STD_NET_TYPES)
        }
        "std::net::ffi" | "std/net/ffi" => Some(STD_NET_FFI),
        "std::net::ops" | "std/net/ops" => Some(STD_NET_OPS),
        _ => None,
    }
}

pub fn register_all(vm: &mut Vm) {
    io::register_io_capabilities(vm);
    net::register_net_capabilities(vm);
}
