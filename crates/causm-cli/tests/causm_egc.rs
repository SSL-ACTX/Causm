use causm_analysis::analyzer::{EntropicAnalyzer, SemanticErrorKind};
use causm_frontend::parser;

#[test]
fn test_egc_consumed_success() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 10
        let y = x
        let _ = y
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.enforce_egc = true;

    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_ok(),
        "Expected analyze to succeed, got {:?}",
        result
    );
    Ok(())
}

#[test]
fn test_egc_ignored_success() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let _x = 10
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.enforce_egc = true;

    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_ok(),
        "Expected analyze to succeed, got {:?}",
        result
    );
    Ok(())
}

#[test]
fn test_egc_unconsumed_leak_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 10
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.enforce_egc = true;

    let result = analyzer.analyze_program(&program);
    assert!(
        result.is_err(),
        "Expected analyze to fail due to unconsumed variable"
    );

    let err = result.unwrap_err();
    assert!(
        matches!(*err.kind, SemanticErrorKind::UnconsumedVariable(ref name) if name == "x"),
        "Expected UnconsumedVariable(x), got {:?}",
        err.kind
    );
    Ok(())
}
