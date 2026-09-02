use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;

#[test]
fn test_z3_temporal_violation() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let x = 10
            let y = 20
            // This costs 2ms before assert_time. (let x, let y)
            assert_time(elapsed >= 5ms)
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("Temporal Assertion Violation"));

    Ok(())
}

#[test]
fn test_z3_isolate_budget_violation() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            isolate slow_worker {
                enable cpu(2ms)
                let x = 1
                let y = 2
                let z = 3 
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("Temporal Assertion Violation"));

    Ok(())
}

#[test]
fn test_z3_use_after_consume_symbolic() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let x = 10
            if (true) {
                yield x
            } else {
                let y = x
            } reconcile auto
            let z = x
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("use of consumed variable")
            || err.contains("has been consumed"),
        "Expected use-after-consume error, got: {err}"
    );

    Ok(())
}

#[test]
fn test_z3_lease_violation_symbolic() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let x = 10
            lease b = x for 100ms {
                // b is leased, cannot be consumed
                yield b
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("Lease Violation"));

    Ok(())
}

#[test]
fn test_z3_loop_double_consume() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let x = 100
            loop (max 10ms) {
                // This consumes x in the first iteration.
                // The second (unrolled) iteration should fail in Z3.
                yield x
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("use of consumed variable")
            || err.contains("has been consumed"),
        "Expected use-after-consume error, got: {err}"
    );

    Ok(())
}

#[test]
fn test_z3_for_loop_item_safety() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let items = [1, 2, 3]
            for item clone items {
                // item is fresh each iteration
                yield item
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_ok(),
        "For loop item should be safe to yield each iteration: {:?}",
        result.err()
    );

    Ok(())
}

#[test]
fn test_z3_causal_paradox() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            anchor start
            let x = 10
            yield x // Commitment!
            rewind_to(start)  // PARADOX: cannot rewind past commitment
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("Causal Paradox") || err.contains("causal horizon"),
        "Expected causal paradox error, got: {err}"
    );

    Ok(())
}

#[test]
fn test_z3_entanglement_violation() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let x = 1
            let y = 2
            entangle(x, y)
            yield x
            // y is now decayed because it is entangled with x
            let z = y
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("entangled variable")
            || err.contains("use of consumed variable")
            || err.contains("has been consumed"),
        "Expected entanglement or consume error, got: {err}"
    );

    Ok(())
}

#[test]
fn test_z3_routine_contract_violation() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            routine slow_calc() taking 2ms {
                let a = 1
                let b = 2
                let c = 3
                let d = 4 
                let e = 5 
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    // Either caught by standard or Z3
    assert!(
        err.contains("Temporal Assertion Violation")
            || err.contains("Routine temporal contract violated")
    );

    Ok(())
}

#[test]
fn test_z3_routine_call_propagation() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            routine fast() taking 5ms {
                let x = 1
            }
            
            isolate boss {
                enable cpu(10ms)
                let _ = fast()
                let _ = fast()
                let z = 1 // 5 + 5 + 1 = 11ms, exceeds 10ms budget
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("Temporal Assertion Violation"));

    Ok(())
}

#[test]
fn test_z3_for_loop_budget_violation() -> anyhow::Result<()> {
    let source = r#"
        @main: {
            let items = [1, 2, 3]
            for item clone items pacing 15ms (max 10ms) {
                let y = item
            }
        }
    "#;

    let program = parser::parse_causm(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("Temporal Assertion Violation"));

    Ok(())
}
