use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn causm_expansion_split_map_topology() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let network = [10, 20]
      let sum = 0
      for item in network step 10ms {
        let res = item * 2
        sum = sum + res
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let sum_reg = ir.symbols.get("sum").expect("sum not found").0;
    let sum_val = vm.root_timeline.arena.peek(sum_reg);

    assert_eq!(sum_val, Some(Payload::Integer(60)));
    Ok(())
}

#[test]
fn causm_expansion_speculate_inside_split_map() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let data = [1, 2, 3]
      let total = 0
      for item in data step 20ms {
        speculate (max 5ms) {
          if (clone(item) == 2) {
            collapse
          }
          commit {
            total = total + item + 10
          }
        } fallback {
          total = total + 0
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let total_reg = ir.symbols.get("total").expect("total not found").0;
    let total_val = vm.root_timeline.arena.peek(total_reg);

    // item 1: 11, item 2: 0, item 3: 13 -> total = 24
    assert_eq!(total_val, Some(Payload::Integer(24)));

    Ok(())
}

#[test]
fn causm_expansion_entanglement_speculation_rollback() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let x = "X"
      let y = "Y"
      entangle(x, y)
      speculate (max 5ms) {
        let use_x = x
        collapse
      } fallback {
        let status = "rolled_back"
      }
      
      match entropy(y) {
        Valid(v):
          let final_y = v
        Consumed:
          let final_y = "consumed"
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(causm_core::value::Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    let final_y_reg = ir.symbols.get("final_y").expect("final_y not found").0;
    let final_y_val = vm.root_timeline.arena.peek(final_y_reg);

    match final_y_val {
        Some(Payload::String(s)) => assert_eq!(s, "Y"),
        _ => panic!("Expected final_y=Y, got {:?}", final_y_val),
    }

    Ok(())
}

#[test]
fn causm_expansion_loop_tick_padding() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      slice 10ms
      loop tick {
        let x = 1
        break
      }
      // after 1 tick of 10ms, elapsed should be at least 10ms
      assert_time(elapsed >= 10ms)
      
      loop tick {
        let y = 2
        break
      }
      // after 2 ticks, elapsed should be at least 20ms
      assert_time(elapsed >= 20ms)
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(causm_core::value::Payload::Null)
        }),
    );
    vm.execute_program(&ir)?;

    Ok(())
}
