use causm_analysis::analyzer::{EntropicAnalyzer, SemanticErrorKind};
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn causm_lease_basic_execution() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { status = "ok", val = 10 }
      lease view = state for 10ms {
        let v = view.val
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::ir::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    // Setup (4) + Lease (1) + Block (2?) + EndLease padding = 15 or 16
    assert!(vm.root_timeline.local_clock >= 10);

    Ok(())
}

#[test]
fn causm_lease_semantic_mutation_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      lease view = state for 10ms {
        state = struct { val = 20 }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::LeaseViolation(name) => assert_eq!(name, "state"),
        _ => panic!("Expected LeaseViolation, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_lease_semantic_nested_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      lease v1 = state for 10ms {
        lease v2 = state for 5ms {
           let x = 1
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::NestedLeasing(name) => assert_eq!(name, "state"),
        _ => panic!("Expected NestedLeasing, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_lease_semantic_consumption_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      lease view = state for 10ms {
        let stolen = state
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::LeaseViolation(name) => assert_eq!(name, "state"),
        _ => panic!("Expected LeaseViolation, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_lease_semantic_view_mutation_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      lease view = state for 10ms {
        view = struct { val = 20 }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::LeaseViolation(name) => assert_eq!(name, "view"),
        _ => panic!("Expected LeaseViolation for view, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_lease_wcet_exceeded_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      lease view = state for 2ms {
        // This block will take more than 2ms in analyzer estimate
        let a = 1
        let b = 2
        let c = 3
        let d = 4
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::LeaseDurationExceeded(wcet, duration) => {
            assert!(wcet > duration);
            assert_eq!(duration, 2);
        }
        _ => panic!("Expected LeaseDurationExceeded, got {:?}", err.kind),
    }

    Ok(())
}

#[test]
fn causm_lease_restores_original_state() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      lease view = state for 10ms {
        let x = view.val
      }
      // state should be Valid here
      let final_val = state.val
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::ir::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let final_val_reg = ir.symbols.get("final_val").expect("final_val not found").0;
    let val = vm.root_timeline.arena.peek(final_val_reg);
    match val {
        Some(Payload::Integer(i)) => assert_eq!(i, 10),
        _ => panic!("Expected state.val to be 10, got {:?}", val),
    }

    Ok(())
}

#[test]
fn causm_lease_semantic_illegal_control_flow_fails() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let state = struct { val = 10 }
      loop (max 100ms) {
        lease view = state for 10ms {
          break
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    let result = analyzer.analyze_program(&program);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match *err.kind {
        SemanticErrorKind::IllegalLeaseControlFlow => {}
        _ => panic!("Expected IllegalLeaseControlFlow, got {:?}", err.kind),
    }

    Ok(())
}
