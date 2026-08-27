use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn causm_acausal_speculate_commit_fallback_timing() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x = "hello"
      speculate (max 3ms) {
        let y = x
        commit {
          let out = y
        }
      } fallback {
        let out = "fallback"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out register not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "hello"),
        _ => panic!("Expected out=hello, got {:?}", out_val),
    }

    // Verify that the timeline has progressed.
    assert!(vm.root_timeline.local_clock > 0);
    Ok(())
}

#[test]
fn causm_acausal_speculate_runs_fallback_on_collapse() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      speculate (max 3ms) {
        collapse
      } fallback {
        let out = "fallback"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out register not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "fallback"),
        _ => panic!("Expected out=fallback, got {:?}", out_val),
    }

    assert!(vm.root_timeline.local_clock > 0);
    Ok(())
}

#[test]
fn causm_acausal_speculate_commit_scoped_variables() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      speculate (max 3ms) {
        let secret = "hidden"
        commit {
          let out = "committed"
        }
      } fallback {
        let out = "fallback"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.set_speculative_commit_mode(causm_core::SpeculationCommitMode::Full);
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out register not found").0;
    let out_val = vm.root_timeline.arena.peek(out_reg);
    match out_val {
        Some(Payload::String(s)) => assert_eq!(s, "committed"),
        _ => panic!("Expected out=committed, got {:?}", out_val),
    }

    let secret_reg = ir
        .symbols
        .get("secret")
        .expect("secret register not found")
        .0;
    let secret_val = vm.root_timeline.arena.peek(secret_reg);
    match secret_val {
        Some(Payload::String(s)) => assert_eq!(s, "hidden"),
        _ => panic!("Expected secret=hidden, got {:?}", secret_val),
    }

    Ok(())
}

#[test]
fn causm_acausal_speculate_selective_commit_mode() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      speculate (max 3ms) {
        let secret = "hidden"
        // NO COMMIT
      } fallback {
        let out = "fallback"
      }
      let final_out = "done"
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.set_speculative_commit_mode(causm_core::SpeculationCommitMode::Selective);
    vm.execute_program(&ir)?;

    let secret_reg = ir
        .symbols
        .get("secret")
        .expect("secret register not found")
        .0;
    // In selective mode without commit, 'secret' should be rolled back
    assert!(vm.root_timeline.arena.peek(secret_reg).is_none());

    let final_out_reg = ir
        .symbols
        .get("final_out")
        .expect("final_out register not found")
        .0;
    assert!(vm.root_timeline.arena.peek(final_out_reg).is_some());

    Ok(())
}
