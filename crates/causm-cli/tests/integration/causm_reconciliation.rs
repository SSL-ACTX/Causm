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
               // Ensure the analyzer identifies the decay state without error.
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

#[test]
fn test_reconcile_no_else_success() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 1
        if (true) {
            x = 2
        } reconcile auto
        
        print(x)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    Ok(())
}

#[test]
fn test_select_reconcile_rules() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        open_chan ch1(1)
        open_chan ch2(1)
        let x = 10
        select (max 10ms) {
            case val1 = chan_recv(ch1): {
                x = 20
            }
            case val2 = chan_recv(ch2): {
                x = 30
            }
        } reconcile (x = first_wins)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    Ok(())
}

#[test]
fn test_split_map_reconcile_auto() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let items = [1, 2, 3]
        let result = 0
        split_map item consume items {
            result = item * 2
        } reconcile auto
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    Ok(())
}

#[test]
fn test_merge_reconcile_keyword() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 10
        split main into [b1, b2]
    }
    @10ms: {
        @b1: {
            x = 20
        }
        @b2: {
            x = 30
        }
    }
    @20ms: {
        merge [b1, b2] into main reconcile (x = first_wins)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    Ok(())
}
