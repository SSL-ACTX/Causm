use causm_runtime::vm::Vm;

pub fn register_net_capabilities(vm: &mut Vm) {
    vm.register_capability("System.NetworkFetch", |params| {
        let url = params.get("url").cloned().unwrap_or_default();
        if url.starts_with("http://") || url.starts_with("https://") {
            println!("\x1b[1;34m[System.NetworkFetch]\x1b[0m HTTP GET {}", url);
            match ureq::get(&url).timeout(std::time::Duration::from_secs(5)).call() {
                Ok(response) => {
                    let body = response.into_string().unwrap_or_else(|_| "".to_string());
                    println!("\x1b[1;32m[System.NetworkFetch]\x1b[0m Response received ({} bytes)", body.len());
                    Ok(causm_core::value::Payload::String(body))
                }
                Err(e) => {
                    println!("\x1b[1;31m[System.NetworkFetch]\x1b[0m Error: {}", e);
                    Err(format!("NetworkFetch transport error: {}", e))
                }
            }
        } else {
            println!(
                "\x1b[1;34m[System.NetworkFetch]\x1b[0m Simulated fetch for internal endpoint: {}",
                url
            );
            Ok(causm_core::value::Payload::String(format!("Simulated payload for {}", url)))
        }
    });
}
