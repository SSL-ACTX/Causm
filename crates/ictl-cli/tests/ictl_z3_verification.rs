use ictl_analysis::analyzer::EntropicAnalyzer;
use ictl_frontend::parser;

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

    let program = parser::parse_ictl(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    if let Err(ref err) = result {
        println!("DEBUG ERROR: {}", err);
    }
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("Temporal Assertion Violation"));

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

    let program = parser::parse_ictl(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    if let Err(ref err) = result {
        println!("DEBUG ERROR: {}", err);
    }
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("Temporal Assertion Violation"));

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

    let program = parser::parse_ictl(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    if let Err(ref err) = result {
        println!("DEBUG ERROR: {}", err);
    }
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("has been consumed"));

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

    let program = parser::parse_ictl(source).unwrap();
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.use_z3 = true;

    let result = analyzer.analyze_program(&program);
    if let Err(ref err) = result {
        println!("DEBUG ERROR: {}", err);
    }
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{}", err).contains("Lease Violation"));

    Ok(())
}
