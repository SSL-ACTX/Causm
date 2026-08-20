use causm_analysis::analyzer::EntropicAnalyzer;
use causm_frontend::parser;

#[test]
fn causm_isochronous_loop_tick_requires_slice() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      loop tick {
        let x = 1
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let res = analyzer.analyze_program(&program);
    assert!(res.is_err(), "loop tick without slice should fail analyzer");

    Ok(())
}

#[test]
fn causm_isochronous_loop_tick_slice_budget_enforced() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        slice 2ms
        loop tick {
          let x = 1
          let y = 2
          let z = 3
          break
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let res = analyzer.analyze_program(&program);
    assert!(
        res.is_err(),
        "loop tick body exceeds slice should fail analyzer"
    );

    Ok(())
}
