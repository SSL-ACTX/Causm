use causm_analysis::analyzer::EntropicAnalyzer;
use causm_core::value::Payload;
use causm_frontend::parser;
use causm_runtime::vm::Vm;

#[test]
fn test_enum_variant_pattern_matching_statement() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        enum Status {
            Inactive,
            Active(int),
            Named(string)
        }

        let s1 = Status::Active(100)
        let mut result = 0

        match s1 {
            Status::Inactive => {
                result = -1
            },
            Status::Active(code) => {
                result = code + 5
            },
            Status::Named(name) => {
                result = 999
            },
            _ => {
                result = 0
            }
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let result_reg = ir.symbols.get("result").unwrap().0;
    let res_payload = vm.root_timeline.arena.peek(result_reg).unwrap();
    assert_eq!(res_payload, Payload::Integer(105));

    Ok(())
}

#[test]
fn test_enum_variant_pattern_matching_expression() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        enum OptionVal {
            None,
            Some(int)
        }

        let v1 = OptionVal::Some(42)
        let out = match v1 {
            OptionVal::Some(n) => n * 2,
            OptionVal::None => 0,
            _ => -1
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let out_reg = ir.symbols.get("out").unwrap().0;
    let out_payload = vm.root_timeline.arena.peek(out_reg).unwrap();
    assert_eq!(out_payload, Payload::Integer(84));

    Ok(())
}

#[test]
fn test_enum_variant_pattern_matching_with_guards() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        enum Temperature {
            Kelvin(int)
        }

        let t = Temperature::Kelvin(350)
        let mut category = ""

        match t {
            Temperature::Kelvin(k) if k > 300 => {
                category = "hot"
            },
            Temperature::Kelvin(k) => {
                category = "cold"
            },
            _ => {
                category = "unknown"
            }
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let cat_reg = ir.symbols.get("category").unwrap().0;
    let cat_payload = vm.root_timeline.arena.peek(cat_reg).unwrap();
    assert_eq!(cat_payload, Payload::String("hot".to_string()));

    Ok(())
}

#[test]
fn test_if_let_enum_variant_destructuring() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        enum Message {
            Ping,
            Echo(string)
        }

        let msg = Message::Echo("chronos")
        let mut echo_val = ""

        if let Message::Echo(text) = msg {
            echo_val = text
        } else {
            echo_val = "none"
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let echo_reg = ir.symbols.get("echo_val").unwrap().0;
    let echo_payload = vm.root_timeline.arena.peek(echo_reg).unwrap();
    assert_eq!(echo_payload, Payload::String("chronos".to_string()));

    Ok(())
}

#[test]
fn test_match_literal_and_wildcard_patterns() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let code = 404
        let msg = match code {
            200 => "ok",
            404 => "not_found",
            500 => "internal_error",
            _ => "unknown"
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let msg_reg = ir.symbols.get("msg").unwrap().0;
    let msg_payload = vm.root_timeline.arena.peek(msg_reg).unwrap();
    assert_eq!(msg_payload, Payload::String("not_found".to_string()));

    Ok(())
}

#[test]
fn test_pattern_matching_tuple_decomposition() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        let pair = (10, 20)
        let sum = match pair {
            (0, 0) => 0,
            (x, 20) => x + 20,
            (x, y) => x + y,
            _ => -1,
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let sum_reg = ir.symbols.get("sum").unwrap().0;
    let sum_payload = vm.root_timeline.arena.peek(sum_reg).unwrap();
    assert_eq!(sum_payload, Payload::Integer(30));

    Ok(())
}

#[test]
fn test_pattern_matching_nested_tuple_and_enum() -> anyhow::Result<()> {
    let source = r#"
    @0ms: {
        enum Command {
            Move(int, int),
            Stop,
        }

        let cmd = Command::Move(5, 12)
        let description = match cmd {
            Command::Stop => "halted",
            Command::Move(0, 0) => "stationary",
            Command::Move(x, y) => f"moving to {x},{y}",
            _ => "unknown",
        }
    }
    "#;

    let program = parser::parse_causm(source)?;
    let mut analyzer = EntropicAnalyzer::new();
    analyzer.analyze_program(&program)?;

    let ir = causm_frontend::lower::lower_program(&program);
    let mut vm = Vm::new();
    vm.execute_program(&ir)?;

    let desc_reg = ir.symbols.get("description").unwrap().0;
    let desc_payload = vm.root_timeline.arena.peek(desc_reg).unwrap();
    assert_eq!(
        desc_payload,
        Payload::String("moving to 5,12".to_string())
    );

    Ok(())
}
