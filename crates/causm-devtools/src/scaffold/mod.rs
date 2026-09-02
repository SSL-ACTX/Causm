// crates/causm-devtools/src/scaffold/mod.rs
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginTemplate {
    RustWasm,
    PythonIpc,
}

impl PluginTemplate {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "rust" | "wasm" | "rust-wasm" => Some(PluginTemplate::RustWasm),
            "python" | "py" | "ipc" => Some(PluginTemplate::PythonIpc),
            _ => None,
        }
    }
}

pub fn scaffold_plugin_project(
    name: &str,
    template: PluginTemplate,
    dest_dir: &Path,
) -> std::io::Result<PathBuf> {
    let project_dir = dest_dir.join(name);
    if project_dir.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Directory '{}' already exists", project_dir.display()),
        ));
    }
    fs::create_dir_all(&project_dir)?;

    match template {
        PluginTemplate::PythonIpc => {
            let script_path = project_dir.join(format!("{}.py", name));
            fs::write(&script_path, PYTHON_TEMPLATE)?;
            Ok(script_path)
        }
        PluginTemplate::RustWasm => {
            let src_dir = project_dir.join("src");
            fs::create_dir_all(&src_dir)?;

            let cargo_toml = format!(
                r#"[package]
name = "{}"
version = "0.0.1"
edition = "2021"
license = "AGPL-3.0-only"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
causm-plugin-sdk = {{ path = "../../crates/causm-plugin-sdk" }}
causm-core = {{ path = "../../crates/causm-core" }}
"#,
                name
            );
            fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;
            fs::write(src_dir.join("lib.rs"), RUST_WASM_TEMPLATE)?;
            Ok(project_dir)
        }
    }
}

const PYTHON_TEMPLATE: &str = r#"#!/usr/bin/env python3
import sys
import json

def main():
    # Read PluginRequest JSON from standard input
    data = json.load(sys.stdin)
    ast = data.get("ast", {})

    # Inspect or transform AST
    diagnostics = []

    response = {
        "status": "Success",
        "modified_ast": ast,
        "emitted_payload": None,
        "diagnostics": diagnostics
    }
    json.dump(response, sys.stdout)

if __name__ == "__main__":
    main()
"#;

const RUST_WASM_TEMPLATE: &str = r#"use causm_plugin_sdk::prelude::*;

#[causm_plugin]
pub fn transform(program: Program, _ctx: &PluginContext) -> Result<Program, PluginError> {
    // Implement AST inspection, linting, or transformation
    Ok(program)
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaffold_rust_wasm_plugin() {
        let temp_dir = std::env::temp_dir()
            .join(format!("causm_scaffold_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let created = scaffold_plugin_project(
            "my_wasm_linter",
            PluginTemplate::RustWasm,
            &temp_dir,
        )
        .expect("scaffolding rust plugin should succeed");

        assert!(created.join("Cargo.toml").exists());
        assert!(created.join("src/lib.rs").exists());

        let cargo_str = fs::read_to_string(created.join("Cargo.toml")).unwrap();
        assert!(cargo_str.contains("my_wasm_linter"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_scaffold_python_ipc_plugin() {
        let temp_dir = std::env::temp_dir()
            .join(format!("causm_scaffold_py_test_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let created = scaffold_plugin_project(
            "my_py_linter",
            PluginTemplate::PythonIpc,
            &temp_dir,
        )
        .expect("scaffolding python plugin should succeed");

        assert!(created.exists());
        assert!(created.ends_with("my_py_linter.py"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
