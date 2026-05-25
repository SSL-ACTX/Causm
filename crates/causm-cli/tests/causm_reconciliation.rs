use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;

#[test]
fn test_reconcile_auto_conflict_detection() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 1
        if (true) {
            x = 2
        } else {
            x = "string"
        } reconcile auto
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    // This should fail because x has different types in branches
    assert!(result.is_err());
    if let Err(e) = result {
        println!("Expected error: {:?}", e);
    }

    Ok(())
}

#[test]
fn test_reconcile_auto_success() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 1
        let y = 10
        if (true) {
            let z = x
        } else {
            // x remains valid here
        } reconcile auto
        
        // x should be Consumed globally because it was consumed in one branch
        match entropy(x) {
            Consumed: let status = "dead"
            Valid(v): let status = "alive"
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    Ok(())
}

#[test]
fn test_reconcile_auto_decay_merging() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let s = struct { a = 1, b = 2, c = 3 }
        if (true) {
            let val = s.a
        } else {
            let val = s.b
        } reconcile auto
        
        // s should be Decayed with both a and b gone
        match entropy(s) {
            Decayed(d): {
               // This should ideally be possible to check in analyzer, 
               // but for now we just ensure it doesn't error and correctly identifies decay.
               let status = "decayed"
            }
            Valid(v): let status = "alive"
            Consumed: let status = "dead"
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    Ok(())
}
