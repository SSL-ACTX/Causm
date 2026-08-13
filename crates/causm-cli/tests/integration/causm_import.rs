use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_import_file_level_and_named_symbols() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join("causm_test_imports");
    let _ = std::fs::create_dir_all(&temp_dir);

    let helper_path = temp_dir.join("helper.csm");
    std::fs::write(
        &helper_path,
        r#"
        @0ms: {
            isolate helper {
                let helper_val = 12345
            }
        }
        "#,
    )?;

    let main_path = temp_dir.join("main.csm");
    let main_source = r#"
    @0ms: {
        isolate main_iso {
            import "helper.csm" as Helper
            let final_val = 99
        }
    }
    "#;
    std::fs::write(&main_path, main_source)?;

    let program = parser::parse_causm_with_imports(main_source, Some(&temp_dir))?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let final_reg = ir.symbols.get("final_val").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(final_reg),
        Some(causm_core::value::Payload::Integer(99))
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}
