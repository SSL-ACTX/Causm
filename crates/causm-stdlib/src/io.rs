use causm_runtime::vm::Vm;

pub fn register_io_capabilities(vm: &mut Vm) {
    vm.register_capability("System.Log", |params| {
        if let Some(msg) = params.get("message") {
            println!("{}", msg);
        }
        Ok(causm_core::value::Payload::Null)
    });
}
