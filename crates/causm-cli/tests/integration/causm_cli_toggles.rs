use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_cli_no_z3_bypass_verification() -> anyhow::Result<()> {
    let source = r#"
    @0ms @no_z3: {
        assert_time(elapsed >= 5ms)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true; // Z3 is enabled globally

    let result = analyzer.analyze_program(&program);
    assert!(result.is_ok(), "Expected verification to pass because @no_z3 was set on the block, got {:?}", result);

    Ok(())
}

#[test]
fn test_cli_chaos_mode_prevents_rewind() -> anyhow::Result<()> {
    let source = r#"
    @0ms @chaos: {
        anchor start
        rewind_to(start)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();

    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_err(),
        "Expected compilation error since rewind is disabled in chaos mode"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("Chaos Mode enabled"),
        "Expected chaos mode error, got: {}",
        err
    );

    Ok(())
}

#[test]
fn test_cli_deterministic_mode_allows_rewind() -> anyhow::Result<()> {
    let source = r#"
    @0ms @deterministic: {
        split main into [w]
        @w: {
            anchor start
            let y = 1
        }
        reset w to start
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();

    let result = analyzer.analyze_program(&program);
    if let Err(ref e) = result {
        println!("Analysis failed with error: {:?}", e);
    }
    assert!(result.is_ok());

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    let result_exec = vm.execute_program(&ir);
    assert!(result_exec.is_ok());

    Ok(())
}

#[test]
fn test_cli_chaos_mode_prevents_rewind_at_runtime() -> anyhow::Result<()> {
    // Force chaos mode at runtime using VM setup to verify the runtime safety path
    let source = r#"
    @0ms: {
        split main into [w]
        @w: {
            anchor start
            let y = 1
        }
        reset w to start
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);
    if let Err(ref e) = result {
        println!("Analysis failed at runtime test with error: {:?}", e);
    }
    assert!(result.is_ok()); // Normal analysis passes since block doesn't declare @chaos

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    // Simulate --chaos CLI flag by setting VM timeline entropy mode directly
    vm.root_timeline.entropy_mode = causm_core::EntropyMode::Chaos;

    let result_exec = vm.execute_program(&ir);
    assert!(
        result_exec.is_err(),
        "Expected execution failure due to rewind in global chaos mode"
    );
    let err = format!("{}", result_exec.unwrap_err());
    assert!(
        err.contains("Entropy Violation"),
        "Expected entropy violation error, got: {}",
        err
    );

    Ok(())
}

#[test]
fn test_cli_block_level_no_z3_bypass_verification() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        @no_z3: {
            assert_time(elapsed >= 5ms)
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_ok(), "Expected verification to pass because block-level @no_z3 bypassed Z3, got: {:?}", result);

    Ok(())
}

#[test]
fn test_cli_block_level_chaos_mode_prevents_rewind() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        @chaos: {
            anchor start
            rewind_to(start)
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();

    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_err(),
        "Expected compilation error since rewind is disabled in chaos block"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("Chaos Mode enabled"),
        "Expected chaos mode error, got: {}",
        err
    );

    Ok(())
}

#[test]
fn test_cli_block_level_deterministic_mode_allows_rewind() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        @chaos: {
            @deterministic: {
                split main into [w]
                @w: {
                    anchor start
                    let y = 1
                }
                reset w to start
            }
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();

    let result = analyzer.analyze_program(&program);
    assert!(result.is_ok());

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    let result_exec = vm.execute_program(&ir);
    assert!(result_exec.is_ok());

    Ok(())
}
