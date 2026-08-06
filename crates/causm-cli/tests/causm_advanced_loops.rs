use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_bounded_while_loop() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = 0
        while (x < 3) (max 10ms) {
            x = x + 1
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let main = &vm.root_timeline;
    // Clock should be padded to at least the loop max budget (10ms) + setup overhead
    assert!(main.local_clock >= 10);

    let x_reg = ir.symbols.get("x").expect("x not found").0;
    assert_eq!(main.arena.peek(x_reg), Some(Payload::Integer(3)));

    Ok(())
}

#[test]
fn test_while_valid_loop() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let x = "data"
        let count = 0
        while valid (x) (max 15ms) {
            count = count + 1
            if (count == 2) {
                let y = x
            } reconcile auto
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let main = &vm.root_timeline;
    let count_reg = ir.symbols.get("count").expect("count not found").0;
    // Loop should execute exactly twice, and then exit because x was consumed by y = x
    assert_eq!(main.arena.peek(count_reg), Some(Payload::Integer(2)));

    Ok(())
}

#[test]
fn test_for_step_loop() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let arr = [10, 20, 30]
        let sum = 0
        for val in arr step 5ms {
            sum = sum + val
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let main = &vm.root_timeline;
    let sum_reg = ir.symbols.get("sum").expect("sum not found").0;
    assert_eq!(main.arena.peek(sum_reg), Some(Payload::Integer(60)));

    // 3 iterations, each paced at 5ms
    assert!(main.local_clock >= 15);

    Ok(())
}

#[test]
fn test_loop_tick_on_channel() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        open_chan c(10)
        let ev = "event"
        chan_send c(ev)
        slice 10ms
        loop tick on c {
            let msg = chan_recv(c)
            break
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    Ok(())
}

#[test]
fn test_advanced_loop_showcase() -> anyhow::Result<()> {
    let source = include_str!("../../../examples/advanced_loop_sc.csm");
    let program = parser::parse_causm(source)?;
    let ir = causm_frontend::lower::lower_program(&program);

    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let mut vm = Vm::new();
    vm.register_capability("System.Log", |_| Ok(()));
    vm.execute_program(&ir)?;

    let main = &vm.root_timeline;
    let sum_reg = ir.symbols.get("sum").expect("sum not found").0;
    let peak_reg = ir.symbols.get("peak").expect("peak not found").0;
    assert_eq!(main.arena.peek(sum_reg), Some(Payload::Integer(191)));
    assert_eq!(main.arena.peek(peak_reg), Some(Payload::Integer(93)));

    Ok(())
}
