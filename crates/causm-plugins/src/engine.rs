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

#[derive(Default)]
pub struct PluginEngine {
    pub plugins: Vec<(String, PluginDriver, HashMap<String, String>)>,
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
        self.plugins.push((name.into(), driver, options));
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
                            PluginDriver::Stdio(StdioPluginDriver::new(cmd))
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };

                self.register_plugin(plugin_name, driver, options);
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

        for (name, plugin, options) in &self.plugins {
            let req = PluginRequest::new(file_path, program.clone())
                .with_phase(crate::protocol::PluginPhase::AstTransform)
                .with_options(options.clone());

            let response = plugin
                .transform(&req)
                .with_context(|| format!("Failed executing plugin '{}'", name))?;

            all_diagnostics.extend(response.diagnostics);

            match response.status {
                PluginStatus::Success => {
                    if let Some(modified) = response.modified_ast {
                        program = modified;
                    }
                }
                PluginStatus::Error(err_msg) => {
                    if all_diagnostics.is_empty() {
                        bail!("Plugin '{}' reported failure: {}", name, err_msg);
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

        for (name, plugin, options) in &self.plugins {
            let req = PluginRequest::new(file_path, program.clone())
                .with_phase(crate::protocol::PluginPhase::PostAnalysis)
                .with_analysis(artifacts.clone())
                .with_options(options.clone());

            let response = plugin.transform(&req).with_context(|| {
                format!("Failed executing post-analysis plugin '{}'", name)
            })?;

            all_diagnostics.extend(response.diagnostics);

            if let PluginStatus::Error(err_msg) = response.status {
                if all_diagnostics.is_empty() {
                    bail!(
                        "Post-analysis plugin '{}' reported failure: {}",
                        name,
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

        for (name, plugin, options) in &self.plugins {
            let req = PluginRequest::new(file_path, program.clone())
                .with_phase(crate::protocol::PluginPhase::IrEmit)
                .with_options(options.clone());

            let response = plugin.transform(&req).with_context(|| {
                format!("Failed executing IR emit plugin '{}'", name)
            })?;

            if let Some(payload) = response.emitted_payload {
                emitted_outputs.push((name.clone(), payload));
            }
        }

        Ok(emitted_outputs)
    }
}
