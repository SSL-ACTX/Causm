use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

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

#[test]
fn causm_isochronous_tick_loop_double_buffered_channels() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate demo {
        require Chan.Manage
        require Chan.Outbound(id="c")
        require Chan.Inbound(id="c")
        slice 5ms
        open_chan c(10)

        loop tick {
          let v = 42
          chan_send c(v)
          break
        }

        loop tick {
          let out = chan_recv(c)
          break
        }
      }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").expect("out not found").0;
    match vm.root_timeline.arena.peek(out_reg) {
        Some(Payload::Integer(v)) => assert_eq!(v, 42),
        _ => panic!("Expected out=42"),
    }

    Ok(())
}

#[test]
fn causm_isochronous_matrix_complex_integration() -> anyhow::Result<()> {
    let source = r#"
@0ms: {
  isolate hft_pipeline {
    require Chan.Manage
    require Chan.Outbound(id="signal_bus")
    require Chan.Inbound(id="signal_bus")
    require System.Log
    enable system_log(1)
    enable memory(5KB)
    slice 20ms
    open_chan signal_bus(8)

    // Tick 1: market reader produces latest price signal
    loop tick {
      let price = 123
      chan_send signal_bus(price)
      break
    }

    // Tick 2: strategy consumer processes the previous tick's signal
    loop tick {
      let signal = chan_recv(signal_bus)
      let out = signal
      break
    }

    // Tick 3: publish execution outcome
    loop tick {
      let result = out
      print(result)
      break
    }
  }
}
    "#;
    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_params| Ok(()));

    vm.execute_program(&ir)?;

    // Check that we got the output of 123
    let result_reg = ir.symbols.get("result").expect("result not found").0;
    match vm.root_timeline.arena.peek(result_reg) {
        Some(Payload::Integer(v)) => assert_eq!(v, 123),
        _ => panic!("Expected result=123"),
    }

    Ok(())
}
