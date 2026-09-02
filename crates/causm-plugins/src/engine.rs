use crate::ipc::StdioPluginDriver;
use crate::protocol::{
    PluginDiagnostic, PluginRequest, PluginResponse, PluginStatus,
};
use crate::wasm::WasmPluginDriver;
use anyhow::{bail, Context, Result};
use causm_core::Program;
use std::collections::HashMap;
use std::path::Path;

pub enum PluginDriver {
    Wasm(WasmPluginDriver),
    Stdio(StdioPluginDriver),
}

impl PluginDriver {
    pub fn from_file_or_command(spec: &str) -> Result<Self> {
        let path = Path::new(spec);
        if path.exists() && path.extension().is_some_and(|ext| ext == "wasm") {
            let wasm_bytes = std::fs::read(path).with_context(|| {
                format!("Failed to read WASM plugin from '{}'", spec)
            })?;
            Ok(PluginDriver::Wasm(WasmPluginDriver::new(wasm_bytes)))
        } else {
            Ok(PluginDriver::Stdio(StdioPluginDriver::new(spec)))
        }
    }

    pub fn transform(&self, req: &PluginRequest) -> Result<PluginResponse> {
        match self {
            PluginDriver::Wasm(driver) => driver.transform(req),
            PluginDriver::Stdio(driver) => driver.transform(req),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PluginScope {
    pub targets: Vec<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl PluginScope {
    pub fn is_platform_supported(&self) -> bool {
        if self.targets.is_empty() {
            return true;
        }
        let current_target = if cfg!(target_arch = "wasm32") {
            "wasm32"
        } else {
            "native"
        };
        self.targets.iter().any(|t| {
            t == current_target
                || (current_target == "native"
                    && (t == "host"
                        || t == "unix"
                        || t == "linux"
                        || t == "macos"
                        || t == "windows"))
        })
    }

    pub fn matches_file(&self, file_path: &str) -> bool {
        // If excludes match, reject
        for excl in &self.exclude {
            if file_path.contains(excl) {
                return false;
            }
        }
        // If includes are specified, at least one must match
        if !self.include.is_empty() {
            return self.include.iter().any(|incl| file_path.contains(incl));
        }
        true
    }
}

pub struct RegisteredPlugin {
    pub name: String,
    pub driver: PluginDriver,
    pub options: HashMap<String, String>,
    pub scope: PluginScope,
}

#[derive(Default)]
pub struct PluginEngine {
    pub plugins: Vec<RegisteredPlugin>,
}

impl PluginEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_plugin(
        &mut self,
        name: impl Into<String>,
        driver: PluginDriver,
        options: HashMap<String, String>,
    ) {
        self.register_plugin_with_scope(
            name,
            driver,
            options,
            PluginScope::default(),
        );
    }

    pub fn register_plugin_with_scope(
        &mut self,
        name: impl Into<String>,
        driver: PluginDriver,
        options: HashMap<String, String>,
        scope: PluginScope,
    ) {
        self.plugins.push(RegisteredPlugin {
            name: name.into(),
            driver,
            options,
            scope,
        });
    }

    pub fn register_from_spec(&mut self, spec: &str) -> Result<()> {
        let driver = PluginDriver::from_file_or_command(spec)?;
        self.register_plugin(spec, driver, HashMap::new());
        Ok(())
    }

    pub fn load_from_causm_toml(&mut self, config_path: &Path) -> Result<()> {
        if !config_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(config_path).with_context(|| {
            format!("Failed to read config file '{}'", config_path.display())
        })?;

        let parsed: toml::Value = toml::from_str(&content).with_context(|| {
            format!("Failed to parse TOML in '{}'", config_path.display())
        })?;

        if let Some(plugins_table) = parsed.get("plugins").and_then(|p| p.as_table())
        {
            let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

            for (plugin_name, plugin_def) in plugins_table {
                let mut options = HashMap::new();
                let mut scope = PluginScope::default();

                let driver = match plugin_def {
                    toml::Value::String(s) => {
                        let target_path = base_dir.join(s);
                        if target_path.exists() {
                            PluginDriver::from_file_or_command(
                                target_path.to_str().unwrap_or(s),
                            )?
                        } else {
                            PluginDriver::from_file_or_command(s)?
                        }
                    }
                    toml::Value::Table(tbl) => {
                        // Read targets scope if present
                        if let Some(targets_val) = tbl.get("targets") {
                            if let Some(arr) = targets_val.as_array() {
                                scope.targets = arr
                                    .iter()
                                    .filter_map(|v| {
                                        v.as_str().map(ToString::to_string)
                                    })
                                    .collect();
                            } else if let Some(s) = targets_val.as_str() {
                                scope.targets = vec![s.to_string()];
                            }
                        }

                        // Read include scope if present
                        if let Some(incl_val) = tbl.get("include") {
                            if let Some(arr) = incl_val.as_array() {
                                scope.include = arr
                                    .iter()
                                    .filter_map(|v| {
                                        v.as_str().map(ToString::to_string)
                                    })
                                    .collect();
                            } else if let Some(s) = incl_val.as_str() {
                                scope.include = vec![s.to_string()];
                            }
                        }

                        // Read exclude scope if present
                        if let Some(excl_val) = tbl.get("exclude") {
                            if let Some(arr) = excl_val.as_array() {
                                scope.exclude = arr
                                    .iter()
                                    .filter_map(|v| {
                                        v.as_str().map(ToString::to_string)
                                    })
                                    .collect();
                            } else if let Some(s) = excl_val.as_str() {
                                scope.exclude = vec![s.to_string()];
                            }
                        }

                        // Check options subtable if any
                        if let Some(opts) =
                            tbl.get("options").and_then(|o| o.as_table())
                        {
                            for (k, v) in opts {
                                let str_val = match v {
                                    toml::Value::String(s) => s.clone(),
                                    _ => v.to_string(),
                                };
                                options.insert(k.clone(), str_val);
                            }
                        }

                        if let Some(path_str) =
                            tbl.get("path").and_then(|p| p.as_str())
                        {
                            let target_path = base_dir.join(path_str);
                            let resolved = if target_path.exists() {
                                target_path.to_str().unwrap_or(path_str)
                            } else {
                                path_str
                            };
                            PluginDriver::from_file_or_command(resolved)?
                        } else if let Some(cmd) =
                            tbl.get("command").and_then(|c| c.as_str())
                        {
                            // In wasm32, IPC plugins cannot spawn processes. Skip if target is wasm32.
                            #[cfg(target_arch = "wasm32")]
                            {
                                eprintln!(
                                    "\x1b[1;33mwarning:\x1b[0m skipping IPC plugin '{}' on wasm32 target: external command '{}' is not supported in WASI sandboxes",
                                    plugin_name, cmd
                                );
                                continue;
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                PluginDriver::Stdio(StdioPluginDriver::new(cmd))
                            }
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                // If targets are specified and don't match the current platform, skip loading
                if !scope.is_platform_supported() {
                    continue;
                }

                self.register_plugin_with_scope(plugin_name, driver, options, scope);
            }
        }

        Ok(())
    }

    pub fn run_ast_pipeline(
        &self,
        file_path: &str,
        mut program: Program,
    ) -> Result<(Program, Vec<PluginDiagnostic>)> {
        let mut all_diagnostics = Vec::new();

        for plugin in &self.plugins {
            if !plugin.scope.matches_file(file_path) {
                continue;
            }

            let req = PluginRequest::new(file_path, program.clone())
                .with_phase(crate::protocol::PluginPhase::AstTransform)
                .with_options(plugin.options.clone());

            let response = plugin.driver.transform(&req).with_context(|| {
                format!("Failed executing plugin '{}'", plugin.name)
            })?;

            all_diagnostics.extend(response.diagnostics);

            match response.status {
                PluginStatus::Success => {
                    if let Some(modified) = response.modified_ast {
                        program = modified;
                    }
                }
                PluginStatus::Error(err_msg) => {
                    if all_diagnostics.is_empty() {
                        bail!(
                            "Plugin '{}' reported failure: {}",
                            plugin.name,
                            err_msg
                        );
                    }
                }
            }
        }

        Ok((program, all_diagnostics))
    }

    pub fn run_post_analysis_pipeline(
        &self,
        file_path: &str,
        program: &Program,
        artifacts: crate::protocol::AnalysisArtifacts,
    ) -> Result<Vec<PluginDiagnostic>> {
        let mut all_diagnostics = Vec::new();

        for plugin in &self.plugins {
            if !plugin.scope.matches_file(file_path) {
                continue;
            }

            let req = PluginRequest::new(file_path, program.clone())
                .with_phase(crate::protocol::PluginPhase::PostAnalysis)
                .with_analysis(artifacts.clone())
                .with_options(plugin.options.clone());

            let response = plugin.driver.transform(&req).with_context(|| {
                format!("Failed executing post-analysis plugin '{}'", plugin.name)
            })?;

            all_diagnostics.extend(response.diagnostics);

            if let PluginStatus::Error(err_msg) = response.status {
                if all_diagnostics.is_empty() {
                    bail!(
                        "Post-analysis plugin '{}' reported failure: {}",
                        plugin.name,
                        err_msg
                    );
                }
            }
        }

        Ok(all_diagnostics)
    }

    pub fn run_ir_emit_pipeline(
        &self,
        file_path: &str,
        program: &Program,
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let mut emitted_outputs = Vec::new();

        for plugin in &self.plugins {
            if !plugin.scope.matches_file(file_path) {
                continue;
            }

            let req = PluginRequest::new(file_path, program.clone())
                .with_phase(crate::protocol::PluginPhase::IrEmit)
                .with_options(plugin.options.clone());

            let response = plugin.driver.transform(&req).with_context(|| {
                format!("Failed executing IR emit plugin '{}'", plugin.name)
            })?;

            if let Some(payload) = response.emitted_payload {
                emitted_outputs.push((plugin.name.clone(), payload));
            }
        }

        Ok(emitted_outputs)
    }
}
