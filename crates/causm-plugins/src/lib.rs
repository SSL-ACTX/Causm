pub mod engine;
pub mod ipc;
pub mod protocol;
pub mod wasm;

pub use engine::{PluginDriver, PluginEngine, PluginScope, RegisteredPlugin};
pub use ipc::StdioPluginDriver;
pub use protocol::*;
pub use wasm::WasmPluginDriver;

#[cfg(test)]
mod tests {
    use super::*;
    use causm_core::*;

    #[test]
    fn test_plugins_stdio_driver_transform() {
        // Python one-liner plugin that appends a warning diagnostic and increments all Integer literals by 10
        let python_cmd = r#"python3 -c '
import sys, json

data = json.load(sys.stdin)
ast = data["ast"]

# Add a warning diagnostic
diagnostics = [{
    "level": "Warning",
    "message": "Transformed by test python plugin",
    "span": None
}]

# Mutate the AST: for any Integer expression, add 10
for tb in ast.get("timelines", []):
    for stmt_obj in tb.get("statements", []):
        stmt = stmt_obj.get("stmt", {})
        if "Assignment" in stmt:
            expr = stmt["Assignment"].get("expr", {})
            if "Integer" in expr:
                expr["Integer"] += 10

out = {
    "status": "Success",
    "modified_ast": ast,
    "diagnostics": diagnostics
}
json.dump(out, sys.stdout)
'"#;

        let driver = StdioPluginDriver::new(python_cmd);
        let program = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Assignment {
                        target: "a".to_string(),
                        mutable: false,
                        var_type: None,
                        lifetime: None,
                        expr: Expression::Integer(5),
                    },
                    Span { start: 0, end: 10 },
                )],
            }],
        };

        let req = PluginRequest::new("test.csm", program);
        let resp = driver
            .transform(&req)
            .expect("stdio transform should succeed");

        assert_eq!(resp.status, PluginStatus::Success);
        assert_eq!(resp.diagnostics.len(), 1);
        assert_eq!(resp.diagnostics[0].level, DiagnosticLevel::Warning);
        assert_eq!(
            resp.diagnostics[0].message,
            "Transformed by test python plugin"
        );

        let modified_ast =
            resp.modified_ast.expect("modified ast should be returned");
        if let Statement::Assignment { expr, .. } =
            &modified_ast.timelines[0].statements[0].stmt
        {
            assert_eq!(*expr, Expression::Integer(15));
        } else {
            panic!("Expected assignment statement");
        }
    }

    #[test]
    fn test_plugins_wasm_driver_transform() {
        // Handcrafted WASM module using WAT:
        // Returns a statically encoded `PluginResponse::success(Some(ast), vec![])`
        // In the test, the WASM plugin writes a response payload into memory and returns (out_ptr << 32 | out_len)
        let sample_program = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(10),
                no_z3: true,
                entropy_mode: None,
                statements: vec![SpannedStatement::new(
                    Statement::Return(Some(Expression::Integer(99))),
                    Span { start: 0, end: 5 },
                )],
            }],
        };
        let sample_resp = PluginResponse::success(
            Some(sample_program.clone()),
            vec![PluginDiagnostic {
                level: DiagnosticLevel::Note,
                message: "WASM plugin executed successfully".to_string(),
                span: None,
            }],
        );
        let resp_bytes =
            bincode::serialize(&sample_resp).expect("serialize sample_resp");
        let resp_len = resp_bytes.len();

        // Generate WAT data segment with exact resp_bytes at offset 2048
        let data_bytes_wat = resp_bytes
            .iter()
            .map(|b| format!("\\{:02x}", b))
            .collect::<String>();

        let wat_src = format!(
            r#"
            (module
                (memory (export "memory") 1)
                (data (i32.const 2048) "{}")
                (global $bump (mut i32) (i32.const 4096))

                (func (export "causm_plugin_alloc") (param $len i32) (result i32)
                    (local $ptr i32)
                    (local.set $ptr (global.get $bump))
                    (global.set $bump (i32.add (global.get $bump) (local.get $len)))
                    (local.get $ptr)
                )

                (func (export "causm_plugin_dealloc") (param $ptr i32) (param $len i32)
                    ;; no-op
                )

                (func (export "causm_plugin_transform") (param $in_ptr i32) (param $in_len i32) (result i64)
                    ;; Returns (2048 << 32) | {}
                    (i64.or
                        (i64.shl (i64.const 2048) (i64.const 32))
                        (i64.const {})
                    )
                )
            )
            "#,
            data_bytes_wat, resp_len, resp_len
        );

        let wasm_bytes =
            wat::parse_str(&wat_src).expect("wat parsing should succeed");
        let driver = WasmPluginDriver::new(wasm_bytes);

        let req = PluginRequest::new("wasm_test.csm", sample_program.clone());
        let resp = driver
            .transform(&req)
            .expect("WASM plugin transform should succeed");

        assert_eq!(resp.status, PluginStatus::Success);
        assert_eq!(resp.diagnostics.len(), 1);
        assert_eq!(
            resp.diagnostics[0].message,
            "WASM plugin executed successfully"
        );
        let returned_ast = resp.modified_ast.expect("returned ast");
        assert_eq!(sample_program, returned_ast);
    }

    #[test]
    fn test_plugins_causm_toml_discovery() {
        let temp_dir = std::env::temp_dir()
            .join(format!("causm_test_toml_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);

        let causm_toml_content = r#"
[package]
name = "test_pkg"
version = "0.1.0"

[plugins.validator]
command = "python3 -c 'import sys, json; data = json.load(sys.stdin); json.dump({\"status\": \"Success\", \"modified_ast\": data[\"ast\"], \"diagnostics\": []}, sys.stdout)'"

[plugins.validator.options]
strict = true
"#;
        let config_file = temp_dir.join("causm.toml");
        std::fs::write(&config_file, causm_toml_content)
            .expect("write temp causm.toml");

        let mut engine = PluginEngine::new();
        engine
            .load_from_causm_toml(&config_file)
            .expect("load from causm.toml should succeed");

        let program = Program {
            timelines: vec![TimelineBlock {
                time: TimeCoordinate::Global(0),
                no_z3: false,
                entropy_mode: None,
                statements: vec![],
            }],
        };

        let (result_ast, diags) = engine
            .run_ast_pipeline("test.csm", program.clone())
            .expect("run_ast_pipeline with discovered plugin");

        assert_eq!(program, result_ast);
        assert!(diags.is_empty());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_plugins_wasm_driver_fuel_exhaustion() {
        // Module with an infinite loop inside causm_plugin_transform
        let wat_src = r#"
        (module
            (memory (export "memory") 1)
            (func (export "causm_plugin_alloc") (param $len i32) (result i32)
                (i32.const 0)
            )
            (func (export "causm_plugin_dealloc") (param $ptr i32) (param $len i32)
            )
            (func (export "causm_plugin_transform") (param $in_ptr i32) (param $in_len i32) (result i64)
                (loop $inf (br $inf))
                (i64.const 0)
            )
        )
        "#;
        let wasm_bytes = wat::parse_str(wat_src).expect("wat parse should succeed");
        // Limit driver to a low fuel budget
        let driver = WasmPluginDriver::new(wasm_bytes).with_fuel_budget(500);

        let sample_program = Program { timelines: vec![] };
        let req = PluginRequest::new("infinite_loop.csm", sample_program);
        let res = driver.transform(&req);

        // Transformation MUST fail due to fuel exhaustion (cannot freeze host)
        assert!(res.is_err());
        let err_str = format!("{:#}", res.unwrap_err());
        assert!(
            err_str.contains("fuel")
                || err_str.contains("trap")
                || err_str.contains("consumed"),
            "Error was: {}",
            err_str
        );
    }

    #[test]
    fn test_plugins_scope_file_matching() {
        let mut scope = engine::PluginScope::default();
        scope.include = vec!["telemetry".to_string()];
        scope.exclude = vec!["mock".to_string()];

        assert!(scope.matches_file("examples/telemetry_worker.csm"));
        assert!(!scope.matches_file("examples/net_collection_showcase.csm"));
        assert!(!scope.matches_file("examples/telemetry_mock.csm"));
    }

    #[test]
    fn test_plugins_scope_platform_filtering() {
        let mut scope = engine::PluginScope::default();
        scope.targets = vec!["native".to_string()];

        #[cfg(not(target_arch = "wasm32"))]
        assert!(scope.is_platform_supported());

        #[cfg(target_arch = "wasm32")]
        assert!(!scope.is_platform_supported());
    }
}
