use causm_runtime::vm::Vm;

pub mod io;
pub mod net;

pub const STD_FS_MOD: &str = include_str!("../csm/std/fs/mod.csm");
pub const STD_FS_TYPES: &str = include_str!("../csm/std/fs/types.csm");
pub const STD_FS_FFI: &str = include_str!("../csm/std/fs/ffi.csm");
pub const STD_FS_OPS: &str = include_str!("../csm/std/fs/ops.csm");
pub const STD_ENV: &str = include_str!("../csm/std/env.csm");
pub const STD_PATH: &str = include_str!("../csm/std/path.csm");

pub fn get_module(path: &str) -> Option<&'static str> {
    match path {
        "std::fs" | "std/fs" => Some(STD_FS_MOD),
        "std::fs::types" | "std/fs/types" => Some(STD_FS_TYPES),
        "std::fs::ffi" | "std/fs/ffi" => Some(STD_FS_FFI),
        "std::fs::ops" | "std/fs/ops" => Some(STD_FS_OPS),
        "std::env" | "std/env" => Some(STD_ENV),
        "std::path" | "std/path" => Some(STD_PATH),
        _ => None,
    }
}

pub fn register_all(vm: &mut Vm) {
    io::register_io_capabilities(vm);
    net::register_net_capabilities(vm);
}
