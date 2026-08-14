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

#[test]
fn test_import_wildcard_symbol_import() -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir().join("causm_test_wildcard_imports");
    let _ = std::fs::create_dir_all(&temp_dir);

    let math_lib_path = temp_dir.join("math_lib.csm");
    std::fs::write(
        &math_lib_path,
        r#"
        @0ms: {
            isolate math_iso {
                pub routine compute_bonus(peek val: int) -> int taking 10ms {
                    let bonus = val + 500
                    yield bonus
                }
            }
        }
        "#,
    )?;

    let main_path = temp_dir.join("main_wildcard.csm");
    let main_source = r#"
    @0ms: {
        isolate main_iso {
            from "math_lib.csm" import *

            let base_val = 100
            let res = call compute_bonus(base_val)
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

    let res_reg = ir.symbols.get("res").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(res_reg),
        Some(causm_core::value::Payload::Integer(600))
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

#[test]
fn test_import_std_time_monotonic_telemetry() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        import "std/time" as Time
        let ts = call Time.unix_timestamp()
        let start = call Time.now()
        let dur = call Time.from_millis(100)
        let total_nanos = dur.nanos_total
    }
    "#;

    let program = parser::parse_causm_with_imports(source, None)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = false;
    analyzer.analyze_program(&program)?;
    let mut vm = Vm::new();
    causm_stdlib::register_all(&mut vm);
    vm.execute_program(&ir)?;

    let total_nanos_reg = ir.symbols.get("total_nanos").unwrap().0;
    assert_eq!(
        vm.root_timeline.arena.peek(total_nanos_reg),
        Some(causm_core::value::Payload::Integer(100_000_000))
    );

    let ts_reg = ir.symbols.get("ts").unwrap().0;
    if let Some(causm_core::value::Payload::Integer(ts_val)) =
        vm.root_timeline.arena.peek(ts_reg)
    {
        assert!(ts_val > 1_700_000_000);
    } else {
        panic!("Expected valid UNIX timestamp from std/time");
    }

    Ok(())
}
