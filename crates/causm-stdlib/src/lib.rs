use causm_runtime::vm::Vm;

pub mod io;
pub mod net;

pub fn register_all(vm: &mut Vm) {
    io::register_io_capabilities(vm);
    net::register_net_capabilities(vm);
}
