use causm_runtime::vm::Vm;

pub fn register_net_capabilities(vm: &mut Vm) {
    vm.register_capability("System.NetworkFetch", |params| {
        let url = params.get("url").cloned().unwrap_or_default();
        Ok(causm_core::value::Payload::String(format!(
            "Simulated payload for {}",
            url
        )))
    });
}
