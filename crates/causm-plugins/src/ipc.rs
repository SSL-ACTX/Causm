use crate::protocol::{PluginRequest, PluginResponse};
#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context;
use anyhow::{bail, Result};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};

pub struct StdioPluginDriver {
    command_line: String,
}

impl StdioPluginDriver {
    pub fn new(command_line: impl Into<String>) -> Self {
        Self {
            command_line: command_line.into(),
        }
    }

    pub fn transform(&self, request: &PluginRequest) -> Result<PluginResponse> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = request;
            bail!(
                "Stdio IPC plugins (external process '{}') are not supported when compiling or running inside WebAssembly / WASI. Use WASM plugins instead.",
                self.command_line
            );
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let req_json = serde_json::to_string(request)
                .context("Failed to serialize PluginRequest to JSON")?;

            let mut child = Command::new("sh")
                .arg("-c")
                .arg(&self.command_line)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .with_context(|| {
                    format!("Failed to spawn plugin process '{}'", self.command_line)
                })?;

            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(req_json.as_bytes())?;
                stdin.flush()?;
            }

            let output = child.wait_with_output().with_context(|| {
                format!("Failed waiting for plugin process '{}'", self.command_line)
            })?;

            if !output.status.success() {
                bail!(
                    "Plugin process '{}' exited with non-zero exit code: {:?}",
                    self.command_line,
                    output.status.code()
                );
            }

            let stdout_str = String::from_utf8(output.stdout)
                .context("Plugin process stdout was not valid UTF-8")?;

            let response: PluginResponse = serde_json::from_str(&stdout_str)
                .with_context(|| {
                    format!(
                        "Failed to parse PluginResponse from stdout: {}",
                        stdout_str
                    )
                })?;

            Ok(response)
        }
    }
}
