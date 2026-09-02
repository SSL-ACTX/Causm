use crate::protocol::{PluginRequest, PluginResponse};
use anyhow::{bail, Context, Result};
use wasmi::{Config, Engine, Linker, Module, Store};

pub const DEFAULT_WASM_FUEL_BUDGET: u64 = 10_000_000; // 10M fuel units

pub struct WasmPluginDriver {
    wasm_bytes: Vec<u8>,
    max_memory_bytes: usize,
    fuel_budget: u64,
}

impl WasmPluginDriver {
    pub fn new(wasm_bytes: Vec<u8>) -> Self {
        Self {
            wasm_bytes,
            max_memory_bytes: 16 * 1024 * 1024, // 16 MB limit
            fuel_budget: DEFAULT_WASM_FUEL_BUDGET,
        }
    }

    pub fn with_fuel_budget(mut self, fuel_budget: u64) -> Self {
        self.fuel_budget = fuel_budget;
        self
    }

    pub fn transform(&self, request: &PluginRequest) -> Result<PluginResponse> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, &self.wasm_bytes)
            .context("Failed to parse WebAssembly plugin module")?;

        let mut store = Store::new(&engine, ());
        store
            .set_fuel(self.fuel_budget)
            .context("Failed to initialize fuel for WASM plugin store")?;

        let linker = Linker::new(&engine);
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .context("Failed to instantiate WASM plugin module")?;

        let memory = instance
            .get_export(&store, "memory")
            .and_then(|ext| ext.into_memory())
            .context("WASM plugin module does not export 'memory'")?;

        let alloc_fn = instance
            .get_typed_func::<u32, u32>(&store, "causm_plugin_alloc")
            .context(
                "WASM plugin does not export 'causm_plugin_alloc(len: u32) -> u32'",
            )?;

        let dealloc_fn = instance
            .get_typed_func::<(u32, u32), ()>(&store, "causm_plugin_dealloc")
            .context("WASM plugin does not export 'causm_plugin_dealloc(ptr: u32, len: u32)'")?;

        let transform_fn = instance
            .get_typed_func::<(u32, u32), u64>(&store, "causm_plugin_transform")
            .context("WASM plugin does not export 'causm_plugin_transform(ptr: u32, len: u32) -> u64'")?;

        let payload_bytes = bincode::serialize(request)
            .context("Failed to serialize PluginRequest with Bincode for WASM")?;

        let req_len = payload_bytes.len() as u32;
        let in_ptr = alloc_fn
            .call(&mut store, req_len)
            .context("Failed calling 'causm_plugin_alloc' in WASM plugin")?;

        memory
            .write(&mut store, in_ptr as usize, &payload_bytes)
            .context(
                "Failed writing request payload to WASM plugin linear memory",
            )?;

        let packed_res = transform_fn
            .call(&mut store, (in_ptr, req_len))
            .context("Failed executing 'causm_plugin_transform' in WASM plugin")?;

        let out_ptr = (packed_res >> 32) as u32;
        let out_len = (packed_res & 0xFFFF_FFFF) as u32;

        if out_len as usize > self.max_memory_bytes {
            bail!(
                "WASM plugin returned payload size {} bytes exceeding maximum memory ceiling of {} bytes",
                out_len,
                self.max_memory_bytes
            );
        }

        let mut out_bytes = vec![0u8; out_len as usize];
        memory
            .read(&store, out_ptr as usize, &mut out_bytes)
            .context("Failed reading response from WASM plugin linear memory")?;

        let _ = dealloc_fn.call(&mut store, (in_ptr, req_len));
        let _ = dealloc_fn.call(&mut store, (out_ptr, out_len));

        let response: PluginResponse = bincode::deserialize(&out_bytes).context(
            "Failed to deserialize PluginResponse from WASM plugin memory",
        )?;

        Ok(response)
    }
}
