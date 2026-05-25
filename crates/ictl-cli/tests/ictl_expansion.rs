use ictl_analysis::analyzer::EntropicAnalyzer;
use ictl_core::value::Payload;
use ictl_frontend::parser;
use ictl_runtime::vm::Vm;

#[test]
fn ictl_expansion_split_map_topology() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let network = topology {
        "node1": 10,
        "node2": 20
      }
      split_map item clone network {
        // item will be a struct { key: "node1", value: 10 } or similar
        let val = item.value
        let res = val * 2
        yield res
      } reconcile (result=first_wins)
    }
    "#;

    let program = parser::parse_ictl(source)?;
    let ir = ictl_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(())
        }),
    );
    vm.execute_program(&ir)?;

    let results_reg = ir
        .symbols
        .get("splitmap_results")
        .expect("splitmap_results not found")
        .0;
    let results_val = vm.root_timeline.arena.peek(results_reg);

    match results_val {
        Some(Payload::Array(arr)) => {
            assert_eq!(arr.len(), 2);
            // Payloads are sorted by key name in VM: node1, node2
            // node1 -> 10 * 2 = 20
            // node2 -> 20 * 2 = 40
            match &arr[0] {
                Payload::Integer(v) => assert_eq!(*v, 20),
                _ => panic!("Expected integer 20, got {:?}", arr[0]),
            }
            match &arr[1] {
                Payload::Integer(v) => assert_eq!(*v, 40),
                _ => panic!("Expected integer 40, got {:?}", arr[1]),
            }
        }
        _ => panic!("Expected array, got {:?}", results_val),
    }

    Ok(())
}

#[test]
fn ictl_expansion_speculate_inside_split_map() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      let data = [1, 2, 3]
      split_map item consume data {
        speculate (max 5ms) {
          if (clone(item) == 2) {
            collapse
          }
          let res = item + 10
          yield res
        } fallback {
          let res = 0
          yield res
        }
      } reconcile (result=first_wins)
    }
    "#;

    let program = parser::parse_ictl(source)?;
    let ir = ictl_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(())
        }),
    );
    vm.execute_program(&ir)?;

    let results_reg = ir
        .symbols
        .get("splitmap_results")
        .expect("splitmap_results not found")
        .0;
    let results_val = vm.root_timeline.arena.peek(results_reg);

    match results_val {
        Some(Payload::Array(arr)) => {
            assert_eq!(arr.len(), 3);
            // item=1 -> 1+10=11
            // item=2 -> collapse -> fallback -> 0
            // item=3 -> 3+10=13
            match &arr[0] {
                Payload::Integer(v) => assert_eq!(*v, 11),
                _ => panic!("Expected 11"),
            }
            match &arr[1] {
                Payload::Integer(v) => assert_eq!(*v, 0),
                _ => panic!("Expected 0"),
            }
            match &arr[2] {
                Payload::Integer(v) => assert_eq!(*v, 13),
                _ => panic!("Expected 13"),
            }
        }
        _ => panic!("Expected array"),
    }

    Ok(())
}

#[test]
fn ictl_expansion_loop_tick_double_buffer() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      isolate producer_consumer {
        require Chan.Outbound(id="c")
        require Chan.Inbound(id="c")
        slice 10ms
        enable cpu(100ms)
        open_chan c(2)
        
        // producer
        split main into [p, c1]
        @p: {
            loop tick {
                let x = 1
                chan_send c(x)
                break
            }
            loop tick {
                let x = 2
                chan_send c(x)
                break
            }
        }
        
        @c1: {
            // First tick: nothing yet (double buffer)
            loop tick {
                // try receive - should fail if it reads from current tick
                // but chan_recv in ICTL blocks or fails if empty?
                // Spec says: "read exclusively from data committed during the PRECEDING tick"
                break
            }
            // Second tick: should get '1'
            loop tick {
                let val = chan_recv(c)
                let out = val
                break
            }
        }
      }
    }
    "#;

    let program = parser::parse_ictl(source)?;
    let ir = ictl_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(())
        }),
    );
    vm.execute_program(&ir)?;

    let consumer = vm.active_branches.get("c1").unwrap();
    let out_reg = ir.symbols.get("out").expect("out not found").0;
    let out_val = consumer.arena.peek(out_reg);

    match out_val {
        Some(Payload::Integer(v)) => assert_eq!(v, 1),
        _ => panic!("Expected out=1 (from first tick of producer)"),
    }

    Ok(())
}

#[test]
fn ictl_expansion_entanglement_speculation_rollback() -> anyhow::Result<()> {
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

    let program = parser::parse_ictl(source)?;
    let ir = ictl_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(())
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
fn ictl_expansion_select_timing_determinism() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
      open_chan c(1)
      let msg = "ready"
      chan_send c(msg)
      slice 1ms
      loop tick { break } // commit send
      
      let out = "init"
      select (max 10ms) {
        case data = chan_recv(c):
          out = data
        timeout:
          out = "timeout"
      }
      debug(out)
      assert_time(elapsed >= 10ms)
    }
    "#;

    let program = parser::parse_ictl(source)?;
    let ir = ictl_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(())
        }),
    );
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn ictl_expansion_loop_tick_padding() -> anyhow::Result<()> {
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

    let program = parser::parse_ictl(source)?;
    let ir = ictl_frontend::ir::lower_program(&program);
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.capability_handlers.insert(
        "System.Log".to_string(),
        Box::new(|params| {
            if let Some(msg) = params.get("message") {
                println!("[LOG] {}", msg);
            }
            Ok(())
        }),
    );
    vm.execute_program(&ir)?;

    Ok(())
}
